//! Configuration management for Heimdall
//!
//! This crate provides functionality for managing the Heimdall configuration,
//! including loading, saving, updating, and deleting configuration settings.

/// Error types for the configuration module
pub mod error;

use crate::error::Error;
use clap::Parser;
use heimdall_common::utils::io::file::{delete_path, read_file, write_file};
use serde::{Deserialize, Serialize};
#[allow(deprecated)]
use std::env::home_dir;
use tracing::{debug, error, info};

/// Command line arguments for the configuration command
#[derive(Debug, Clone, Parser)]
#[clap(
    about = "Display and edit the current configuration",
    after_help = "For more information, read the wiki: https://jbecker.dev/r/heimdall-rs/wiki",
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
    /// The URL for the Ethereum RPC endpoint
    pub rpc_url: String,

    /// The URL for a local Ethereum RPC endpoint
    pub local_rpc_url: String,

    /// The API key for Etherscan services
    pub etherscan_api_key: String,

    /// The API key for Transpose services
    pub transpose_api_key: String,

    /// The API key for OpenAI services
    pub openai_api_key: String,
}

impl Default for Configuration {
    fn default() -> Self {
        Configuration {
            rpc_url: "".to_string(),
            local_rpc_url: "http://localhost:8545".to_string(),
            etherscan_api_key: "".to_string(),
            transpose_api_key: "".to_string(),
            openai_api_key: "".to_string(),
        }
    }
}

#[allow(deprecated)]
impl Configuration {
    /// Returns the current configuration.
    pub fn load() -> Result<Self, Error> {
        let mut home = home_dir().ok_or_else(|| {
            Error::Generic(
                "failed to get home directory. does your os support `std::env::home_dir()`?"
                    .to_string(),
            )
        })?;
        home.push(".bifrost");
        home.push("config.toml");

        // if the config file doesn't exist, create it
        if !home.exists() {
            let config = Configuration::default();
            config.save()?;
        }

        // read the config file
        let contents = read_file(
            home.to_str()
                .ok_or_else(|| Error::Generic("failed to convert path to string".to_string()))?,
        )
        .map_err(|e| Error::Generic(format!("failed to read config file: {e}")))?;

        // parse the config file
        let mut config: Configuration = toml::from_str(&contents)
            .map_err(|e| Error::ParseError(format!("failed to parse config file: {e}")))?;

        // load mesc config if enabled
        if !mesc::is_mesc_enabled() {
            return Ok(config);
        }

        if let Some(endpoint) = mesc::get_default_endpoint(Some("heimdall"))
            .map_err(|e| Error::Generic(format!("MESC error: {e}")))?
        {
            debug!("overriding rpc_url with mesc endpoint");
            config.rpc_url = endpoint.url;
        }
        if let Some(key) = mesc::metadata::get_api_key("etherscan", Some("heimdall"))
            .map_err(|e| Error::Generic(format!("MESC error: {e}")))?
        {
            debug!("overriding etherscan_api_key with mesc key");
            config.etherscan_api_key = key;
        }
        if let Some(key) = mesc::metadata::get_api_key("transpose", Some("heimdall"))
            .map_err(|e| Error::Generic(format!("MESC error: {e}")))?
        {
            debug!("overriding transpose_api_key with mesc key");
            config.transpose_api_key = key;
        }
        if let Some(key) = mesc::metadata::get_api_key("openai", Some("heimdall"))
            .map_err(|e| Error::Generic(format!("MESC error: {e}")))?
        {
            debug!("overriding openai_api_key with mesc key");
            config.openai_api_key = key;
        }

        Ok(config)
    }

    /// Saves the current configuration to disk.
    pub fn save(&self) -> Result<(), Error> {
        let mut home = home_dir().ok_or_else(|| {
            Error::Generic(
                "failed to get home directory. does your os support `std::env::home_dir()`?"
                    .to_string(),
            )
        })?;
        home.push(".bifrost");
        home.push("config.toml");

        write_file(
            home.to_str()
                .ok_or_else(|| Error::Generic("failed to convert path to string".to_string()))?,
            &toml::to_string(&self)
                .map_err(|e| Error::ParseError(format!("failed to serialize config: {e}")))?,
        )
        .map_err(|e| Error::Generic(format!("failed to write config file: {e}")))?;

        Ok(())
    }

    /// Deletes the configuration file at `$HOME/.bifrost/config.toml`.
    pub fn delete() -> Result<(), Error> {
        let mut home = home_dir().ok_or_else(|| {
            Error::Generic(
                "failed to get home directory. does your os support `std::env::home_dir()`?"
                    .to_string(),
            )
        })?;
        home.push(".bifrost");
        home.push("config.toml");

        delete_path(
            home.to_str()
                .ok_or_else(|| Error::Generic("failed to convert path to string".to_string()))?,
        );

        Ok(())
    }

