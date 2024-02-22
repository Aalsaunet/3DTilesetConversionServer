// use crate::Shutdown;

use std::{
    fs, future::Future, io::{prelude::*, Read}, path::Path, process::{Command, Stdio}, str::from_utf8, sync::Arc
};

use mini_redis::Connection;
use regex::Regex;
// use reqwest::blocking::Client;
use std::fs::File;
// use tileset_conversion_server::ThreadPool;
// use num_cpus;

// use mini_redis::server;
// use clap::Parser;
use tokio::{net::{TcpListener, TcpStream}, sync::{broadcast, mpsc, Semaphore}};
use tokio::signal;
use tokio::time::{self, Duration};

pub type Error = Box<dyn std::error::Error + Send + Sync>;
pub type Result<T> = std::result::Result<T, Error>;

const TILESERVER_URL: &str = "https://waapi.webatlas.no/3d-tiles/tileserver.fcgi/";
const API_KEY: &str = "?api_key=DB124B20-9D21-4647-B65A-16C651553E48";
const MAX_CONNECTIONS: usize = 250;

const PATH_TILESET_DIR: &str = "tmp/tilesets";
const PATH_B3DM_DIR: &str = "tmp/b3dms";
const PATH_GLB_DIR: &str = "tmp/glbs";

#[tokio::main]
pub async fn main() -> mini_redis::Result<()> {
    // Ensure the required directories exists
    fs::create_dir_all(PATH_TILESET_DIR).expect(format!("Couldn't create required dir {}", PATH_TILESET_DIR).as_str());
    fs::create_dir_all(PATH_B3DM_DIR).expect(format!("Couldn't create required dir {}", PATH_B3DM_DIR).as_str());
    fs::create_dir_all(PATH_GLB_DIR).expect(format!("Couldn't create required dir {}", PATH_GLB_DIR).as_str());

    // Bind a TCP listener
    let hostname = format!("{}:7878", get_hostname()); // e.g 192.168.1.2:7878
    let listener = TcpListener::bind(&hostname).await?;
    println!("Started 3DTiles Conversion Server listening on {}...", &hostname);

    run(listener, signal::ctrl_c()).await;
    
    Ok(())
}

#[derive(Debug)]
struct Listener {
    listener: TcpListener,
    limit_connections: Arc<Semaphore>,
    notify_shutdown: broadcast::Sender<()>,
    shutdown_complete_rx: mpsc::Receiver<()>,
    shutdown_complete_tx: mpsc::Sender<()>,
}

#[derive(Debug)]
struct Handler {
    connection: Connection,
    limit_connections: Arc<Semaphore>,
    shutdown: Shutdown,
    _shutdown_complete: mpsc::Sender<()>,
}

pub async fn run(listener: TcpListener, shutdown: impl Future) -> crate::Result<()> {
    let (notify_shutdown, _) = broadcast::channel(1);
    let (shutdown_complete_tx, shutdown_complete_rx) = mpsc::channel(1);
    
    let mut server = Listener {
        listener,
        limit_connections: Arc::new(Semaphore::new(MAX_CONNECTIONS)),
        notify_shutdown,
        shutdown_complete_tx,
        shutdown_complete_rx,
    };

    tokio::select! {
        res = server.run() => {
            if let Err(err) = res {
                println!("Failed to accept");
            }
        }
        _ = shutdown => {
            println!("Shutting down");
        }
    }

    let Listener {
        mut shutdown_complete_rx,
        shutdown_complete_tx,
        notify_shutdown,
        ..
    } = server;
    drop(notify_shutdown);
    drop(shutdown_complete_tx);

    let _ = shutdown_complete_rx.recv().await;

    Ok(())
}

impl Listener {
    async fn run(&mut self) -> crate::Result<()> {
        println!("Accepting inbound connections");

        loop {
            self.limit_connections.acquire().await.unwrap().forget();

            let socket = self.accept().await?;

            let mut handler = Handler {

                connection: Connection::new(socket),

                limit_connections: self.limit_connections.clone(),

                shutdown: Shutdown::new(self.notify_shutdown.subscribe()),

                _shutdown_complete: self.shutdown_complete_tx.clone(),
            };

            tokio::spawn(async move {
                if let Err(err) = handler.run().await {
                    println!("Connection error");
                }
            });
        }
    }

    async fn accept(&mut self) -> crate::Result<TcpStream> {
        let mut backoff = 1;

        loop {
            match self.listener.accept().await {
                Ok((socket, _)) => return Ok(socket),
                Err(err) => {
                    if backoff > 64 {
                        return Err(err.into());
                    }
                }
            }

            time::sleep(Duration::from_secs(backoff)).await;

            backoff *= 2;
        }
    }
}

