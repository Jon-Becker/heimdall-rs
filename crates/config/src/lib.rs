//! Configuration management for Heimdall
//!
//! This crate provides functionality for managing the Heimdall configuration,
//! including loading, saving, updating, and deleting configuration settings.

/// Error types for the configuration module
pub mod error;

use crate::error::Error;
use clap::Parser;
use serde::{Deserialize, Serialize};
#[allow(deprecated)]
use std::env::home_dir;
use std::path::{Path, PathBuf};
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

    /// The API key for OpenRouter services
    pub openrouter_api_key: String,

    /// The default model to use for OpenRouter LLM calls (e.g., "openai/gpt-4o-mini")
    #[serde(default)]
    pub openrouter_model: String,
}

impl Default for Configuration {
    fn default() -> Self {
        Configuration {
            rpc_url: "".to_string(),
            local_rpc_url: "http://localhost:8545".to_string(),
            etherscan_api_key: "".to_string(),
            transpose_api_key: "".to_string(),
            openrouter_api_key: "".to_string(),
            openrouter_model: "".to_string(),
        }
    }
}

#[allow(deprecated)]
impl Configuration {
    /// Returns the current configuration.
    pub fn load() -> Result<Self, Error> {
        let mut config = Self::load_from(&Self::path()?)?;

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
        if let Some(key) = mesc::metadata::get_api_key("openrouter", Some("heimdall"))
            .map_err(|e| Error::Generic(format!("MESC error: {e}")))?
        {
            debug!("overriding openrouter_api_key with mesc key");
            config.openrouter_api_key = key;
        }

        Ok(config)
    }

    fn path() -> Result<PathBuf, Error> {
        let home = home_dir().ok_or_else(|| {
            Error::Generic(
                "failed to get home directory. does your os support `std::env::home_dir()`?"
                    .to_string(),
            )
        })?;
        Ok(home.join(".bifrost").join("config.toml"))
    }

    fn load_from(path: &Path) -> Result<Self, Error> {
        if !path.exists() {
            let config = Self::default();
            config.save_to(path)?;
        }
        let contents = std::fs::read_to_string(path)
            .map_err(|e| Error::Generic(format!("failed to read config file: {e}")))?;
        toml::from_str(&contents)
            .map_err(|e| Error::ParseError(format!("failed to parse config file: {e}")))
    }

    fn save_to(&self, path: &Path) -> Result<(), Error> {
        let contents = toml::to_string(self)
            .map_err(|e| Error::ParseError(format!("failed to serialize config: {e}")))?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| Error::Generic(format!("failed to create config directory: {e}")))?;
        }
        std::fs::write(path, contents)
            .map_err(|e| Error::Generic(format!("failed to write config file: {e}")))
    }

    fn delete_from(path: &Path) -> Result<(), Error> {
        match std::fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(Error::Generic(format!("failed to delete config file: {error}"))),
        }
    }

    /// Saves the current configuration to disk.
    pub fn save(&self) -> Result<(), Error> {
        self.save_to(&Self::path()?)
    }

    /// Deletes the configuration file at `$HOME/.bifrost/config.toml`.
    pub fn delete() -> Result<(), Error> {
        Self::delete_from(&Self::path()?)
    }

    /// Update a single key/value pair in the configuration.
    pub fn update(&mut self, key: &str, value: &str) -> Result<(), Error> {
        self.update_at(key, value, &Self::path()?)
    }

    fn update_at(&mut self, key: &str, value: &str, path: &Path) -> Result<(), Error> {
        self.set(key, value)?;
        let mut stored = Self::load_from(path)?;
        stored.set(key, value)?;
        stored.save_to(path)
    }

    fn set(&mut self, key: &str, value: &str) -> Result<(), Error> {
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
            "openrouter_api_key" => {
                self.openrouter_api_key = value.to_string();
            }
            "openrouter_model" => {
                self.openrouter_model = value.to_string();
            }
            _ => {
                return Err(Error::Generic(format!(
                    "invalid key: \'{key}\' is not a valid configuration key."
                )))
            }
        }

        Ok(())
    }
}

/// The `config` command is used to display and edit the current configuration.
pub fn config(args: ConfigArgs) -> Result<(), Error> {
    if !args.key.is_empty() {
        if !args.value.is_empty() {
            let mut config = Configuration::load()?;
            config.update(&args.key, &args.value)?;
            info!("updated configuration! Set \'{}\' = \'{}\' .", &args.key, &args.value);
        } else {
            error!("found key but no value to set. Please specify a value to set, use `heimdall config --help` for more information.");
        }
    } else {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn updating_one_value_does_not_persist_external_overrides() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let local = Configuration {
            rpc_url: "local-rpc".into(),
            etherscan_api_key: "local-key".into(),
            ..Configuration::default()
        };
        local.save_to(&path).unwrap();
        let mut effective = Configuration::load_from(&path).unwrap();
        effective.rpc_url = "external-rpc".into();
        effective.etherscan_api_key = "external-secret".into();
        effective.update_at("openrouter_model", "new-model", &path).unwrap();
        let stored = Configuration::load_from(&path).unwrap();
        assert_eq!(stored.rpc_url, "local-rpc");
        assert_eq!(stored.etherscan_api_key, "local-key");
        assert_eq!(stored.openrouter_model, "new-model");
        assert_eq!(effective.rpc_url, "external-rpc");
        assert_eq!(effective.openrouter_model, "new-model");
    }

    #[test]
    fn configuration_round_trip_and_delete() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested/config.toml");
        let mut config = Configuration::load_from(&path).unwrap();
        assert_eq!(config.local_rpc_url, "http://localhost:8545");
        assert!(config.rpc_url.is_empty());
        config.set("rpc_url", "https://example.invalid").unwrap();
        config.set("etherscan_api_key", "key").unwrap();
        config.save_to(&path).unwrap();
        let loaded = Configuration::load_from(&path).unwrap();
        assert_eq!(loaded.rpc_url, "https://example.invalid");
        assert_eq!(loaded.etherscan_api_key, "key");
        assert!(config.set("unknown", "value").is_err());
        Configuration::delete_from(&path).unwrap();
        assert!(Configuration::load_from(&path).unwrap().rpc_url.is_empty());
    }
}
