use clap::{AppSettings, Parser};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
#[allow(deprecated)]
use std::env::home_dir;

use util::*;

pub mod util;

/// Clap argument parser for the cache subcommand
#[derive(Debug, Clone, Parser)]
#[clap(
    about = "Manage heimdall-rs' cached objects",
    after_help = "For more information, read the wiki: https://jbecker.dev/r/heimdall-rs/wiki",
    global_setting = AppSettings::DeriveDisplayOrder,
    override_usage = "heimdall cache <SUBCOMMAND>"
)]
pub struct CacheArgs {
    #[clap(subcommand)]
    pub sub: Subcommands,
}

/// A simple clap subcommand with no arguments
#[derive(Debug, Clone, Parser)]
pub struct NoArguments {}

/// Clap subcommand parser for cache subcommands
#[derive(Debug, Clone, Parser)]
#[clap(
    about = "Manage heimdall-rs' cached objects",
    after_help = "For more information, read the wiki: https://jbecker.dev/r/heimdall-rs/wiki"
)]
#[allow(clippy::large_enum_variant)]
pub enum Subcommands {
    #[clap(name = "clean", about = "Removes all cached objects in ~/.bifrost/cache")]
    Clean(NoArguments),

    #[clap(name = "ls", about = "Lists all cached objects in ~/.bifrost/cache")]
    Ls(NoArguments),

    #[clap(name = "size", about = "Prints the size of the cache in ~/.bifrost/cache")]
    Size(NoArguments),
}

/// A simple cache object that stores a value and an expiry time \
/// The expiry time is a unix timestamp
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Cache<T> {
    pub value: T,
    pub expiry: u64,
}

/// Clear the cache, removing all objects
///
/// ```
/// use heimdall_cache::{clear_cache, store_cache, keys};
///
/// /// add a value to the cache
/// store_cache("clear_cache_key", "value", None);
///
/// /// assert that the cache contains the key
/// assert!(keys("*").contains(&"clear_cache_key".to_string()));
///
/// /// clear the cache
/// clear_cache();
///
/// /// assert that the cache no longer contains the key
/// assert!(!keys("*").contains(&"clear_cache_key".to_string()));
/// ```
#[allow(deprecated)]
pub fn clear_cache() {
    let home = home_dir().unwrap();
    let cache_dir = home.join(".bifrost").join("cache");

    for entry in cache_dir.read_dir().unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        delete_path(path.to_str().unwrap());
    }
}

/// Check if a cached object exists
///
/// ```
/// use heimdall_cache::{store_cache, exists};
///
/// /// add a value to the cache
/// store_cache("exists_key", "value", None);
///
/// /// assert that the cache contains the key
/// assert!(exists("exists_key"));
///
/// /// assert that the cache does not contain a non-existent key
/// assert!(!exists("non_existent_key"));
/// ```
#[allow(deprecated)]
pub fn exists(key: &str) -> bool {
    let home = home_dir().unwrap();
    let cache_dir = home.join(".bifrost").join("cache");
    let cache_file = cache_dir.join(format!("{key}.bin"));

    cache_file.exists()
}

/// List all cached objects
///
/// ```
/// use heimdall_cache::{store_cache, keys};
///
/// /// add a value to the cache
/// store_cache("keys_key", "value", None);
///
/// /// assert that the cache contains the key
/// assert!(keys("*").contains(&"keys_key".to_string()));
///
/// /// assert that the cache does not contain a non-existent key
/// assert!(!keys("*").contains(&"non_existent_key".to_string()));
///
/// /// assert that the cache contains the key
/// assert!(keys("keys_*").contains(&"keys_key".to_string()));
/// ```
#[allow(deprecated)]
pub fn keys(pattern: &str) -> Vec<String> {
    let home = home_dir().unwrap();
    let cache_dir = home.join(".bifrost").join("cache");
    let mut keys = Vec::new();

    // remove wildcard
    let pattern = pattern.replace('*', "");

    for entry in cache_dir.read_dir().unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        let key = path.file_name().unwrap().to_str().unwrap().to_string();
        if pattern.is_empty() || key.contains(&pattern) {
            keys.push(key.replace(".bin", ""));
        }
    }

    // sort keys alphabetically
    keys.sort();

    keys
}

