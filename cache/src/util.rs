use std::{
    fmt::Write as FmtWrite,
    fs::File,
    io::{Read, Write},
    num::ParseIntError,
    process::Command,
};

/// Decode a hex string into a bytearray
///
/// ```
/// use heimdall_cache::util::decode_hex;
///
/// let hex = "48656c6c6f20576f726c64"; // "Hello World" in hex
/// let result = decode_hex(hex);
/// assert_eq!(result, Ok(vec![72, 101, 108, 108, 111, 32, 87, 111, 114, 108, 100]));
/// ```
pub fn decode_hex(s: &str) -> Result<Vec<u8>, ParseIntError> {
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16)).collect()
}

/// Encode a bytearray into a hex string
///
/// ```
/// use heimdall_cache::util::encode_hex;
///
/// let bytes = vec![72, 101, 108, 108, 111, 32, 87, 111, 114, 108, 100];
/// let result = encode_hex(bytes);
/// assert_eq!(result, "48656c6c6f20576f726c64");
/// ```
pub fn encode_hex(s: Vec<u8>) -> String {
    s.iter().fold(String::new(), |mut acc, b| {
        write!(acc, "{b:02x}", b = b).unwrap();
        acc
    })
}

/// Prettify bytes into a human-readable format \
/// e.g. 1024 -> 1 KB
///
/// ```
/// use heimdall_cache::util::prettify_bytes;
///
/// let bytes = 500;
/// let result = prettify_bytes(bytes);
/// assert_eq!(result, "500 B");
///
/// let bytes = 500_000;
/// let result = prettify_bytes(bytes);
/// assert_eq!(result, "488 KB");
/// ```
pub fn prettify_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        let kb = bytes / 1024;
        return format!("{kb} KB");
    } else if bytes < 1024 * 1024 * 1024 {
        let mb = bytes / (1024 * 1024);
        return format!("{mb} MB");
    } else {
        let gb = bytes / (1024 * 1024 * 1024);
        return format!("{gb} GB");
    }
}

/// Write contents to a file on the disc
///
/// ```no_run
/// use heimdall_cache::util::write_file;
///
/// let path = "/tmp/test.txt";
/// let contents = "Hello, World!";
/// let result = write_file(path, contents);
/// ```
pub fn write_file(_path: &str, contents: &str) -> Option<String> {
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

/// Read contents from a file on the disc
///
/// ```no_run
/// use heimdall_cache::util::read_file;
///
/// let path = "/tmp/test.txt";
/// let contents = read_file(path);
/// assert!(contents.is_some());
/// ```
pub fn read_file(_path: &str) -> Option<String> {
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

/// Delete a file or directory on the disc
///
/// ```no_run
/// use heimdall_cache::util::delete_path;
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
    use crate::util::*;

    #[test]
    fn test_decode_hex_valid_hex() {
        let hex = "48656c6c6f20576f726c64"; // "Hello World" in hex
        let result = decode_hex(hex);
        assert_eq!(result, Ok(vec![72, 101, 108, 108, 111, 32, 87, 111, 114, 108, 100]));
    }

    #[test]
    fn test_decode_hex_invalid_hex() {
        let hex = "48656c6c6f20576f726c4G"; // Invalid hex character 'G'
        let result = decode_hex(hex);
        assert!(result.is_err());
    }

    #[test]
    fn test_encode_hex() {
        let bytes = vec![72, 101, 108, 108, 111, 32, 87, 111, 114, 108, 100];
        let result = encode_hex(bytes);
        assert_eq!(result, "48656c6c6f20576f726c64");
    }

    #[test]
    fn test_prettify_bytes_less_than_1_kb() {
        let bytes = 500;
        let result = prettify_bytes(bytes);
        assert_eq!(result, "500 B");
    }

    #[test]
    fn test_prettify_bytes_less_than_1_mb() {
        let bytes = 500_000;
        let result = prettify_bytes(bytes);
        assert_eq!(result, "488 KB");
    }

    #[test]
    fn test_prettify_bytes_less_than_1_gb() {
        let bytes = 500_000_000;
        let result = prettify_bytes(bytes);
        assert_eq!(result, "476 MB");
    }

    #[test]
    fn test_prettify_bytes_greater_than_1_gb() {
        let bytes = 5_000_000_000;
        let result = prettify_bytes(bytes);
        assert_eq!(result, "4 GB");
    }

    #[test]
    fn test_write_file_successful() {
        let path = "/tmp/test.txt";
        let contents = "Hello, World!";
        let result = write_file(path, contents);
        assert_eq!(result, Some(path.to_string()));
    }

    #[test]
    fn test_write_file_failure() {
        // Assuming the path is read-only or permission denied
        let path = "/root/test.txt";
        let contents = "Hello, World!";
        let result = write_file(path, contents);
        assert_eq!(result, None);
    }

    #[test]
    fn test_read_file_successful() {
        let path = "/tmp/test.txt";
        let contents = "Hello, World!";
        write_file(path, contents);

        let result = read_file(path);
        assert!(result.is_some());
    }

    #[test]
    fn test_read_file_failure() {
        let path = "/nonexistent/test.txt";
        let result = read_file(path);
        assert_eq!(result, None);
    }

    #[test]
    fn test_delete_path_successful() {
        let path = "/tmp/test_dir";
        std::fs::create_dir(path).unwrap();

        let result = delete_path(path);
        assert!(result);
    }

    #[test]
    fn test_delete_path_failure() {
        let path = "/nonexistent/test_dir";
        let result = delete_path(path);
        assert!(result);
    }
}
