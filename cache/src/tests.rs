#[allow(deprecated)]
#[cfg(test)]
mod tests {
    use crate::{check_expiry, clear_cache, delete_cache, exists, keys, read_cache, store_cache};
    use serde::{Deserialize, Serialize};
    use std::env::home_dir;

    #[test]
    fn test_store_cache() {
        store_cache("key", "value".to_string(), None);

        // assert cached file exists
        let home = home_dir().unwrap();
        let cache_dir = home.join(".bifrost").join("cache");
        let cache_file = cache_dir.join("key.bin");
        assert!(cache_file.exists());
    }

    #[test]
    fn test_get_cache() {
        store_cache("key3", "value".to_string(), None);
        let value = read_cache("key3");
        let value: String = value.unwrap();

        // assert stored value matches
        assert_eq!(value, "value");
    }

    #[test]
    fn test_store_struct() {
        #[derive(Serialize, Deserialize, Debug)]
        struct TestStruct {
            name: String,
            age: u8,
        }

        let test_struct = TestStruct { name: "test".to_string(), age: 1 };

        store_cache("struct", test_struct, None);

        // assert cached file exists
        let home = home_dir().unwrap();
        let cache_dir = home.join(".bifrost").join("cache");
        let cache_file = cache_dir.join("struct.bin");
        assert!(cache_file.exists());
    }

    #[test]
    fn test_get_struct() {
        #[derive(Serialize, Deserialize, Debug)]
        struct TestStruct {
            name: String,
            age: u8,
        }

        let test_struct = TestStruct { name: "test".to_string(), age: 1 };

        store_cache("struct2", test_struct, None);
        let value = read_cache("struct2");
        let value: TestStruct = value.unwrap();

        // assert stored value matches
        assert_eq!(value.name, "test");
        assert_eq!(value.age, 1);
    }

    #[test]
    fn test_expiry() {
        store_cache("dead", "value".to_string(), Some(0));

        // assert cached file exists
        let home = home_dir().unwrap();
        let cache_dir = home.join(".bifrost").join("cache");
        let cache_file = cache_dir.join("dead.bin");
        assert!(cache_file.exists());

        // wait for expiry
        std::thread::sleep(std::time::Duration::from_secs(2));

        // check expiry
        check_expiry::<String>();

        assert!(!cache_file.exists());
    }

    #[test]
    fn test_keys() {
        store_cache("some_key", "some_value", None);
        store_cache("some_other_key", "some_value", None);
        store_cache("not_a_key", "some_value", None);

        assert_eq!(keys("some_"), vec!["some_key", "some_other_key"]);
    }

    #[test]
    fn test_keys_wildcard() {
        store_cache("a", "some_value", None);
        store_cache("b", "some_value", None);
        store_cache("c", "some_value", None);
        store_cache("d", "some_value", None);
        store_cache("e", "some_value", None);
        store_cache("f", "some_value", None);

        assert!(vec!["a", "b", "c", "d", "e", "f"]
            .iter()
            .all(|key| { keys("*").contains(&key.to_string()) }));
    }

    #[test]
    fn test_clear_cache() {
        store_cache("a2", "some_value", None);
        store_cache("b2", "some_value", None);
        store_cache("c2", "some_value", None);
        store_cache("d2", "some_value", None);
        store_cache("e2", "some_value", None);
        store_cache("f2", "some_value", None);

        assert!(keys("*").len() >= 6);

        clear_cache();

        assert_eq!(keys("a2").len(), 0);
    }

    #[test]
    fn test_exists() {
        assert!(!exists("does_not_exist"));
        store_cache("does_not_exist", "some_value", None);
        assert!(exists("does_not_exist"));
        delete_cache("does_not_exist");
    }
}

#[cfg(test)]
mod test_util {
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
        let result = write_file(&path.to_string(), &contents.to_string());
        assert_eq!(result, Some(path.to_string()));
    }

    #[test]
    fn test_write_file_failure() {
        // Assuming the path is read-only or permission denied
        let path = "/root/test.txt";
        let contents = "Hello, World!";
        let result = write_file(&path.to_string(), &contents.to_string());
        assert_eq!(result, None);
    }

    #[test]
    fn test_read_file_successful() {
        let path = "/tmp/test.txt";
        let contents = "Hello, World!";
        write_file(&path.to_string(), &contents.to_string());

        let result = read_file(&path.to_string());
        assert!(result.is_some());
    }

    #[test]
    fn test_read_file_failure() {
        let path = "/nonexistent/test.txt";
        let result = read_file(&path.to_string());
        assert_eq!(result, None);
    }

    #[test]
    fn test_delete_path_successful() {
        let path = "/tmp/test_dir";
        std::fs::create_dir(&path).unwrap();

        let result = delete_path(&path.to_string());
        assert!(result);
    }

    #[test]
    fn test_delete_path_failure() {
        let path = "/nonexistent/test_dir";
        let result = delete_path(&path.to_string());
        assert!(result);
    }
}
