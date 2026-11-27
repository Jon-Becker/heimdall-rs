//! A simple cache system for heimdall-rs
//! Stores objects in ~/.bifrost/cache as bincode serialized files
//! Objects are stored with an expiry time, and are deleted if they are expired

use clap::Parser;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
#[allow(deprecated)]
use std::env::home_dir;
use std::path::PathBuf;

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

/// Filesystem-backed cache. The root is explicit so each operation uses the same location.
#[derive(Debug)]
struct CacheStore {
    root: PathBuf,
}

impl CacheStore {
    #[allow(deprecated)]
    fn default_location() -> Result<Self, Error> {
        let home = home_dir().ok_or_else(|| {
            Error::Generic(
                "failed to get home directory. does your os support `std::env::home_dir()`?"
                    .to_string(),
            )
        })?;
        Ok(Self { root: home.join(".bifrost").join("cache") })
    }

    fn file(&self, key: &str) -> PathBuf {
        self.root.join(format!("{key}.bin"))
    }

    fn clear(&self) -> Result<(), Error> {
        for entry in self.root.read_dir()? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                std::fs::remove_dir_all(entry.path())?;
            } else {
                std::fs::remove_file(entry.path())?;
            }
        }
        Ok(())
    }

    fn exists(&self, key: &str) -> bool {
        self.file(key).exists()
    }

    fn keys(&self, pattern: &str) -> Result<Vec<String>, Error> {
        let pattern = pattern.replace('*', "");
        let mut keys = Vec::new();
        for entry in self.root.read_dir()? {
            let entry = entry?;
            let filename = entry.file_name();
            let key = filename
                .to_str()
                .ok_or_else(|| Error::Generic("failed to convert path to string".to_string()))?;
            if pattern.is_empty() || key.contains(&pattern) {
                keys.push(key.replace(".bin", ""));
            }
        }
        keys.sort();
        Ok(keys)
    }

    fn delete(&self, key: &str) -> Result<(), Error> {
        match std::fs::remove_file(self.file(key)) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.into()),
        }
    }

    fn read<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>, Error> {
        let contents = match std::fs::read_to_string(self.file(key)) {
            Ok(contents) => contents,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        let binary = decode_hex(&contents)
            .map_err(|e| Error::Generic(format!("failed to decode hex: {e:?}")))?;
        let cache: Cache<T> = bincode::deserialize(&binary)
            .map_err(|e| Error::Generic(format!("failed to deserialize cache object: {e:?}")))?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| Error::Generic(format!("failed to get current time: {e:?}")))?
            .as_secs();
        if cache.expiry < now {
            self.delete(key)?;
            return Ok(None);
        }
        Ok(Some(cache.value))
    }

    fn store<T: Serialize>(&self, key: &str, value: T, expiry: Option<u64>) -> Result<(), Error> {
        let expiry = match expiry {
            Some(expiry) => expiry,
            None => {
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_err(|e| Error::Generic(format!("failed to get current time: {e:?}")))?
                    .as_secs() +
                    60 * 60 * 24 * 90
            }
        };
        let encoded = bincode::serialize(&Cache { value, expiry })
            .map_err(|e| Error::Generic(format!("failed to serialize cache object: {e:?}")))?;
        let file = self.file(key);
        if let Some(parent) = file.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(file, encode_hex(encoded))?;
        Ok(())
    }

    fn size(&self) -> Result<u64, Error> {
        self.root.read_dir()?.try_fold(0, |size, entry| Ok(size + entry?.metadata()?.len()))
    }
}

/// Clear all cached objects.
pub fn clear_cache() -> Result<(), Error> {
    CacheStore::default_location()?.clear()
}

/// Check whether a cached object exists.
pub fn exists(key: &str) -> Result<bool, Error> {
    Ok(CacheStore::default_location()?.exists(key))
}

/// List cached objects whose names contain the given pattern (ignoring `*`).
pub fn keys(pattern: &str) -> Result<Vec<String>, Error> {
    CacheStore::default_location()?.keys(pattern)
}

