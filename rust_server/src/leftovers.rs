
// type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
// use error_chain::error_chain;
// error_chain! {
//     foreign_links {
//         Io(std::io::Error);
//         HttpRequest(reqwest::Error);
//     }
// }

// Parse received request from Unity
// let buf_reader = BufReader::new(&mut stream);
// let http_request: Vec<_> = buf_reader
//     .lines()
//     .map(|result| result.expect("Failed to unwrap http_request result"))
//     .take_while(|line| !line.is_empty())
//     .collect();

// let Some(request_path) = http_request.first() else {
//     println!("Failed to unwrap request from Unity: {:#?}", http_request);
//     return;
// };

// Request tilesets from remote server
// fetch_all_tilesets();

// Convert from 3DTiles-1.0 to 3DTiles-1.1
// convert_all_tilesets();



// fn convert_glb_to_b3dm(filename_stemmed: &str) {
//     // npx 3d-tiles-tools glbToB3dm -i ./specs/data/CesiumTexturedBox/CesiumTexturedBox.glb -o ./output/CesiumTexturedBox.b3dm
//     let cmd = format!("npx 3d-tiles-tools glbToB3dm -i {}/{}.glb -o {}/{}.b3dm", PATH_GLB, &filename_stemmed, PATH_B3DM, &filename_stemmed);
//     let _ = if cfg!(target_os = "windows") {
//         Command::new("cmd")
//             .args(["/C", &cmd])
//             .output()
//             .expect("Error when upgrading tileset")
//     } else {
//         Command::new("sh")
//             .arg("-c")
//             .arg(&cmd)
//             .output()
//             .expect("Error when upgrading tileset")
//     };
//     println!("Converted {:#?} from glb to b3dm", filename_stemmed);
// }

// fn optimize_b3dm(filename_stemmed: &str) {
//     // npx 3d-tiles-tools optimizeB3dm -i ./specs/data/Textured/batchedTextured.b3dm -o ./output/optimized.b3dm --options --draco.compressMeshes --draco.compressionLevel=9
//     let cmd = format!("npx 3d-tiles-tools optimizeB3dm -i {}/{}.b3dm -o {}/{}.b3dm  
//                                 --options --draco.compressMeshes --draco.compressionLevel=9"
//                                 , PATH_1_0, &filename_stemmed, PATH_B3DM, &filename_stemmed);
//     let _ = if cfg!(target_os = "windows") {
//         Command::new("cmd")
//             .args(["/C", &cmd])
//             .output()
//             .expect("Error when upgrading tileset")
//     } else {
//         Command::new("sh")
//             .arg("-c")
//             .arg(&cmd)
//             .output()
//             .expect("Error when upgrading tileset")
//     };
//     println!("Converted {:#?} from glb to b3dm", filename_stemmed);
// }

    // let Some(caps) = re.captures(request_path) else {
    //     println!("No match found for request!");
    //     return;
    // };

    // stream_tileset(&stream, &caps["match"]);

    // fn stream_tileset(mut stream: &TcpStream, filename: &str) {
    //     let path_1_0 = PATH_TILESET_DIR.to_string() + "/" + filename;
    
    //     if filename.contains("tileset.json") {
    //         if !Path::new(&path_1_0).exists() {
    //             println!("{} is not available locally. Fetching it", filename);
    //             let url = TILESET_URL.to_string() + filename + API_KEY;
    //             request_and_cache_tileset(&url, filename);
    //         }
    
    //         let status_line = "HTTP/1.1 200 OK";
    //         let contents = fs::read_to_string(&path_1_0).expect("Unable to read file");
    //         let length: usize = contents.len();
    //         let response = format!("{status_line}\r\nContent-Length: {length}\r\n\r\n{contents}");
        
    //         if let Err(e) = stream.write_all(response.as_bytes()) {
    //             println!("Error when streaming tileset: {}", e);
    //         }; 
    //     } else if filename.contains("model") { // Assume suffixless model is a b3dm 
    //         let filename_stemmed = Path::new(filename).file_stem().unwrap().to_str().unwrap();
    //         let path_glb = PATH_GLB_DIR.to_string() + "/" + filename_stemmed + ".glb";
            
    //         if !Path::new(&path_glb).exists() {
    //             if !Path::new(&path_1_0).exists() {
    //                 println!("{} is not available locally. Fetching it", filename);
    //                 let url = TILESET_URL.to_string() + filename + API_KEY;
    //                 request_and_cache_binary_model_file(&url, filename);
    //             }   
    //             // Convert the model file to a glb file and return it
    //             if filename.contains("cmpt") { convert_cmpt_to_glb(filename_stemmed); } 
    //             else { convert_b3dm_to_glb(filename, filename_stemmed); }   
    //         }
    
    //         let contents = fs::read(path_glb).expect("Unable to read file");  //MIME type: model/gltf-binary or application/octet-stream
    //         let response = format!("HTTP/1.0 200 OK\r\nContent-Type: model/gltf-binary\r\nContent-Length: {}\r\n\r\n", contents.len());
    //         stream.write_all(response.as_bytes()).unwrap(); stream.write_all(&contents).unwrap(); stream.flush().unwrap(); 
    //     } else {
    //         println!("Unknown requested file: {}", filename);
    //     }
    //     println!("Sent {:#?} to Unity", filename);
    // }


/////// DOWNLOAD EVERYTHING FUNCTIONS ////////
// const TILESET_URL_FULL: &str = "https://waapi.webatlas.no/3d-tiles/tileserver.fcgi/tileset.json?api_key=DB124B20-9D21-4647-B65A-16C651553E48";
// fn fetch_all_tilesets() {
//     let result = request_tileset(TILESET_URL_FULL);
//     fs::write(PATH_1_0.to_string() + "/tileset.json", &result).expect("Unable to write file");

//     // Fetch all referenced tilesets recursively
//     fetch_child_tilesets(result);
//     println!("Fetched all 3DTiles-1.0 tilesets");
// }

// fn fetch_child_tilesets(result: String) {
//     let re = Regex::new(r"(?<match>[0-9]*tileset.json|[0-9]+model.cmpt|[0-9]+model.b3dm|[0-9]+model)").unwrap();
//     let matches: Vec<_> = re.find_iter(&result).map(|m| m.as_str()).collect();
//     for m in matches.iter() {
//         let path = PATH_1_0.to_string() + "/" + m;
//         if !Path::new(&path).exists() {
//             println!("Sending request for {}", m);
//             let url = TILESET_URL.to_string() + m + API_KEY;
//             if m.contains("cmpt") || m.contains("b3dm") || m.contains("model") {
//                 request_and_cache_binary_model_file(&url, m);
//                 continue; 
//             }

//             let result = request_tileset(&url); 
//             fs::write(format!("{}/{}", PATH_1_0, m), &result).expect("Unable to write file");
//             fetch_child_tilesets(result);
//         } else {
//             println!("{} already cached locally.", m);
//             if m.contains("cmpt") || m.contains("b3dm") || m.contains("model") { continue; }
//             let result: String = fs::read_to_string(path).expect("Unable to read file");
//             fetch_child_tilesets(result);
//         }    
//     }
// }

// fn request_tileset(req_url: &str) -> String {
//     // Send request to webatlas and parse response
//     let mut res = reqwest::blocking::get(req_url).unwrap();
//     let mut body = String::new();
//     res.read_to_string(&mut body).unwrap();
//     return body;
// }