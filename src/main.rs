use std::{
    fs,
    io::{prelude::*, BufReader, Read},
    net::{TcpListener, TcpStream},
    process::Command,
};

// use error_chain::error_chain;
// error_chain! {
//     foreign_links {
//         Io(std::io::Error);
//         HttpRequest(reqwest::Error);
//     }
// }

const NORKART_URL_FULL: &str = "https://waapi.webatlas.no/3d-tiles/tileserver.fcgi/tileset.json?api_key=DB124B20-9D21-4647-B65A-16C651553E48";
const NORKART_URL: &str = "https://waapi.webatlas.no/3d-tiles/tileserver.fcgi/tileset.json?api_key=";
const NORKART_API_KEY: &str = "DB124B20-9D21-4647-B65A-16C651553E48";

fn main() {
    let listener = TcpListener::bind("127.0.0.1:7878").unwrap();

    for stream in listener.incoming() {
        let stream = stream.unwrap();

        handle_connection(stream);
    }
}

fn handle_connection(mut stream: TcpStream) {
    // Make required directories if they don't already exits
    fs::create_dir_all("tmp/1_0").unwrap();
    fs::create_dir_all("tmp/1_1").unwrap();
    
    // Parse received request
    let buf_reader = BufReader::new(&mut stream);
    let http_request: Vec<_> = buf_reader
        .lines()
        .map(|result| result.unwrap())
        .take_while(|line| !line.is_empty())
        .collect();
    
    println!("Request from Unity: {:#?}", http_request);

    // Send request to webatlas and parse response
    let mut res = reqwest::blocking::get(NORKART_URL_FULL).unwrap();
    let mut body = String::new();
    res.read_to_string(&mut body).unwrap(); 
    fs::write("tmp/1_0/tileset.json", body).expect("Unable to write file");

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
    
    // Create response back to the CesiumForUnity plugin
    let status_line = "HTTP/1.1 200 OK";
    let contents = fs::read_to_string("tmp/1_1/tileset.json").expect("Unable to read file");
    let length = contents.len();

    let response = format!("{status_line}\r\nContent-Length: {length}\r\n\r\n{contents}");
    // println!("Response:\n{}", response);
    stream.write_all(response.as_bytes()).unwrap();


    // println!("Status: {}", res.status());
    // println!("Headers:\n{:#?}", res.headers());
    // println!("Body:\n{}", body);
    
    


    
}

// fn handle_connection(mut stream: TcpStream) {
//     let buf_reader = BufReader::new(&mut stream);
//     let request_line = buf_reader.lines().next().unwrap().unwrap();

//     let (status_line, filename) = if request_line == "GET / HTTP/1.1" {
//         ("HTTP/1.1 200 OK", "hello.html")
//     } else {
//         ("HTTP/1.1 404 NOT FOUND", "404.html")
//     };

//     let contents = fs::read_to_string(filename).unwrap();
//     let length = contents.len();

//     let response =
//         format!("{status_line}\r\nContent-Length: {length}\r\n\r\n{contents}");

//     stream.write_all(response.as_bytes()).unwrap();
// }

// fn handle_connection(mut stream: TcpStream) {
//     let buf_reader = BufReader::new(&mut stream);
//     let http_request: Vec<_> = buf_reader
//         .lines()
//         .map(|result| result.unwrap())
//         .take_while(|line| !line.is_empty())
//         .collect();

//     let status_line = "HTTP/1.1 200 OK";
//     let contents = fs::read_to_string("hello.html").unwrap();
//     let length = contents.len();

//     let response =
//         format!("{status_line}\r\nContent-Length: {length}\r\n\r\n{contents}");

//     stream.write_all(response.as_bytes()).unwrap();
// }

// Command::new("sh")
    //     .arg("-c")
    //     .arg("npx 3d-tiles-tools upgrade -f -i tmp/tileset0.json -o tmp/tileset1.json")
    //     .output()
    //     .expect("Error when upgrading tileset");

    // Command::new("npx 3d-tiles-tools upgrade")
    //     .arg("-f")
    //     .arg("-i")
    //     .arg("/Users/adr0x/Projects/3DTilesetConversionServer/tmp/tileset0.json")
    //     .arg("-o")
    //     .arg("/Users/adr0x/Projects/3DTilesetConversionServer/tmp/tileset1.json")
    //     .output()
    //     .expect("Error when upgrading tileset");