/// Delete a cached object, if present.
pub fn delete_cache(key: &str) -> Result<(), Error> {
    CacheStore::default_location()?.delete(key)
}

/// Read a cached object, removing it if it has expired.
pub fn read_cache<T: 'static + DeserializeOwned>(key: &str) -> Result<Option<T>, Error> {
    CacheStore::default_location()?.read(key)
}

/// Store a value with an optional expiry as a Unix timestamp; the default is 90 days from now.
pub fn store_cache<T: Serialize>(key: &str, value: T, expiry: Option<u64>) -> Result<(), Error> {
    CacheStore::default_location()?.store(key, value, expiry)
}

/// Return a cached value, or call the function and cache its result.
pub async fn with_cache<T, F, Fut>(key: &str, func: F) -> eyre::Result<T>
where
    T: 'static + Serialize + DeserializeOwned + Send + Sync,
    F: FnOnce() -> Fut + Send,
    Fut: std::future::Future<Output = Result<T, eyre::Report>> + Send, {
    let cached = match read_cache::<T>(key) {
        Ok(value) => value,
        Err(error) => {
            tracing::warn!("failed to read cache key '{}': {}", key, error);
            None
        }
    };
    if let Some(value) = cached {
        tracing::debug!("cache hit for key: '{}'", key);
        return Ok(value);
    }
    tracing::debug!("cache miss for key: '{}'", key);
    let value = func().await?;
    store_cache(key, &value, None)?;
    Ok(value)
}

/// Cache subcommand handler.
pub fn cache(args: CacheArgs) -> Result<(), Error> {
    let store = CacheStore::default_location()?;
    match args.sub {
        Subcommands::Clean(_) => {
            store.clear()?;
            println!("Cache cleared.");
        }
        Subcommands::Ls(_) => {
            let keys = store.keys("*")?;
            println!("Displaying {} cached objects:", keys.len());
            for (i, key) in keys.iter().enumerate() {
                println!("{i:>5} : {key}");
            }
        }
        Subcommands::Size(_) => {
            println!("Cached objects: {}", store.keys("*")?.len());
            println!("Cache size: {}", prettify_bytes(store.size()?));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store_at(path: &std::path::Path) -> CacheStore {
        CacheStore { root: path.join("cache") }
    }

    #[test]
    fn cache_round_trip_and_delete() {
        let dir = tempfile::tempdir().unwrap();
        let store = store_at(dir.path());
        assert_eq!(store.read::<String>("key").unwrap(), None);
        store.store("key", "value".to_string(), None).unwrap();
        assert!(store.exists("key"));
        assert_eq!(store.read::<String>("key").unwrap().as_deref(), Some("value"));
        store.delete("key").unwrap();
        assert!(!store.exists("key"));
    }

    #[test]
    fn expired_entries_are_removed() {
        let dir = tempfile::tempdir().unwrap();
        let store = store_at(dir.path());
        store.store("key", "value", Some(1)).unwrap();
        assert_eq!(store.read::<String>("key").unwrap(), None);
        assert!(!store.exists("key"));
    }

    #[test]
    fn malformed_cache_files_return_errors() {
        let dir = tempfile::tempdir().unwrap();
        let store = store_at(dir.path());
        std::fs::create_dir_all(&store.root).unwrap();
        for contents in ["0", "é", "0z"] {
            std::fs::write(store.file("key"), contents).unwrap();
            assert!(store.read::<String>("key").is_err());
        }
    }

    #[test]
    fn list_and_clear_only_the_selected_store() {
        let dir = tempfile::tempdir().unwrap();
        let store = store_at(dir.path());
        store.store("some_key", 1, None).unwrap();
        store.store("some_other_key", 2, None).unwrap();
        store.store("other", 3, None).unwrap();
        assert_eq!(store.keys("some_*").unwrap(), vec!["some_key", "some_other_key"]);
        store.clear().unwrap();
        assert!(store.keys("*").unwrap().is_empty());
    }
}
