use std::{
    fs, io::{prelude::*, Read}, path::Path, process::Command
};

use regex::Regex;
use std::fs::File;
use rust_fetcher::ThreadPool;
use num_cpus;

const TILESET_URL_FULL: &str = "https://waapi.webatlas.no/3d-tiles/tileserver.fcgi/tileset.json?api_key=DB124B20-9D21-4647-B65A-16C651553E48";
const TILESERVER_URL: &str = "https://waapi.webatlas.no/3d-tiles/tileserver.fcgi/";
const API_KEY: &str = "?api_key=DB124B20-9D21-4647-B65A-16C651553E48";

const PATH_TILESET_DIR: &str = "tileset_cache/tilesets";
const PATH_B3DM_DIR: &str = "tileset_cache/b3dms";
const PATH_GLB_DIR: &str = "tileset_cache/glbs";

fn main() {
    // Ensure the required directories exists
    fs::create_dir_all(PATH_TILESET_DIR).expect(format!("Couldn't create required dir {}", PATH_TILESET_DIR).as_str());
    fs::create_dir_all(PATH_B3DM_DIR).expect(format!("Couldn't create required dir {}", PATH_B3DM_DIR).as_str());
    fs::create_dir_all(PATH_GLB_DIR).expect(format!("Couldn't create required dir {}", PATH_GLB_DIR).as_str());
    
    let thread_count = num_cpus::get();
    let thread_pool = ThreadPool::new(thread_count);
    let root_filename = "tileset.json";
    let Ok(root_body) = handle_tileset(root_filename.to_string()) else {
        println!("Unable to fetch file {}", root_filename);
        return;
    };

    // Fetch all referenced tilesets recursively 
    fetch_tileset_and_models_recursively(thread_pool, root_body);
    println!("Fetched all tilesets and referenced models");
}

/////// FETCH FUNCTIONS ////////
fn fetch_tileset_and_models_recursively(thread_pool: ThreadPool, body: String) {
    let reg_expr = Regex::new(r"(?<tileset>[0-9]*tileset.json)|(?<model>[0-9]+model.cmpt|[0-9]+model.b3dm|[0-9]+model)").unwrap();
    match reg_expr.captures(&body) {
        Some(caps) => {
            if caps.name("tileset").is_some() {
                thread_pool.execute(move || {
                    if let Ok(content) = handle_tileset(caps["tileset"].to_string()) {
                        fetch_tileset_and_models_recursively(thread_pool, content)
                    }
                }); 
            }
            else {
                thread_pool.execute(|| {
                    handle_model(&caps["model"])
                });    
            }
        }
        None => return,
    };
}

fn handle_tileset(filename: String) -> Result<String, String> {
    let tileset_path = PATH_TILESET_DIR.to_string() + "/" + &filename;
    if !Path::new(&tileset_path).exists() {
        println!("{} is not available locally. Fetching it.", filename);
        let url = TILESERVER_URL.to_string() + &filename + API_KEY; 
        return request_and_cache_tileset(&url, &filename);
    } else {
        let Ok(content) = fs::read_to_string(&tileset_path) else {
            return Err(format!("Unable to read file {}", &filename));
        }; 
        return Ok(content);
    };
}

fn request_and_cache_tileset(req_url: &str, file_name: &str) -> Result<String, String> {    
    let Ok(mut response) = reqwest::blocking::get(req_url) else {
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

fn handle_model(filename: &str) {
    let filename_stemmed = Path::new(filename).file_stem().unwrap().to_str().unwrap();
    let path_b3dm = PATH_B3DM_DIR.to_string() + "/" + filename_stemmed + ".b3dm";
    let path_glb = PATH_GLB_DIR.to_string() + "/" + filename_stemmed + ".glb";
    if !Path::new(&path_glb).exists() {
        if !Path::new(&path_b3dm).exists() {
            println!("{} is not available locally. Fetching it.", filename);
            let url = TILESERVER_URL.to_string() + filename + API_KEY;
            let was_success = request_and_cache_binary_model_file( &url, &path_b3dm);
            if !was_success {
                return; 
            }
        }   
        // Convert the model file to a glb file and return it
        if filename.contains("cmpt") { convert_cmpt_to_glb(filename_stemmed); } 
        else { convert_b3dm_to_glb(filename, filename_stemmed); }   
    }
}

fn request_and_cache_binary_model_file(req_url: &str, target_file_path: &str) -> bool {
    let Ok(response) = reqwest::blocking::get(req_url) else {
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

/////// CONVERTER FUNCTIONS ////////
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