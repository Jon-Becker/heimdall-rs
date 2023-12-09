use super::logging::Logger;

use std::{
    env,
    fs::File,
    io::{Read, Write},
    process::Command,
};

/// Convert a long path to a short path.
///
/// ```no_run
/// use heimdall_common::utils::io::file::short_path;
///
/// let path = "/some/long/path/that/is/cwd/something.json";
/// let short_path = short_path(path);
/// assert_eq!(short_path, "./something.json");
/// ```
pub fn short_path(path: &str) -> String {
    match env::current_dir() {
        Ok(dir) => path.replace(&dir.into_os_string().into_string().unwrap(), "."),
        Err(_) => path.to_owned(),
    }
}

/// Write contents to a file on the disc
///
/// ```no_run
/// use heimdall_common::utils::io::file::write_file;
///
/// let path = "/tmp/test.txt";
/// let contents = "Hello, World!";
/// let result = write_file(path, contents);
/// ```
pub fn write_file(_path: &str, contents: &str) -> String {
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

/// Write contents to a file on the disc
///
/// ```no_run
/// use heimdall_common::utils::io::file::write_lines_to_file;
///
/// let path = "/tmp/test.txt";
/// let contents = vec![String::from("Hello"), String::from("World!")];
/// let result = write_lines_to_file(path, contents);
/// ```
pub fn write_lines_to_file(_path: &str, contents: Vec<String>) {
    write_file(_path, &contents.join("\n"));
}

/// Read contents from a file on the disc
///
/// ```no_run
/// use heimdall_common::utils::io::file::read_file;
///
/// let path = "/tmp/test.txt";
/// let contents = read_file(path);
/// ```
pub fn read_file(_path: &str) -> String {
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

/// Delete a file from the disc
///
/// ```no_run
/// use heimdall_common::utils::io::file::delete_path;
///
/// let path = "/tmp/test.txt";
/// let result = delete_path(path);
/// ```
pub fn delete_path(_path: &str) -> bool {
    let path = std::path::Path::new(_path);
    Command::new("rm").args(["-rf", path.to_str().unwrap()]).output().is_ok()
}
