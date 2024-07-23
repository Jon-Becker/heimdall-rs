use std::{
    env,
    fs::File,
    io::{Read, Write},
    path::Path,
    process::Command,
};

use eyre::Result;

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
        Ok(dir) => path.replace(&dir.into_os_string().into_string().unwrap_or(String::new()), "."),
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
pub fn write_file(path_str: &str, contents: &str) -> Result<()> {
    let path = Path::new(path_str);

    // Create the directory if it doesn't exist
    std::fs::create_dir_all(
        path.parent().ok_or_else(|| eyre::eyre!("unable to create directory"))?,
    )?;

    let mut file = File::create(path)?;
    file.write_all(contents.as_bytes())?;

    Ok(())
}

/// Read contents from a file on the disc
///
/// ```no_run
/// use heimdall_common::utils::io::file::read_file;
///
/// let path = "/tmp/test.txt";
/// let contents = read_file(path);
/// ```
pub fn read_file(path: &str) -> Result<String> {
    let path = Path::new(path);
    let mut file = File::open(path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    Ok(contents)
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
    let path = match std::path::Path::new(_path).to_str() {
        Some(path) => path,
        None => return false,
    };

    Command::new("rm").args(["-rf", path]).output().is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_write_file_successful() {
        let path = "/tmp/test2.txt";
        let contents = "Hello, World!";
        let result = write_file(path, contents);
        assert!(result.is_ok());
    }

    #[test]
    fn test_write_file_failure() {
        // Assuming the path is read-only or permission denied
        let path = "/root/test2.txt";
        let contents = "Hello, World!";
        let result = write_file(path, contents);
        assert!(result.is_err());
    }

    #[test]
    fn test_read_file_successful() {
        let path = "/tmp/test2.txt";
        let contents = "Hello, World!";
        write_file(path, contents).expect("unable to write file");

        let result = read_file(path);
        assert!(result.is_ok());
    }

    #[test]
    fn test_read_file_failure() {
        let path = "/nonexistent/test2.txt";
        let result = read_file(path);
        assert!(result.is_err());
    }

    #[test]
    fn test_delete_path_successful() {
        let path = "/tmp/test_dir2";
        std::fs::create_dir(path).expect("unable to create directory");

        let result = delete_path(path);
        assert!(result);
    }

    #[test]
    fn test_delete_path_failure() {
        let path = "/nonexistent/test_dir2";
        let result = delete_path(path);
        assert!(result);
    }
}
