use clap::{AppSettings, Parser};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
#[allow(deprecated)]
use std::env::home_dir;
use std::{collections::HashMap, num::ParseIntError};

use util::*;

pub mod util;

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

/// Clap subcommand parser for the cache subcommand
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

    #[clap(name = "export", about = "Exports all cached objects in ~/.bifrost/cache to a file")]
    Export(ExportArgs),

    #[clap(name = "import", about = "Imports cached objects from a file into ~/.bifrost/cache")]
    Import(ImportArgs),
}

/// Clap argument parser for the export subcommand
#[derive(Debug, Clone, Parser)]
#[clap(
    after_help = "For more information, read the wiki: https://jbecker.dev/r/heimdall-rs/wiki",
    global_setting = AppSettings::DeriveDisplayOrder,
    override_usage = "heimdall cache export"
)]
pub struct ExportArgs {
    /// The path to export the cache to
    #[clap(short, long, default_value = "./cache-export.bin")]
    pub output: String,
}

/// Clap argument parser for the import subcommand
#[derive(Debug, Clone, Parser)]
#[clap(
    after_help = "For more information, read the wiki: https://jbecker.dev/r/heimdall-rs/wiki",
    global_setting = AppSettings::DeriveDisplayOrder,
    override_usage = "heimdall cache import"
)]
pub struct ImportArgs {
    /// The path to the binary file to import the cache from
    #[clap(short, long, default_value = "./cache-export.bin")]
    pub input: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Cache<T> {
    pub value: T,
    pub expiry: u64,
}

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

#[allow(deprecated)]
pub fn exists(key: &str) -> bool {
    let home = home_dir().unwrap();
    let cache_dir = home.join(".bifrost").join("cache");
    let cache_file = cache_dir.join(format!("{key}.bin"));

    cache_file.exists()
}

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

#[allow(deprecated)]
pub fn delete_cache(key: &str) {
    let home = home_dir().unwrap();
    let cache_dir = home.join(".bifrost").join("cache");
    let cache_file = cache_dir.join(format!("{key}.bin"));

    if cache_file.exists() {
        std::fs::remove_file(cache_file).unwrap();
    }
}

#[allow(deprecated)]
pub fn check_expiry<T>() -> bool
where
    T: DeserializeOwned, {
    let home = home_dir().unwrap();
    let cache_dir = home.join(".bifrost").join("cache");

    for entry in cache_dir.read_dir().unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        let binary_string = match read_file(path.to_str().unwrap()) {
            Some(s) => s,
            None => return false,
        };

        let binary_vec = decode_hex(&binary_string);
        if binary_vec.is_err() {
            return false
        }

        let cache: Result<Cache<T>, _> = bincode::deserialize(&binary_vec.unwrap());
        if cache.is_err() {
            delete_path(path.to_str().unwrap());
        };

        let cache = cache.unwrap();
        if cache.expiry <
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs()
        {
            // delete file
            delete_path(path.to_str().unwrap());
        }
    }
    true
}

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

    let cache: Cache<T> = match bincode::deserialize(&binary_vec.unwrap()) {
        Ok(c) => c,
        Err(_) => return None,
    };
    Some(*Box::new(cache.value))
}

#[allow(deprecated)]
pub fn read_cache_raw(key: &str) -> Result<Vec<u8>, ParseIntError> {
    let home = home_dir().unwrap();
    let cache_dir = home.join(".bifrost").join("cache");
    let cache_file = cache_dir.join(format!("{key}.bin"));

    let binary_string = match read_file(cache_file.to_str().unwrap()) {
        Some(s) => s,
        None => return Ok(vec![]),
    };

    decode_hex(&binary_string)
}

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
        Subcommands::Export(args) => {
            println!("Beginning cache export");

            // get all keys as a hashmap of cache keys to bincode values
            let k_v_bin_map: HashMap<String, Vec<u8>> = keys("*")
                .iter()
                .map(|key| {
                    let value = read_cache_raw(key).unwrap();
                    (key.to_string(), value)
                })
                .collect();

            // serialize the hashmap
            println!("Serializing {} cached objects", k_v_bin_map.len());
            let encoded: Vec<u8> = bincode::serialize(&k_v_bin_map).unwrap();
            let binary_string = encode_hex(encoded);
            write_file(&args.output, &binary_string);

            println!("Cache exported to {}", args.output);
        }
        Subcommands::Import(args) => {
            let home = home_dir().unwrap();
            let cache_dir = home.join(".bifrost").join("cache");

            println!("Beginning cache import");

            // read the file
            let binary_string = match read_file(&args.input) {
                Some(s) => s,
                None => {
                    println!("Failed to read file {}", args.input);
                    return Ok(())
                }
            };
            let binary_obj = decode_hex(&binary_string)?;
            let k_v_bin_map: HashMap<String, Vec<u8>> = bincode::deserialize(&binary_obj)?;
            println!("Deserialized {} cached objects", k_v_bin_map.len());

            // write each key-value pair to the cache
            k_v_bin_map.iter().for_each(|(key, value)| {
                let binary_string = encode_hex(value.to_vec());
                write_file(cache_dir.join(format!("{key}.bin")).to_str().unwrap(), &binary_string);
            });

            println!("Cache imported from {}", args.input);
        }
    }

    Ok(())
}

#[allow(deprecated)]
#[cfg(test)]
mod tests {
    use crate::{
        check_expiry, delete_cache, exists, keys, read_cache, read_cache_raw, store_cache,
        util::encode_hex,
    };
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
    fn test_read_cache_raw() {
        store_cache("read_cache_raw_1", "value".to_string(), None);
        let value = read_cache_raw("read_cache_raw_1").unwrap();
        let stringified = encode_hex(value);

        // assert stored value matches
        assert!(stringified.contains("76616c7565"));
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