/// Delete a cached object
/// ```
/// use heimdall_cache::{store_cache, delete_cache, keys};
///
/// /// add a value to the cache
/// store_cache("delete_cache_key", "value", None);
///
/// /// assert that the cache contains the key
/// assert!(keys("*").contains(&"delete_cache_key".to_string()));
///
/// /// delete the cached object
/// delete_cache("delete_cache_key");
///
/// /// assert that the cache does not contain the key
/// assert!(!keys("*").contains(&"delete_cache_key".to_string()));
/// ```
#[allow(deprecated)]
pub fn delete_cache(key: &str) {
    let home = home_dir().unwrap();
    let cache_dir = home.join(".bifrost").join("cache");
    let cache_file = cache_dir.join(format!("{key}.bin"));

    if cache_file.exists() {
        std::fs::remove_file(cache_file).unwrap();
    }
}

/// Read a cached object
///
/// ```
/// use heimdall_cache::{store_cache, read_cache};
///
/// /// add a value to the cache
/// store_cache("read_cache_key", "value", None);
///
/// /// read the cached object
/// assert_eq!(read_cache::<String>("read_cache_key").unwrap(), "value");
/// ```
#[allow(deprecated)]
pub fn read_cache<T>(key: &str) -> Option<T>
where
    T: 'static + DeserializeOwned, {
    let home = home_dir().unwrap();
    let cache_dir = home.join(".bifrost").join("cache");
    let cache_file = cache_dir.join(format!("{key}.bin"));

    let binary_string = match read_file(cache_file.to_str().unwrap()) {
        Some(s) => s,
        None => return None,
    };

    let binary_vec = decode_hex(&binary_string);

    if binary_vec.is_err() {
        return None
    }

    let cache: Cache<T> = match bincode::deserialize::<Cache<T>>(&binary_vec.unwrap()) {
        Ok(c) => {
            // check if the cache has expired, if so, delete it and return None
            if c.expiry <
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
            {
                delete_cache(key);
                return None
            }

            c
        }
        Err(_) => return None,
    };
    Some(*Box::new(cache.value))
}

/// Store a value in the cache, with an optional expiry time \
/// If no expiry time is specified, the object will expire in 90 days
///
/// ```
/// use heimdall_cache::{store_cache, read_cache};
///
/// /// add a value to the cache with no expiry time (90 days)
/// store_cache("store_cache_key", "value", None);
///
/// /// add a value to the cache with an expiry time of 1 day
/// store_cache("store_cache_key2", "value", Some(60 * 60 * 24));
/// ```
#[allow(deprecated)]
pub fn store_cache<T>(key: &str, value: T, expiry: Option<u64>)
where
    T: Serialize, {
    let home = home_dir().unwrap();
    let cache_dir = home.join(".bifrost").join("cache");
    let cache_file = cache_dir.join(format!("{key}.bin"));

    // expire in 90 days
    let expiry = expiry.unwrap_or(
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() +
            60 * 60 * 24 * 90,
    );

    let cache = Cache { value: value, expiry: expiry };
    let encoded: Vec<u8> = bincode::serialize(&cache).unwrap();
    let binary_string = encode_hex(encoded);
    write_file(cache_file.to_str().unwrap(), &binary_string);
}

/// Cache subcommand handler
#[allow(deprecated)]
pub fn cache(args: CacheArgs) -> Result<(), Box<dyn std::error::Error>> {
    match args.sub {
        Subcommands::Clean(_) => {
            clear_cache();
            println!("Cache cleared.")
        }
        Subcommands::Ls(_) => {
            let keys = keys("*");
            println!("Displaying {} cached objects:", keys.len());

            for (i, key) in keys.iter().enumerate() {
                println!("{i:>5} : {key}");
            }
        }
        Subcommands::Size(_) => {
            let home = home_dir().unwrap();
            let cache_dir = home.join(".bifrost").join("cache");
            let mut size = 0;

            for entry in cache_dir.read_dir().unwrap() {
                let entry = entry.unwrap();
                let path = entry.path();
                let metadata = std::fs::metadata(path).unwrap();
                size += metadata.len();
            }

            println!("Cached objects: {}", keys("*").len());
            println!("Cache size: {}", prettify_bytes(size));
        }
    }

    Ok(())
}

#[allow(deprecated)]
#[cfg(test)]
mod tests {
    use crate::{delete_cache, exists, keys, read_cache, store_cache};
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

        assert!(["a", "b", "c", "d", "e", "f"]
            .iter()
            .all(|key| { keys("*").contains(&key.to_string()) }));
    }

    #[test]
    fn test_exists() {
        assert!(!exists("does_not_exist"));
        store_cache("does_not_exist", "some_value", None);
        assert!(exists("does_not_exist"));
        delete_cache("does_not_exist");
    }
}