impl Handler {
    async fn run(&mut self) -> crate::Result<()> {

        while !self.shutdown.is_shutdown() {

            let maybe_frame = tokio::select! {
                res = self.connection.read_frame() => res?,
                _ = self.shutdown.recv() => {
                    return Ok(());
                }
            };

            let frame = match maybe_frame {
                Some(frame) => frame,
                None => return Ok(()),
            };

            // SERVER SPECIFIC CODE HERE HERE
            let request_path = match  from_utf8(&frame) {
                Ok(v) => v,
                Err(e) => {println!("Failed to unwrap request from Unity: {:#?}", e); Err(e); },
            };

            let re = Regex::new(r"(?<tileset>[0-9]*tileset.json)|(?<model>[0-9]+model.cmpt|[0-9]+model.b3dm|[0-9]+model)").unwrap();
            match re.captures(request_path) {
                Some(caps) => {
                    if caps.name("tileset").is_some() {
                        self.stream_tileset(&caps["tileset"])
                    } 
                    else {
                        self.stream_model(&caps["model"])
                    }
                }
                None => self.not_found_response(),
            };
        }

        Ok(())
    }

    /////// RESPONSE FUNCTIONS ////////
    async fn stream_tileset(&mut self, filename: &str) {
        let tileset_path = PATH_TILESET_DIR.to_string() + "/" + filename;
        let contents: String = 
            if !Path::new(&tileset_path).exists() {
                println!("{} is not available locally. Fetching it.", filename);
                let url = TILESERVER_URL.to_string() + filename + API_KEY; 
                let Ok(c) = request_and_cache_tileset(client, &url, filename) else {
                    println!("Unable to fetch file {}", &tileset_path);
                    not_found_response(&stream);
                    return;
                }; 
                c
            } else {
                let Ok(c) = fs::read_to_string(&tileset_path) else {
                    println!("Unable to read file {}", &tileset_path);
                    not_found_response(&stream);
                    return;
                }; 
                c
            };

        let status_line = "HTTP/1.1 200 OK";
        let length: usize = contents.len();
        let response = format!("{status_line}\r\nContent-Length: {length}\r\n\r\n{contents}");

        if let Err(e) = stream.write_all(response.as_bytes()) {
            println!("Error when streaming tileset: {}", e);
            not_found_response(&stream);
        }; 

        if let Err(e) = stream.flush() {
            println!("Error when flushing: {}", e);
        };
        println!("Streamed tileset {:#?}", filename);
    }

    async fn stream_model(&mut self, filename: &str) {
        let filename_stemmed = Path::new(filename).file_stem().unwrap().to_str().unwrap();
        let path_b3dm = PATH_B3DM_DIR.to_string() + "/" + filename_stemmed + ".b3dm";
        let path_glb = PATH_GLB_DIR.to_string() + "/" + filename_stemmed + ".glb";
        if !Path::new(&path_glb).exists() {
            if !Path::new(&path_b3dm).exists() {
                println!("{} is not available locally. Fetching it.", filename);
                let url = TILESERVER_URL.to_string() + filename + API_KEY;
                let was_success = request_and_cache_binary_model_file(client, &url, &path_b3dm);
                if !was_success {
                    not_found_response(&stream);
                    return; 
                }
            }   
            // Convert the model file to a glb file and return it
            if filename.contains("cmpt") { convert_cmpt_to_glb(filename_stemmed); } 
            else { convert_b3dm_to_glb(filename, filename_stemmed); }   
        }

        //MIME type: model/gltf-binary or application/octet-stream
        let Ok(contents) = fs::read(&path_glb) else {
            println!("Unable to read file {}", &path_glb);
            not_found_response(&stream);
            return;
        };

        let response = format!("HTTP/1.0 200 OK\r\nContent-Type: model/gltf-binary\r\nContent-Length: {}\r\n\r\n", contents.len());
        
        if let Err(e) = stream.write_all(response.as_bytes()) {
            println!("Error when streaming model: {}", e); 
            not_found_response(&stream);
            return;
        }; 

        if let Err(e) = stream.write_all(&contents) {
            println!("Error when streaming model: {}", e); 
            self.not_found_response();
            return;
        };

        if let Err(_) = stream.flush() {
            println!("Error when flushing"); return;
        }; 
        
        println!("Streamed model {:#?}", filename);
    }

    async fn not_found_response(&mut self) {
        // let contents = fs::read_to_string(filename).unwrap();
        let response = format!(
            "{}\r\nContent-Length: {}\r\n\r\n{}",
            "HTTP/1.1 404 NOT FOUND",
            0,
            ""
        );

        self.connection.write_frame(response);
    }


    /////// STREAM REQUEST FUNCTIONS ////////
    async fn request_and_cache_tileset(&mut self, req_url: &str, file_name: &str) -> Result<String, String> {    
        let Ok(mut response) = client.get(req_url).send() else {
            return Err(format!("Failed to fetch from: {}", req_url));
        };

        let mut body = String::new();
        if let Err(e) = response.read_to_string(&mut body) {
            return Err(format!("Error when reading response to string: {}", e));
        };

        if let Err(e) = fs::write(format!("{}/{}", PATH_TILESET_DIR, file_name), &body) {
            return Err(format!("Error when writing tileset to file: {}", e));
        };

        return Ok(body);
    }

