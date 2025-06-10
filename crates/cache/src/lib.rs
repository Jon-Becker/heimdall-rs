//! A simple cache system for heimdall-rs
//! Stores objects in ~/.bifrost/cache as bincode serialized files
//! Objects are stored with an expiry time, and are deleted if they are expired

use clap::Parser;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
#[allow(deprecated)]
use std::env::home_dir;

use error::Error;
use util::*;

pub mod error;
pub(crate) mod util;

/// Clap argument parser for the cache subcommand
#[derive(Debug, Clone, Parser)]
#[clap(
    about = "Manage heimdall-rs' cached objects",
    after_help = "For more information, read the wiki: https://jbecker.dev/r/heimdall-rs/wiki",
    override_usage = "heimdall cache <SUBCOMMAND>"
)]
pub struct CacheArgs {
    /// Cache subcommand
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
    /// Clear the cache, removing all objects
    #[clap(name = "clean", about = "Removes all cached objects in ~/.bifrost/cache")]
    Clean(NoArguments),

    /// List all cached objects
    #[clap(name = "ls", about = "Lists all cached objects in ~/.bifrost/cache")]
    Ls(NoArguments),

    /// Print the size of the cache in ~/.bifrost/cache
    #[clap(name = "size", about = "Prints the size of the cache in ~/.bifrost/cache")]
    Size(NoArguments),
}

