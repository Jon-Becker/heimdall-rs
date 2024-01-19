use clap::{AppSettings, Parser};
use heimdall_common::{utils::io::{
    file::{delete_path, read_file, write_file},
    logging::*,
}, error, info, success};
use serde::{Deserialize, Serialize};
#[allow(deprecated)]
use std::env::home_dir;

pub static DEFAULT_CONFIG: &str = "rpc_url = \"\"
local_rpc_url = \"http://localhost:8545\"
etherscan_api_key = \"\"
transpose_api_key = \"\"
openai_api_key = \"\"
";

#[derive(Debug, Clone, Parser)]
#[clap(
    about = "Display and edit the current configuration",
    after_help = "For more information, read the wiki: https://jbecker.dev/r/heimdall-rs/wiki",
    global_setting = AppSettings::DeriveDisplayOrder,
    override_usage = "heimdall config [OPTIONS]"
)]
pub struct ConfigArgs {
    /// The target key to update.
    #[clap(required = false, default_value = "")]
    key: String,

    /// The value to set the key to.
    #[clap(required = false, default_value = "")]
    value: String,
}

/// The [`Configuration`] struct represents the configuration of the CLI. All heimdall core modules
/// will attempt to read from this configuration when possible.
#[derive(Deserialize, Serialize, Debug)]
pub struct Configuration {
    pub rpc_url: String,
    pub local_rpc_url: String,
    pub etherscan_api_key: String,
    pub transpose_api_key: String,
    pub openai_api_key: String,
}

#[allow(deprecated)]
/// Writes the given configuration to the disc at `$HOME/.bifrost/config.toml`.
pub fn write_config(contents: &str) {
    match home_dir() {
        Some(mut home) => {
            home.push(".bifrost");
            home.push("config.toml");

            let _ = write_file(home.into_os_string().to_str().unwrap(), contents);
        }
        None => {
            error!(
                "couldn't resolve the bifrost directory. Is your $HOME variable set correctly?",
            );
            std::process::exit(1)
        }
    }
}

#[allow(deprecated)]
/// Deletes the configuration file at `$HOME/.bifrost/config.toml`.
pub fn delete_config() {
    match home_dir() {
        Some(mut home) => {
            home.push(".bifrost");
            home.push("config.toml");

            let _ = delete_path(home.into_os_string().to_str().unwrap());
        }
        None => {
            error!(
                "couldn't resolve the bifrost directory. Is your $HOME variable set correctly?",
            );
            std::process::exit(1)
        }
    }
}

#[allow(deprecated)]
/// Reads the configuration file at `$HOME/.bifrost/config.toml`.
pub fn read_config() -> String {
    match home_dir() {
        Some(mut home) => {
            home.push(".bifrost");
            home.push("config.toml");

            if home.as_path().exists() {
                // the file exists, read it
                return read_file(home.into_os_string().to_str().unwrap())
            } else {
                // the file does not exist, create it
                write_config(DEFAULT_CONFIG);
                return read_file(home.into_os_string().to_str().unwrap())
            }
        }
        None => {
            error!(
                "couldn't resolve the bifrost directory. Is your $HOME variable set correctly?",
            );
            std::process::exit(1)
        }
    }
}

/// Returns the [`Configuration`] struct after parsing the configuration file at
/// `$HOME/.bifrost/config.toml`.
pub fn get_config() -> Configuration {
    let contents = read_config();

    // toml parse from contents into Configuration
    let config: Configuration = match toml::from_str(&contents) {
        Ok(config) => config,
        Err(e) => {
            error!("failed to parse config file: {}", e);
            info!("regenerating config file...");
            delete_config();
            return get_config()
        }
    };
    config
}

/// update a single key/value pair in the configuration file
pub fn update_config(key: &str, value: &str) {
    let mut contents = get_config();

    // update the key in the struct and ensure it's the correct type
    match key {
        "rpc_url" => {
            contents.rpc_url = value.to_string();
        }
        "local_rpc_url" => {
            contents.local_rpc_url = value.to_string();
        }
        "etherscan_api_key" => {
            contents.etherscan_api_key = value.to_string();
        }
        "transpose_api_key" => {
            contents.transpose_api_key = value.to_string();
        }
        "openai_api_key" => {
            contents.openai_api_key = value.to_string();
        }
        _ => {
            error!("unknown configuration key \'{}\' .", key);
            std::process::exit(1)
        }
    }

    // write the updated config to disk
    let serialized_config = toml::to_string(&contents).unwrap();
    write_config(&serialized_config);
}

/// The `config` command is used to display and edit the current configuration.
pub fn config(args: ConfigArgs) {
    if !args.key.is_empty() {
        if !args.value.is_empty() {
            // read the config file and update the key/value pair
            update_config(&args.key, &args.value);
            success!(
                "updated configuration! Set \'{}\' = \'{}\' .",
                &args.key, &args.value
            );
        } else {
            // key is set, but no value is set
            error!("found key but no value to set. Please specify a value to set, use `heimdall config --help` for more information.");
            std::process::exit(1);
        }
    } else {
        // no key is set, print the config file
        println!("{:#?}", get_config());
        info!("use `heimdall config <KEY> <VALUE>` to set a key/value pair.");
    }
}
