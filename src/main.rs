use std::{
    fs,
    io::{prelude::*, BufReader, Read},
    net::{TcpListener, TcpStream},
    process::Command,
    path::Path
};
// use std::io::Cursor;
use regex::Regex;
use std::fs::File;


// type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
// use error_chain::error_chain;
// error_chain! {
//     foreign_links {
//         Io(std::io::Error);
//         HttpRequest(reqwest::Error);
//     }
// }

const NORKART_URL_FULL: &str = "https://waapi.webatlas.no/3d-tiles/tileserver.fcgi/tileset.json?api_key=DB124B20-9D21-4647-B65A-16C651553E48";
const NORKART_URL: &str = "https://waapi.webatlas.no/3d-tiles/tileserver.fcgi/";
const NORKART_API_KEY: &str = "?api_key=DB124B20-9D21-4647-B65A-16C651553E48";

fn main() {
    // Make required directories if they don't already exits
    fs::create_dir_all("tmp/1_0").unwrap();
    fs::create_dir_all("tmp/1_1").unwrap();
    fs::create_dir_all("tmp/1_2").unwrap();
    let mut is_first_request = true;
    
    let listener = TcpListener::bind("127.0.0.1:7878").unwrap();
    for stream in listener.incoming() {
        let stream = stream.unwrap();
        handle_connection(stream, is_first_request);
        is_first_request = false;
    }
}

fn handle_connection(mut stream: TcpStream, is_first_request: bool) {
    // Parse received request
    let buf_reader = BufReader::new(&mut stream);
    let http_request: Vec<_> = buf_reader
        .lines()
        .map(|result| result.unwrap())
        .take_while(|line| !line.is_empty())
        .collect();

    println!("Request from Unity: {:#?}", http_request.first().unwrap());

    // Request root tileset
    fetch_all_tilesets();

    // Convert from 3DTiles-1.0 to 3DTiles-1.1
    // convert_all_tilesets();
    
    // Create response back to the CesiumForUnity plugin
    if is_first_request {
        stream_tileset(&stream, "tileset.json");
        return;
    }

    let request_path = http_request.first().unwrap();
    let re = Regex::new(r"(?<ts>[0-9]+tileset.json|[0-9]+model.cmpt)").unwrap();
    let Some(caps) = re.captures(request_path) else {
        // stream_tileset(&stream, "tileset.json");
        println!("No match!");
        return;
    };

    stream_tileset(&stream, &caps["ts"]);
    // stream_all_tilesets(&stream);
}

/////// REQUEST FUNCTIONS ////////
fn fetch_all_tilesets() {
    let result = request_tileset(NORKART_URL_FULL);
    fs::write("tmp/1_0/tileset.json", &result).expect("Unable to write file");

    // Find all references tilesets
    fetch_child_tilesets(result);
    println!("Fetched all 3DTiles-1.0 tilesets");
}

fn fetch_child_tilesets(result: String) {
    let re = Regex::new(r"([0-9]+tileset.json|[0-9]+model.cmpt)").unwrap();
    let matches: Vec<_> = re.find_iter(&result).map(|m| m.as_str()).collect();
    for m in matches.iter() {
        let path = "tmp/1_0/".to_string() + m;
        if !Path::new(&path).exists() {
            println!("Sending request for {}", m);
            let url = NORKART_URL.to_string() + m + NORKART_API_KEY;
            if m.contains("cmpt") {
                request_cmpt(&url, m);
                continue; 
            }

            let result = request_tileset(&url); 
            fs::write(format!("tmp/1_0/{}", m), &result).expect("Unable to write file");
            fetch_child_tilesets(result);
        } else {
            println!("{} already cached locally.", m);
            if m.contains("cmpt") { continue; }
            let result: String = fs::read_to_string(path).expect("Unable to read file");
            fetch_child_tilesets(result);
        }    
    }
}

fn request_tileset(req_url: &str) -> String {
    // Send request to webatlas and parse response
    let mut res = reqwest::blocking::get(req_url).unwrap();
    let mut body = String::new();
    res.read_to_string(&mut body).unwrap();
    return body;
}

fn request_cmpt(req_url: &str, file_name: &str) {
    // Send request to webatlas and parse response
    let response = reqwest::blocking::get(req_url).unwrap();
    let path_str = "tmp/1_0/".to_string() + file_name;
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

/////// RESPONSE FUNCTIONS ////////
fn stream_tileset(mut stream: &TcpStream, tileset: &str) {
    let status_line = "HTTP/1.1 200 OK";
    let contents = fs::read_to_string(format!("tmp/1_0/{}", tileset)).expect("Unable to read file");
    let length: usize = contents.len();
    let response = format!("{status_line}\r\nContent-Length: {length}\r\n\r\n{contents}");

    if let Err(e) = stream.write_all(response.as_bytes()) {
        println!("Error when streaming tileset: {}", e);
    };
    println!("Sent {:#?} to Unity", tileset);
}

/////// CONVERSION FUNCTIONS ////////
fn convert_all_tilesets() {
    let _ = if cfg!(target_os = "windows") {
        Command::new("cmd")
            .args(["/C", "npx 3d-tiles-tools upgrade -f -i tmp/1_0/tileset.json -o tmp/1_1/tileset.json"])
            .output()
            .expect("Error when upgrading tileset")
    } else {
        Command::new("sh")
            .arg("-c")
            .arg("npx 3d-tiles-tools upgrade -f -i tmp/1_0/tileset.json -o tmp/1_1/tileset.json")
            .output()
            .expect("Error when upgrading tileset")
    };
    println!("Converted all 3DTiles-1.0 tilesets into the 1.1 format");
}

// fn stream_all_tilesets(mut stream: &TcpStream) {
//     let status_line = "HTTP/1.1 200 OK";
//     let contents = fs::read_to_string("tmp/1_2/tileset.json").expect("Unable to read file");
//     let length: usize = contents.len();
//     let response = format!("{status_line}\r\nContent-Length: {length}\r\n\r\n{contents}");

//     if let Err(e) = stream.write_all(response.as_bytes()) {
//         println!("Error: {}", e);
//     };
//     // stream_child_tilesets(stream, contents);
// }

// fn stream_child_tilesets(mut stream: &TcpStream, parent_content: String) {
//     let re = Regex::new(r"([0-9]+tileset.json)").unwrap();
//     let matches: Vec<_> = re.find_iter(&parent_content).map(|m| m.as_str()).collect();
//     for m in matches.iter() {
//         let status_line = "HTTP/1.1 200 OK";
//         let child_content = fs::read_to_string(format!("tmp/1_2/{}", m)).expect("Unable to read file");
//         let length: usize = child_content.len();
//         let response = format!("{status_line}\r\nContent-Length: {length}\r\n\r\n{child_content}");

//         if let Err(e) = stream.write_all(response.as_bytes()) {
//             println!("Error: {}", e);
//         };

//         stream_child_tilesets(stream, child_content);
//     }
// }

