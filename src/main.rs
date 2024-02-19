use std::{
    fs, io::{prelude::*, BufReader, Read}, net::{TcpListener, TcpStream}, path::Path, process::Command,
};

use regex::Regex;
use std::fs::File;
use tileset_conversion_server::ThreadPool;
use num_cpus;

const TILESERVER_URL: &str = "https://waapi.webatlas.no/3d-tiles/tileserver.fcgi/";
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

    for stream in listener.incoming() {
        let stream = stream.expect("Failed to unwrap TcpStream");
        pool.execute(|| {
            handle_connection(stream);
        });
    }
    println!("Shutting down server.");
}

fn handle_connection(mut stream: TcpStream) {
    // Parse received request from Unity
    let buf_reader = BufReader::new(&mut stream);
    let http_request: Vec<_> = buf_reader
        .lines()
        .map(|result| result.expect("Failed to unwrap http_request result"))
        .take_while(|line| !line.is_empty())
        .collect();
    
    let Some(request_path) = http_request.first() else {
        println!("Failed to unwrap request from Unity: {:#?}", http_request);
        return;
    };

    let re = Regex::new(r"(?<tileset>[0-9]*tileset.json)|(?<model>[0-9]+model.cmpt|[0-9]+model.b3dm|[0-9]+model)").unwrap();
    match re.captures(request_path) {
        Some(caps) => {
            if caps.name("tileset").is_some() {stream_tileset(&stream, &caps["tileset"])}
            else {stream_model(&stream, &caps["model"])}
        }
        None => return,
    };
}

/////// RESPONSE FUNCTIONS ////////
fn stream_tileset(mut stream: &TcpStream, filename: &str) {
    let tileset_path = PATH_TILESET_DIR.to_string() + "/" + filename;
    if !Path::new(&tileset_path).exists() {
        //println!("{} is not available locally. Fetching it", filename);
        let url = TILESERVER_URL.to_string() + filename + API_KEY;
        let was_success = request_and_cache_tileset(&url, filename);
        if !was_success { return; }
    }

    let Ok(contents) = fs::read_to_string(&tileset_path) else {
        println!("Unable to read file {}", &tileset_path);
        return;
    };

    let status_line = "HTTP/1.1 200 OK";
    let length: usize = contents.len();
    let response = format!("{status_line}\r\nContent-Length: {length}\r\n\r\n{contents}");

    if let Err(e) = stream.write_all(response.as_bytes()) {
        println!("Error when streaming tileset: {}", e);
    }; 
    //println!("Sent {:#?} to Unity", filename);
}

fn stream_model(mut stream: &TcpStream, filename: &str) {
    let filename_stemmed = Path::new(filename).file_stem().unwrap().to_str().unwrap();
    let path_b3dm = PATH_B3DM_DIR.to_string() + "/" + filename_stemmed + ".b3dm";
    let path_glb = PATH_GLB_DIR.to_string() + "/" + filename_stemmed + ".glb";
    if !Path::new(&path_glb).exists() {
        if !Path::new(&path_b3dm).exists() {
            //println!("{} is not available locally. Fetching it", filename);
            let url = TILESERVER_URL.to_string() + filename + API_KEY;
            let was_success = request_and_cache_binary_model_file(&url, &path_b3dm);
            if !was_success { return; }
        }   
        // Convert the model file to a glb file and return it
        if filename.contains("cmpt") { convert_cmpt_to_glb(filename_stemmed); } 
        else { convert_b3dm_to_glb(filename, filename_stemmed); }   
    }

    //MIME type: model/gltf-binary or application/octet-stream
    let Ok(contents) = fs::read(&path_glb) else {
        println!("Unable to read file {}", &path_glb);
        return;
    };

    let response = format!("HTTP/1.0 200 OK\r\nContent-Type: model/gltf-binary\r\nContent-Length: {}\r\n\r\n", contents.len());
    
    if let Err(e) = stream.write_all(response.as_bytes()) {
        println!("Error when streaming model: {}", e); return;
    }; 

    if let Err(e) = stream.write_all(&contents) {
        println!("Error when streaming model: {}", e); return;
    };

    if let Err(_) = stream.flush() {
        println!("Error when flushing"); return;
    }; 
    
    //println!("Sent {:#?} to Unity", filename);
}

/////// STREAM REQUEST FUNCTIONS ////////
fn request_and_cache_tileset(req_url: &str, file_name: &str) -> bool {
    // Send request to webatlas and parse response
    let Ok(mut response) = reqwest::blocking::get(req_url) else {
        println!("Failed to fetch from: {}", req_url);
        return false;
    };

    let mut body = String::new();
    if let Err(e) = response.read_to_string(&mut body) {
        println!("Error when reading response to string: {}", e);
        return false;
    };

    if let Err(e) = fs::write(format!("{}/{}", PATH_TILESET_DIR, file_name), &body) {
        println!("Error when writing tileset to file: {}", e);
        return false;
    };

    return true;
}

fn request_and_cache_binary_model_file(req_url: &str, target_file_path: &str) -> bool {
    // Send request to webatlas and parse response
    let Ok(response) = reqwest::blocking::get(req_url) else {
        println!("Failed to fetch from: {}", req_url);
        return false;
    };

    let mut file = match File::create(Path::new(&target_file_path)) {
        Ok(file) => file,
        Err(_) => return false, //panic!("Couldn't create {}", why),
    };

    // let content =  response.bytes().expect("Failed to unwrap bytes from the response");
    let Ok(content) = response.bytes() else {
        println!("Failed to unwrap bytes from the response:");
        return false;
    };

    if let Err(e) = file.write_all(&content) {
        println!("Error when writing model to file: {}", e);
        return false;
    };

    return true;
}

/////// CONVERSION FUNCTIONS ////////
fn convert_cmpt_to_glb(filename_stemmed: &str) {
    // npx 3d-tiles-tools cmptToGlb -i ./specs/data/composite.cmpt -o ./output/extracted.glb
    // let cmd = format!("npx 3d-tiles-tools cmptToGlb -i {}/{}.cmpt -o {}/{}.glb", PATH_TILESET_DIR, &filename_stemmed, PATH_GLB_DIR, &filename_stemmed);
    let cmd = format!("npx 3d-tiles-tools cmptToGlb -i {}/{}.b3dm -o {}/{}.glb", PATH_B3DM_DIR, &filename_stemmed, PATH_GLB_DIR, &filename_stemmed);
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
    // println!("Converted {:#?} from cmpt to glb", filename_stemmed);
}

fn convert_b3dm_to_glb(filename: &str, filename_stemmed: &str) {
    // npx 3d-tiles-tools convertB3dmToGlb  -i ./specs/data/composite.cmpt -o ./output/extracted.glb
    let cmd = format!("npx 3d-tiles-tools convertB3dmToGlb -i {}/{} -o {}/{}.glb", PATH_B3DM_DIR, &filename, PATH_GLB_DIR, &filename_stemmed);
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
    // println!("Converted {:#?} from b3dm to glb", filename_stemmed);
}