    async fn request_and_cache_binary_model_file(&mut self, req_url: &str, target_file_path: &str) -> bool {
        let Ok(response) = client.get(req_url).send() else {
            println!("Failed to fetch from: {}", req_url);
            return false;
        };

        let mut file = match File::create(Path::new(&target_file_path)) {
            Ok(file) => file,
            Err(_) => return false,
        };

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
}

impl Drop for Handler {
    fn drop(&mut self) {
        self.limit_connections.add_permits(1);
    }
}

// fn handle_connection(mut stream: TcpStream, client: Client) {
//     let mut buffer = [0; 1024];
//     if let Err(e) = stream.read(&mut buffer){
//         println!("Error when reading request header from stream: {}", e); return;
//     };

//     let request_path = match from_utf8(&buffer) {
//         Ok(v) => v,
//         Err(e) => {println!("Failed to unwrap request from Unity: {:#?}", e); return; },
//     };

//     let re = Regex::new(r"(?<tileset>[0-9]*tileset.json)|(?<model>[0-9]+model.cmpt|[0-9]+model.b3dm|[0-9]+model)").unwrap();
//     match re.captures(request_path) {
//         Some(caps) => {
//             if caps.name("tileset").is_some() {stream_tileset(&stream, &client, &caps["tileset"])}
//             else {stream_model(&stream, &client, &caps["model"])}
//         }
//         None => not_found_response(&stream),
//     };
// }




/////// COMMANDLINE FUNCTIONS ////////
fn get_hostname() -> String {
    let output = if cfg!(target_os = "windows") {
        Command::new("cmd")
            .args(["/C", "hostname -I"])
            .stdout(Stdio::piped())
            .output()
            .expect("Error when querying hostname")
    } else if cfg!(target_os = "macos") {
        Command::new("sh")
            .arg("-c")
            .arg("ipconfig getifaddr en0")
            .stdout(Stdio::piped())
            .output()
            .expect("Error when querying hostname")
    } else {
        Command::new("sh")
            .arg("-c")
            .arg("hostname -I")
            .stdout(Stdio::piped())
            .output()
            .expect("Error when querying hostname")
    };

    let result = String::from_utf8(output.stdout).expect("Error when querying hostname");
    return result.trim().to_string();
}

fn convert_cmpt_to_glb(filename_stemmed: &str) {
    // npx 3d-tiles-tools cmptToGlb -i ./specs/data/composite.cmpt -o ./output/extracted.glb
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
}

// fn main() {
//     // Ensure the required directories exists
//     fs::create_dir_all(PATH_TILESET_DIR).expect(format!("Couldn't create required dir {}", PATH_TILESET_DIR).as_str());
//     fs::create_dir_all(PATH_B3DM_DIR).expect(format!("Couldn't create required dir {}", PATH_B3DM_DIR).as_str());
//     fs::create_dir_all(PATH_GLB_DIR).expect(format!("Couldn't create required dir {}", PATH_GLB_DIR).as_str());
    
//     let thread_count = num_cpus::get() * THREADS_PER_CPU;
//     let pool = ThreadPool::new(thread_count);
//     let hostname = format!("{}:7878", get_hostname()); // e.g 192.168.1.2:7878
//     let listener = TcpListener::bind(&hostname).expect("Failed to bind TcpListener");
//     println!("Started 3DTiles Conversion Server with {} threads listening on {}...", thread_count, &hostname);

//     for stream in listener.incoming() {
//         let stream = stream.expect("Failed to unwrap TcpStream");
//         let client = reqwest::blocking::Client::new();
//         pool.execute(|| {
//             handle_connection(stream, client);
//         });
//     }
//     println!("Shutting down server.");
// }

#[derive(Debug)]
pub(crate) struct Shutdown {
    /// `true` if the shutdown signal has been received
    is_shutdown: bool,

    /// The receive half of the channel used to listen for shutdown.
    notify: broadcast::Receiver<()>,
}

impl Shutdown {
    /// Create a new `Shutdown` backed by the given `broadcast::Receiver`.
    pub(crate) fn new(notify: broadcast::Receiver<()>) -> Shutdown {
        Shutdown {
            is_shutdown: false,
            notify,
        }
    }

    /// Returns `true` if the shutdown signal has been received.
    pub(crate) fn is_shutdown(&self) -> bool {
        self.is_shutdown
    }

    /// Receive the shutdown notice, waiting if necessary.
    pub(crate) async fn recv(&mut self) {
        // If the shutdown signal has already been received, then return
        // immediately.
        if self.is_shutdown {
            return;
        }

        // Cannot receive a "lag error" as only one value is ever sent.
        let _ = self.notify.recv().await;

        // Remember that the signal has been received.
        self.is_shutdown = true;
    }
}