use super::logging::Logger;

use std::{
    fs::File,
    io::{Write, Read}
};


pub fn write_file(_path: &String, contents: &String) -> String {
    let path = std::path::Path::new(_path);
    let prefix = path.parent().unwrap();
    std::fs::create_dir_all(prefix).unwrap();
    
    let mut file = match File::create(path) {
        Ok(file) => file,
        Err(_) => {
            let logger = Logger::new("");
            logger.error(&format!("failed to create file \"{}\" .", _path).to_string());
            std::process::exit(1)
        }
    };
    match file.write_all(contents.as_bytes()) {
        Ok(_) => {},
        Err(_) => {
            let logger = Logger::new("");
            logger.error(&format!("failed to write to file \"{}\" .", _path).to_string());
            std::process::exit(1)
        }
    }

    return _path.to_string();
}


pub fn read_file(_path: &String) -> String {
    let path = std::path::Path::new(_path);
    let mut file = match File::open(path) {
        Ok(file) => file,
        Err(_) => {
            let logger = Logger::new("");
            logger.error(&format!("failed to open file \"{}\" .", _path).to_string());
            std::process::exit(1)
        }
    };
    let mut contents = String::new();
    match file.read_to_string(&mut contents) {
        Ok(_) => {},
        Err(_) => {
            let logger = Logger::new("");
            logger.error(&format!("failed to read file \"{}\" .", _path).to_string());
            std::process::exit(1)
        }
    }
    return contents;
}