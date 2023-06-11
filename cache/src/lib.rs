use clap::{AppSettings, Parser};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
#[allow(deprecated)]
use std::env::home_dir;

use util::*;

pub mod tests;
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

#[derive(Debug, Clone, Parser)]
pub struct NoArguments {}

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
        delete_path(&path.to_str().unwrap().to_string());
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
        let binary_string = match read_file(&path.to_str().unwrap().to_string()) {
            Some(s) => s,
            None => return false,
        };

        let binary_vec = decode_hex(&binary_string);
        if binary_vec.is_err() {
            return false
        }

        let cache: Result<Cache<T>, _> = bincode::deserialize(&binary_vec.unwrap());
        if cache.is_err() {
            delete_path(&path.to_str().unwrap().to_string());
        };

        let cache = cache.unwrap();
        if cache.expiry <
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs()
        {
            // delete file
            delete_path(&path.to_str().unwrap().to_string());
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

    let binary_string = match read_file(&cache_file.to_str().unwrap().to_string()) {
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
    write_file(&cache_file.to_str().unwrap().to_string(), &binary_string);
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
    }

    Ok(())
}
