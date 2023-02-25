// decode a hex into an array of integer values
pub fn decode_hex(s: &str) -> Result<Vec<u8>, ParseIntError> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16))
        .collect()
}


// encode a hex into a string
pub fn encode_hex(s: Vec<u8>) -> String {
    s.iter()
        .map(|b| format!("{:02x}", b))
        .collect()
}

use std::{
    fs::File,
    io::{Write, Read}, process::Command, num::ParseIntError
};


pub fn write_file(_path: &String, contents: &String) -> Option<String> {
    let path = std::path::Path::new(_path);
    let prefix = path.parent().unwrap();
    std::fs::create_dir_all(prefix).unwrap();
    
    let mut file = match File::create(path) {
        Ok(file) => file,
        Err(_) => {
            return None
        }
    };
    match file.write_all(contents.as_bytes()) {
        Ok(_) => {},
        Err(_) => {
            return None
        }
    }

    return Some(_path.to_string());
}

pub fn read_file(_path: &String) -> Option<String> {
    let path = std::path::Path::new(_path);
    let mut file = match File::open(path) {
        Ok(file) => file,
        Err(_) => {
            return None
        }
    };
    let mut contents = String::new();
    match file.read_to_string(&mut contents) {
        Ok(_) => {},
        Err(_) => {
            return None
        }
    }
    return Some(contents);
}

pub fn delete_path(_path: &String) -> bool {
    let path = std::path::Path::new(_path);
    match Command::new("rm")
        .args(&["-rf", &path.to_str().unwrap()])
        .output()
    {
        Ok(_) => {
            return true;
        },
        Err(_) => {
            return false;
        },
    }
}