    /// Update a single key/value pair in the configuration.
    pub fn update(&mut self, key: &str, value: &str) -> Result<(), Error> {
        // update the key in the struct and ensure it's the correct type
        match key {
            "rpc_url" => {
                self.rpc_url = value.to_string();
            }
            "local_rpc_url" => {
                self.local_rpc_url = value.to_string();
            }
            "etherscan_api_key" => {
                self.etherscan_api_key = value.to_string();
            }
            "transpose_api_key" => {
                self.transpose_api_key = value.to_string();
            }
            "openai_api_key" => {
                self.openai_api_key = value.to_string();
            }
            _ => {
                return Err(Error::Generic(format!(
                    "invalid key: \'{key}\' is not a valid configuration key."
                )))
            }
        }

        // write the updated config to disk
        self.save()?;

        Ok(())
    }
}

/// The `config` command is used to display and edit the current configuration.
pub fn config(args: ConfigArgs) -> Result<(), Error> {
    if !args.key.is_empty() {
        if !args.value.is_empty() {
            // read the config file and update the key/value pair
            let mut config = Configuration::load()?;
            config.update(&args.key, &args.value)?;
            info!("updated configuration! Set \'{}\' = \'{}\' .", &args.key, &args.value);
        } else {
            // key is set, but no value is set
            error!("found key but no value to set. Please specify a value to set, use `heimdall config --help` for more information.");
        }
    } else {
        // no key is set, print the config file
        println!("{:#?}", Configuration::load()?);
        info!("use `heimdall config <KEY> <VALUE>` to set a key/value pair.");
    }

    Ok(())
}

/// Parse user input --rpc-url into a full url
pub fn parse_url_arg(url: &str) -> Result<String, String> {
    if mesc::is_mesc_enabled() {
        if let Ok(Some(endpoint)) = mesc::get_endpoint_by_query(url, Some("heimdall")) {
            return Ok(endpoint.url);
        }
    }
    Ok(url.to_string())
}

#[allow(deprecated)]
#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    // Test default configuration
    #[test]
    #[serial]
    fn test_default_configuration() {
        let config = Configuration::default();
        assert_eq!(config.rpc_url, "");
        assert_eq!(config.local_rpc_url, "http://localhost:8545");
        assert_eq!(config.etherscan_api_key, "");
        assert_eq!(config.transpose_api_key, "");
        assert_eq!(config.openai_api_key, "");
    }

    // Test loading configuration from a file
    #[test]
    #[serial]
    fn test_load_configuration() {
        // delete config file if it exists
        Configuration::delete().expect("failed to delete config file");
        let config = Configuration::load().expect("failed to load config file");

        assert_eq!(config.rpc_url, "");
        assert_eq!(config.local_rpc_url, "http://localhost:8545");
        assert_eq!(config.etherscan_api_key, "");
        assert_eq!(config.transpose_api_key, "");
        assert_eq!(config.openai_api_key, "");
    }

    // Test saving configuration to a file
    #[test]
    #[serial]
    fn test_save_configuration() {
        // delete config file if it exists
        Configuration::delete().expect("failed to delete config file");
        let mut config = Configuration::default();

        // update rpc_url
        config.update("rpc_url", "http://localhost:8545").expect("failed to update rpc_url");

        // save the config file
        config.save().expect("failed to save config file");

        // load the config file
        let loaded_config = Configuration::load().expect("failed to load config file");

        // ensure the config file was saved correctly
        assert_eq!(loaded_config.rpc_url, "http://localhost:8545");
        assert_eq!(loaded_config.local_rpc_url, "http://localhost:8545");
        assert_eq!(loaded_config.etherscan_api_key, "");
        assert_eq!(loaded_config.transpose_api_key, "");
        assert_eq!(loaded_config.openai_api_key, "");
    }

    // Test deleting configuration file
    #[test]
    #[serial]
    fn test_delete_configuration() {
        // delete config file if it exists
        Configuration::delete().expect("failed to delete config file");
        let mut config = Configuration::load().expect("failed to load config file");

        // save some values to the config file
        config.update("rpc_url", "http://localhost:8545").expect("failed to update rpc_url");
        config
            .update("etherscan_api_key", "1234567890")
            .expect("failed to update etherscan_api_key");

        // delete config file if it exists
        Configuration::delete().expect("failed to delete config file");
        let config = Configuration::load().expect("failed to load config file");

        assert_eq!(config.rpc_url, "");
        assert_eq!(config.local_rpc_url, "http://localhost:8545");
        assert_eq!(config.etherscan_api_key, "");
        assert_eq!(config.transpose_api_key, "");
        assert_eq!(config.openai_api_key, "");
    }
}