/// A simple cache object that stores a value and an expiry time \
/// The expiry time is a unix timestamp
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Cache<T> {
    /// The value stored in the cache
    pub value: T,
    /// The expiry time of the cache object
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
/// assert!(keys("*").expect("!").contains(&"clear_cache_key".to_string()));
///
/// /// clear the cache
/// clear_cache();
///
/// /// assert that the cache no longer contains the key
/// assert!(!keys("*").expect("!").contains(&"clear_cache_key".to_string()));
/// ```
#[allow(deprecated)]
pub fn clear_cache() -> Result<(), Error> {
    let home = home_dir().ok_or_else(|| {
        Error::Generic(
            "failed to get home directory. does your os support `std::env::home_dir()`?"
                .to_string(),
        )
    })?;
    let cache_dir = home.join(".bifrost").join("cache");

    for entry in cache_dir
        .read_dir()
        .map_err(|e| Error::Generic(format!("failed to read cache directory: {e:?}")))?
    {
        let entry =
            entry.map_err(|e| Error::Generic(format!("failed to read cache entry: {e:?}")))?;
        delete_path(
            entry
                .path()
                .to_str()
                .ok_or_else(|| Error::Generic("failed to convert path to string".to_string()))?,
        );
    }

    Ok(())
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
/// assert!(exists("exists_key").expect("!"));
///
/// /// assert that the cache does not contain a non-existent key
/// assert!(!exists("non_existent_key").expect("!"));
/// ```
#[allow(deprecated)]
pub fn exists(key: &str) -> Result<bool, Error> {
    let home = home_dir().ok_or_else(|| {
        Error::Generic(
            "failed to get home directory. does your os support `std::env::home_dir()`?"
                .to_string(),
        )
    })?;
    let cache_dir = home.join(".bifrost").join("cache");
    let cache_file = cache_dir.join(format!("{key}.bin"));

    Ok(cache_file.exists())
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
/// assert!(keys("*").expect("!").contains(&"keys_key".to_string()));
///
/// /// assert that the cache does not contain a non-existent key
/// assert!(!keys("*").expect("!").contains(&"non_existent_key".to_string()));
///
/// /// assert that the cache contains the key
/// assert!(keys("keys_*").expect("!").contains(&"keys_key".to_string()));
/// ```
#[allow(deprecated)]
pub fn keys(pattern: &str) -> Result<Vec<String>, Error> {
    let home = home_dir().ok_or_else(|| {
        Error::Generic(
            "failed to get home directory. does your os support `std::env::home_dir()`?"
                .to_string(),
        )
    })?;
    let cache_dir = home.join(".bifrost").join("cache");
    let mut keys = Vec::new();

    // remove wildcard
    let pattern = pattern.replace('*', "");

    for entry in cache_dir
        .read_dir()
        .map_err(|e| Error::Generic(format!("failed to read cache directory: {e:?}")))?
    {
        let entry =
            entry.map_err(|e| Error::Generic(format!("failed to read cache entry: {e:?}")))?;
        let key = entry
            .path()
            .file_name()
            .ok_or_else(|| Error::Generic("failed to get file name".to_string()))?
            .to_str()
            .ok_or_else(|| Error::Generic("failed to convert path to string".to_string()))?
            .to_string();
        if pattern.is_empty() || key.contains(&pattern) {
            keys.push(key.replace(".bin", ""));
        }
    }

    // sort keys alphabetically
    keys.sort();

    Ok(keys)
}

/// Delete a cached object
/// ```
/// use heimdall_cache::{store_cache, delete_cache, keys};
///
/// /// add a value to the cache
/// store_cache("delete_cache_key", "value", None);
///
/// /// assert that the cache contains the key
/// assert!(keys("*").expect("!").contains(&"delete_cache_key".to_string()));
///
/// /// delete the cached object
/// delete_cache("delete_cache_key");
///
/// /// assert that the cache does not contain the key
/// assert!(!keys("*").expect("!").contains(&"delete_cache_key".to_string()));
/// ```
#[allow(deprecated)]
pub fn delete_cache(key: &str) -> Result<(), Error> {
    let home = home_dir().ok_or_else(|| {
        Error::Generic(
            "failed to get home directory. does your os support `std::env::home_dir()`?"
                .to_string(),
        )
    })?;
    let cache_dir = home.join(".bifrost").join("cache");
    let cache_file = cache_dir.join(format!("{key}.bin"));

    if cache_file.exists() {
        std::fs::remove_file(cache_file)
            .map_err(|e| Error::Generic(format!("failed to delete cache file: {e:?}")))?;
    }

    Ok(())
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
/// assert_eq!(read_cache::<String>("read_cache_key").expect("!").expect("!"), "value");
/// ```
#[allow(deprecated)]
pub fn read_cache<T>(key: &str) -> Result<Option<T>, Error>
where
    T: 'static + DeserializeOwned, {
    let home = home_dir().ok_or_else(|| {
        Error::Generic(
            "failed to get home directory. does your os support `std::env::home_dir()`?"
                .to_string(),
        )
    })?;
    let cache_dir = home.join(".bifrost").join("cache");
    let cache_file = cache_dir.join(format!("{key}.bin"));

    let binary_string = match read_file(
        cache_file
            .to_str()
            .ok_or_else(|| Error::Generic("failed to convert path to string".to_string()))?,
    ) {
        Ok(s) => s,
        Err(_) => return Ok(None),
    };

    let binary_vec = decode_hex(&binary_string)
        .map_err(|e| Error::Generic(format!("failed to decode hex: {e:?}")))?;

    let cache: Cache<T> = bincode::deserialize::<Cache<T>>(&binary_vec)
        .map_err(|e| Error::Generic(format!("failed to deserialize cache object: {e:?}")))?;

    // check if the cache has expired, if so, delete it and return None
    if cache.expiry <
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| Error::Generic(format!("failed to get current time: {e:?}")))?
            .as_secs()
    {
        delete_cache(key)?;
        return Ok(None);
    }

    Ok(Some(*Box::new(cache.value)))
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
pub fn store_cache<T>(key: &str, value: T, expiry: Option<u64>) -> Result<(), Error>
where
    T: Serialize, {
    let home = home_dir().ok_or_else(|| {
        Error::Generic(
            "failed to get home directory. does your os support `std::env::home_dir()`?"
                .to_string(),
        )
    })?;
    let cache_dir = home.join(".bifrost").join("cache");
    let cache_file = cache_dir.join(format!("{key}.bin"));

    // expire in 90 days
    let expiry = expiry.unwrap_or(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| Error::Generic(format!("failed to get current time: {e:?}")))?
            .as_secs() +
            60 * 60 * 24 * 90,
    );

    let cache = Cache { value, expiry };
    let encoded: Vec<u8> = bincode::serialize(&cache)
        .map_err(|e| Error::Generic(format!("failed to serialize cache object: {e:?}")))?;
    let binary_string = encode_hex(encoded);
    write_file(
        cache_file
            .to_str()
            .ok_or_else(|| Error::Generic("failed to convert path to string".to_string()))?,
        &binary_string,
    )?;

    Ok(())
}

/// Takes in an &str and an async function that returns a Result<T, E> where T is ser/de
/// and E is an error type. \
/// If the key exists in the cache, it will return the value, otherwise it will call the function
/// and store the result in the cache, returning the value.
pub async fn with_cache<T, F, Fut>(key: &str, func: F) -> eyre::Result<T>
where
    T: 'static + Serialize + DeserializeOwned + Send + Sync,
    F: FnOnce() -> Fut + Send,
    Fut: std::future::Future<Output = Result<T, eyre::Report>> + Send, {
    // Try to read from cache
    match read_cache::<T>(key) {
        Ok(Some(cached_value)) => {
            tracing::debug!("cache hit for key: '{}'", key);
            Ok(cached_value)
        }
        Ok(None) | Err(_) => {
            tracing::debug!("cache miss for key: '{}'", key);

            // If cache read fails or returns None, execute the function
            let value = func().await?;

            // Store the result in the cache
            store_cache(key, &value, None)?;

            Ok(value)
        }
    }
}

/// Cache subcommand handler
#[allow(deprecated)]
pub fn cache(args: CacheArgs) -> Result<(), Error> {
    match args.sub {
        Subcommands::Clean(_) => {
            clear_cache()?;
            println!("Cache cleared.")
        }
        Subcommands::Ls(_) => {
            let keys = keys("*")?;
            println!("Displaying {} cached objects:", keys.len());

            for (i, key) in keys.iter().enumerate() {
                println!("{i:>5} : {key}");
            }
        }
        Subcommands::Size(_) => {
            let home = home_dir().ok_or_else(|| {
                Error::Generic(
                    "failed to get home directory. does your os support `std::env::home_dir()`?"
                        .to_string(),
                )
            })?;
            let cache_dir = home.join(".bifrost").join("cache");
            let mut size = 0;

            for entry in cache_dir
                .read_dir()
                .map_err(|e| Error::Generic(format!("failed to read cache directory: {e:?}")))?
            {
                let entry = entry
                    .map_err(|e| Error::Generic(format!("failed to read cache entry: {e:?}")))?;
                let path = entry.path();
                let metadata = std::fs::metadata(path)
                    .map_err(|e| Error::Generic(format!("failed to get metadata: {e:?}")))?;
                size += metadata.len();
            }

            println!("Cached objects: {}", keys("*")?.len());
            println!("Cache size: {}", prettify_bytes(size));
        }
    }

    Ok(())
}

#[allow(deprecated)]
#[allow(unused_must_use)]
#[cfg(test)]
mod tests {
    use crate::{delete_cache, exists, keys, read_cache, store_cache};
    use serde::{Deserialize, Serialize};
    use std::env::home_dir;

    #[test]
    fn test_store_cache() {
        store_cache("key", "value".to_string(), None);

        // assert cached file exists
        let home = home_dir().expect("failed to get home_dir");
        let cache_dir = home.join(".bifrost").join("cache");
        let cache_file = cache_dir.join("key.bin");
        assert!(cache_file.exists());
    }

    #[test]
    fn test_get_cache() {
        store_cache("key3", "value".to_string(), None);
        let value = read_cache("key3");
        let value: String = value.expect("failed to get cache").expect("failed to get cache");

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
        let home = home_dir().expect("failed to get home_dir");
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
        let value: TestStruct = value.expect("failed to get cache").expect("failed to get cache");

        // assert stored value matches
        assert_eq!(value.name, "test");
        assert_eq!(value.age, 1);
    }

    #[test]
    fn test_keys() {
        store_cache("some_key", "some_value", None);
        store_cache("some_other_key", "some_value", None);
        store_cache("not_a_key", "some_value", None);

        assert_eq!(keys("some_").expect("failed to get keys"), vec!["some_key", "some_other_key"]);
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
            .all(|key| { keys("*").expect("failed to get keys").contains(&key.to_string()) }));
    }

    #[test]
    fn test_exists() {
        assert!(!exists("does_not_exist").expect("failed to check if key exists"));
        store_cache("does_not_exist", "some_value", None);
        assert!(exists("does_not_exist").expect("failed to check if key exists"));
        delete_cache("does_not_exist");
    }
}
