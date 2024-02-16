use std::{
    fs,
    io::{prelude::*, BufReader, Read},
    net::{TcpListener, TcpStream},
    process::Command,
    path::Path
};

use regex::Regex;
use std::fs::File;
use tileset_conversion_server::ThreadPool;
use num_cpus;

const TILESET_URL: &str = "https://waapi.webatlas.no/3d-tiles/tileserver.fcgi/";
const API_KEY: &str = "?api_key=DB124B20-9D21-4647-B65A-16C651553E48";

const PATH_TILESET_DIR: &str = "tmp/tilesets";
const PATH_B3DM_DIR: &str = "tmp/b3dms";
const PATH_GLB_DIR: &str = "tmp/glbs";

fn main() {
    // Ensure the required 3DTiles-1.0 directory exists
    fs::create_dir_all(PATH_TILESET_DIR).expect(format!("Couldn't create required dir {}", PATH_TILESET_DIR).as_str());
    fs::create_dir_all(PATH_B3DM_DIR).expect(format!("Couldn't create required dir {}", PATH_B3DM_DIR).as_str());
    fs::create_dir_all(PATH_GLB_DIR).expect(format!("Couldn't create required dir {}", PATH_GLB_DIR).as_str());

    let listener = TcpListener::bind("127.0.0.1:7878").expect("Failed to bind TcpListener");
    let pool = ThreadPool::new(num_cpus::get());

    for stream in listener.incoming().take(2) {
        let stream = stream.unwrap();
        pool.execute(|| {
            handle_connection(stream);
        });
    }

    println!("Shutting down.");
}

fn handle_connection(mut stream: TcpStream) {
    // Parse received request from Unity
    let buf_reader = BufReader::new(&mut stream);
    let http_request: Vec<_> = buf_reader
        .lines()
        .map(|result| result.unwrap())
        .take_while(|line| !line.is_empty())
        .collect();
    // println!("Request from Unity: {:#?}", http_request.first().unwrap());


    
    // Create response back to Unity
    let request_path = http_request.first().unwrap();
    let re = Regex::new(r"(?<match>[0-9]*tileset.json|[0-9]+model.cmpt|[0-9]+model.b3dm|[0-9]+model)").unwrap();
    let Some(caps) = re.captures(request_path) else {
        println!("No match found for request!");
        return;
    };

    stream_tileset(&stream, &caps["match"]);
    // println!("Done!")
}

/////// RESPONSE FUNCTIONS ////////
fn stream_tileset(mut stream: &TcpStream, filename: &str) {
    let path_1_0 = PATH_TILESET_DIR.to_string() + "/" + filename;

    if filename.contains("tileset.json") {
        if !Path::new(&path_1_0).exists() {
            println!("{} is not available locally. Fetching it", filename);
            let url = TILESET_URL.to_string() + filename + API_KEY;
            request_and_cache_tileset(&url, filename);
        }

        let status_line = "HTTP/1.1 200 OK";
        let contents = fs::read_to_string(&path_1_0).expect("Unable to read file");
        let length: usize = contents.len();
        let response = format!("{status_line}\r\nContent-Length: {length}\r\n\r\n{contents}");
    
        if let Err(e) = stream.write_all(response.as_bytes()) {
            println!("Error when streaming tileset: {}", e);
        }; 
    } else if filename.contains("model") { // Assume suffixless model is a b3dm 
        let filename_stemmed = Path::new(filename).file_stem().unwrap().to_str().unwrap();
        let path_glb = PATH_GLB_DIR.to_string() + "/" + filename_stemmed + ".glb";
        
        if !Path::new(&path_glb).exists() {
            if !Path::new(&path_1_0).exists() {
                println!("{} is not available locally. Fetching it", filename);
                let url = TILESET_URL.to_string() + filename + API_KEY;
                request_and_cache_binary_model_file(&url, filename);
            }   
            // Convert the model file to a glb file and return it
            if filename.contains("cmpt") { convert_cmpt_to_glb(filename_stemmed); } 
            else { convert_b3dm_to_glb(filename, filename_stemmed); }   
        }

        let contents = fs::read(path_glb).expect("Unable to read file");  //MIME type: model/gltf-binary or application/octet-stream
        let response = format!("HTTP/1.0 200 OK\r\nContent-Type: model/gltf-binary\r\nContent-Length: {}\r\n\r\n", contents.len());
        stream.write_all(response.as_bytes()).unwrap(); stream.write_all(&contents).unwrap(); stream.flush().unwrap(); 
    } else {
        println!("Unknown requested file: {}", filename);
    }
    println!("Sent {:#?} to Unity", filename);
}

/////// STREAM REQUEST FUNCTIONS ////////
fn request_and_cache_tileset(req_url: &str, file_name: &str) {
    // Send request to webatlas and parse response
    let mut res = reqwest::blocking::get(req_url).unwrap();
    let mut body = String::new();
    res.read_to_string(&mut body).unwrap();
    fs::write(format!("{}/{}", PATH_TILESET_DIR, file_name), &body).expect("Unable to write file");
}

fn request_and_cache_binary_model_file(req_url: &str, file_name: &str) {
    // Send request to webatlas and parse response
    let response = reqwest::blocking::get(req_url).unwrap();
    let path_str = PATH_TILESET_DIR.to_string() + "/" + file_name;
    let path = Path::new(&path_str);

    let mut file = match File::create(&path) {
        Err(why) => panic!("Couldn't create {}", why),
        Ok(file) => file,
    };

    let content =  response.bytes().unwrap();
    if let Err(e) = file.write_all(&content) {
        println!("Error when writing cmpt to file: {}", e);
    };
}

/////// CONVERSION FUNCTIONS ////////
fn convert_cmpt_to_glb(filename_stemmed: &str) {
    // npx 3d-tiles-tools cmptToGlb -i ./specs/data/composite.cmpt -o ./output/extracted.glb
    let cmd = format!("npx 3d-tiles-tools cmptToGlb -i {}/{}.cmpt -o {}/{}.glb", PATH_TILESET_DIR, &filename_stemmed, PATH_GLB_DIR, &filename_stemmed);
    let _ = if cfg!(target_os = "windows") {
        Command::new("cmd")
            .args(["/C", &cmd])
            .output()
            .expect("Error when upgrading tileset")
    } else {
        Command::new("sh")
            .arg("-c")
            .arg(&cmd)
            .output()
            .expect("Error when upgrading tileset")
    };
    println!("Converted {:#?} from cmpt to glb", filename_stemmed);
}

fn convert_b3dm_to_glb(filename: &str, filename_stemmed: &str) {
    // npx 3d-tiles-tools convertB3dmToGlb  -i ./specs/data/composite.cmpt -o ./output/extracted.glb
    let cmd = format!("npx 3d-tiles-tools convertB3dmToGlb -i {}/{} -o {}/{}.glb", PATH_TILESET_DIR, &filename, PATH_GLB_DIR, &filename_stemmed);
    let _ = if cfg!(target_os = "windows") {
        Command::new("cmd")
            .args(["/C", &cmd])
            .output()
            .expect("Error when upgrading tileset")
    } else {
        Command::new("sh")
            .arg("-c")
            .arg(&cmd)
            .output()
            .expect("Error when upgrading tileset")
    };
    println!("Converted {:#?} from b3dm to glb", filename_stemmed);
}
