use toml;
use serde::{Deserialize, Serialize};
use std::{env::home_dir};

use clap::{AppSettings, Parser};

use heimdall_common::{
    io::{
        file::{read_file, write_file},
        logging::*,
    }
};


#[derive(Debug, Clone, Parser)]
#[clap(about = "Display and edit the current configuration",
       after_help = "For more information, read the wiki: https://jbecker.dev/r/heimdall-rs/wiki",
       global_setting = AppSettings::DeriveDisplayOrder, 
       override_usage = "heimdall config [OPTIONS]")]
pub struct ConfigArgs {
    // The target key to update.
    #[clap(required=false, default_value="")]
    key: String,

    #[clap(required=false, default_value="")]
    value: String,

}


pub static DEFAULT_CONFIG: &str = "verbosity = 0
quiet = 0
output = \"\"
rpc_url = \"\"
local_rpc_url = \"http://localhost:8545\"
etherscan_api_key = \"\"
";


#[derive(Deserialize, Serialize, Debug)]
pub struct Configuration {
    verbosity: u8,
    quiet: u8,
    output: String,
    rpc_url: String,
    local_rpc_url: String,
    etherscan_api_key: String,
}


pub fn write_config(contents: String) {
    match home_dir() {
        Some(mut home) => {
            home.push(".bifrost");
            home.push("config.toml");
            
            let _ = write_file(&home.into_os_string().to_str().unwrap().to_string(), &contents);
        }
        None => {
            error(&format!("couldn't resolve the bifrost directory. Is your $HOME variable set correctly?").to_string());
            std::process::exit(1)
        }
    }
}


pub fn read_config() -> String {
    match home_dir() {
        Some(mut home) => {
            home.push(".bifrost");
            home.push("config.toml");
            
            if home.as_path().exists() {

                // the file exists, read it
                return read_file(&home.into_os_string().to_str().unwrap().to_string());
            }
            else {

                // the file does not exist, create it
                write_config(DEFAULT_CONFIG.to_string());
                return read_file(&home.into_os_string().to_str().unwrap().to_string());
            }
        }
        None => {
            error(&format!("couldn't resolve the bifrost directory. Is your $HOME variable set correctly?").to_string());
            std::process::exit(1)
        }
    }
}


pub fn get_config() -> Configuration {
    let contents = read_config();

    // toml parse from contents into Configuration
    let config: Configuration = toml::from_str(&contents).unwrap();
    return config;
}


pub fn update_config(key: &String, value: &String) {
    let mut contents = get_config();

    // update the key in the struct and ensure it's the correct type
    match key.as_str() {
        "verbosity" => {
            contents.verbosity = match value.parse::<u8>() {
                Ok(v) => v,
                Err(_) => {
                    error(&format!("key \'{}\' expects a number but got value \'{}\' .", &key, &value).to_string());
                    std::process::exit(1)
                }
            };
        }
        "quiet" => {
            contents.quiet = match value.parse::<u8>() {
                Ok(v) => v,
                Err(_) => {
                    error(&format!("key \'{}\' expects a number but got value \'{}\' .", &key, &value).to_string());
                    std::process::exit(1)
                }
            };
        }
        "output" => {
            contents.output = value.to_string();
        }
        "rpc_url" => {
            contents.rpc_url = value.to_string();
        }
        "local_rpc_url" => {
            contents.local_rpc_url = value.to_string();
        }
        "etherscan_api_key" => {
            contents.etherscan_api_key = value.to_string();
        }
        _ => {
            error(&format!("unknown configuration key \'{}\' .", key).to_string());
            std::process::exit(1)
        }
    }

    // write the updated config to disk
    let serialized_config = toml::to_string(&contents).unwrap();
    write_config(serialized_config);
}


pub fn config(args: ConfigArgs) {
    if &args.key != "" {
        
        if &args.value != "" {
            
            // read the config file and update the key/value pair
            update_config(&args.key, &args.value);
            success(&format!("updated configuration! Set \'{}\' = \'{}\' .", &args.key, &args.value).to_string());
        }
        else {

            // key is set, but no value is set
            error("found key but no value to set. Please specify a value to set, use `heimdall config --help` for more information.");
            std::process::exit(1);
        }
    }
    else {

        // no key is set, print the config file
        println!("{:#?}", get_config());
        info("use `heimdall config <KEY> <VALUE>` to set a key/value pair.");
    }
}