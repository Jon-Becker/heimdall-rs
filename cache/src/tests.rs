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
        std::thread::sleep(std::time::Duration::from_secs(1));

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
