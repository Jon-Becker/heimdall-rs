use std::{
    env,
    fs::File,
    io::{Read, Write},
    path::Path,
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
pub fn delete_path(path: &str) -> bool {
    let path = Path::new(path);
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_dir() => std::fs::remove_dir_all(path).is_ok(),
        Ok(_) => std::fs::remove_file(path).is_ok(),
        Err(error) => error.kind() == std::io::ErrorKind::NotFound,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn files_can_be_written_read_and_deleted() {
        let directory = tempfile::tempdir().unwrap();
        let file = directory.path().join("nested/file.txt");
        let path = file.to_str().unwrap();
        write_file(path, "Hello, World!").unwrap();
        assert_eq!(read_file(path).unwrap(), "Hello, World!");
        assert!(delete_path(path));
        assert!(!file.exists());
        assert!(delete_path(path));
    }

    #[test]
    fn filesystem_errors_are_reported() {
        let directory = tempfile::tempdir().unwrap();
        let missing = directory.path().join("missing");
        assert!(read_file(missing.to_str().unwrap()).is_err());
        let parent_file = directory.path().join("parent");
        std::fs::write(&parent_file, "not a directory").unwrap();
        assert!(write_file(parent_file.join("child").to_str().unwrap(), "value").is_err());
    }

    #[test]
    fn deletes_directories_recursively() {
        let directory = tempfile::tempdir().unwrap();
        let nested = directory.path().join("nested");
        std::fs::create_dir(&nested).unwrap();
        std::fs::write(nested.join("file"), "value").unwrap();
        assert!(delete_path(nested.to_str().unwrap()));
        assert!(!nested.exists());
    }
}
