use super::logging::Logger;

use std::{
    env,
    fs::File,
    io::{Read, Write},
    process::Command,
};

pub fn short_path(path: &str) -> String {
    let current_dir = match env::current_dir() {
        Ok(dir) => dir.into_os_string().into_string().unwrap(),
        Err(_) => std::process::exit(1),
    };
    path.replace(&current_dir, ".")
}

pub fn write_file(_path: &String, contents: &String) -> String {
    let path = std::path::Path::new(_path);
    let prefix = path.parent().unwrap();
    std::fs::create_dir_all(prefix).unwrap();

    let mut file = match File::create(path) {
        Ok(file) => file,
        Err(_) => {
            let (logger, _) = Logger::new("");
            logger.error(&format!("failed to create file \"{_path}\" ."));
            std::process::exit(1)
        }
    };
    match file.write_all(contents.as_bytes()) {
        Ok(_) => {}
        Err(_) => {
            let (logger, _) = Logger::new("");
            logger.error(&format!("failed to write to file \"{_path}\" ."));
            std::process::exit(1)
        }
    }

    _path.to_string()
}

pub fn write_lines_to_file(_path: &String, contents: Vec<String>) {
    write_file(_path, &contents.join("\n"));
}

pub fn read_file(_path: &String) -> String {
    let path = std::path::Path::new(_path);
    let mut file = match File::open(path) {
        Ok(file) => file,
        Err(_) => {
            let (logger, _) = Logger::new("");
            logger.error(&format!("failed to open file \"{_path}\" ."));
            std::process::exit(1)
        }
    };
    let mut contents = String::new();
    match file.read_to_string(&mut contents) {
        Ok(_) => {}
        Err(_) => {
            let (logger, _) = Logger::new("");
            logger.error(&format!("failed to read file \"{_path}\" ."));
            std::process::exit(1)
        }
    }
    contents
}

pub fn delete_path(_path: &String) -> bool {
    let path = std::path::Path::new(_path);
    Command::new("rm").args(["-rf", path.to_str().unwrap()]).output().is_ok()
}
