use std::{
    fs::File,
    io::{Read, Write},
    num::ParseIntError,
    process::Command,
};

// decode a hex into an array of integer values
pub fn decode_hex(s: &str) -> Result<Vec<u8>, ParseIntError> {
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16)).collect()
}

// encode a hex into a string
pub fn encode_hex(s: Vec<u8>) -> String {
    s.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn prettify_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        let kb = bytes / 1024;
        return format!("{kb} KB")
    } else if bytes < 1024 * 1024 * 1024 {
        let mb = bytes / (1024 * 1024);
        return format!("{mb} MB")
    } else {
        let gb = bytes / (1024 * 1024 * 1024);
        return format!("{gb} GB")
    }
}

pub fn write_file(_path: &String, contents: &String) -> Option<String> {
    let path = std::path::Path::new(_path);
    let prefix = path.parent().unwrap();
    match std::fs::create_dir_all(prefix) {
        Ok(_) => {}
        Err(_) => return None,
    }

    let mut file = match File::create(path) {
        Ok(file) => file,
        Err(_) => return None,
    };
    match file.write_all(contents.as_bytes()) {
        Ok(_) => {}
        Err(_) => return None,
    }

    Some(_path.to_string())
}

pub fn read_file(_path: &String) -> Option<String> {
    let path = std::path::Path::new(_path);
    let mut file = match File::open(path) {
        Ok(file) => file,
        Err(_) => return None,
    };
    let mut contents = String::new();
    match file.read_to_string(&mut contents) {
        Ok(_) => {}
        Err(_) => return None,
    }
    Some(contents)
}

pub fn delete_path(_path: &String) -> bool {
    let path = match std::path::Path::new(_path).to_str() {
        Some(path) => path,
        None => return false,
    };

    Command::new("rm").args(["-rf", path]).output().is_ok()
}
