use std::str::FromStr;

use clap::{AppSettings, Parser};
use ethers::{
    core::types::{H256},
    providers::{Middleware, Provider, Http},
};

use heimdall_common::{io::logging::Logger, consts::TRANSACTION_HASH_REGEX};


#[derive(Debug, Clone, Parser)]
#[clap(about = "Decode calldata into readable types",
       after_help = "For more information, read the wiki: https://jbecker.dev/r/heimdall-rs/wiki",
       global_setting = AppSettings::DeriveDisplayOrder, 
       override_usage = "heimdall decode <TARGET> [OPTIONS]")]
pub struct DecodeArgs {
    // The target to decode, either a transaction hash or string of bytes.
    #[clap(required=true)]
    pub target: String,

    // Set the output verbosity level, 1 - 5.
    #[clap(flatten)]
    pub verbose: clap_verbosity_flag::Verbosity,

    // The RPC provider to use for fetching target bytecode.
    #[clap(long="rpc-url", short, default_value = "", hide_default_value = true)]
    pub rpc_url: String,

    // When prompted, always select the default value.
    #[clap(long, short)]
    pub default: bool,

}

pub fn decode(args: DecodeArgs) {
    let (logger, _)= Logger::new(args.verbose.log_level().unwrap().as_str());
    
    let calldata: String;

    // determine whether or not the target is a transaction hash
    if TRANSACTION_HASH_REGEX.is_match(&args.target) {

        // create new runtime block
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        
        // We are decoding a transaction hash, so we need to fetch the calldata from the RPC provider.
        calldata = rt.block_on(async {

            // make sure the RPC provider isn't empty
            if &args.rpc_url.len() <= &0 {
                logger.error("decoging an on-chain transaction requires an RPC provider. Use `heimdall decode --help` for more information.");
                std::process::exit(1);
            }

            // create new provider
            let provider = match Provider::<Http>::try_from(&args.rpc_url) {
                Ok(provider) => provider,
                Err(_) => {
                    logger.error(&format!("failed to connect to RPC provider '{}' .", &args.rpc_url).to_string());
                    std::process::exit(1)
                }
            };

            // safely unwrap the transaction hash
            let transaction_hash = match H256::from_str(&args.target) {
                Ok(transaction_hash) => transaction_hash,
                Err(_) => {
                    logger.error(&format!("failed to parse transaction hash '{}' .", &args.target));
                    std::process::exit(1)
                }
            };

            // fetch the transaction from the node
            let raw_transaction = match provider.get_transaction(transaction_hash).await {
                Ok(bytecode) => {
                    match bytecode {
                        Some(bytecode) => bytecode,
                        None => {
                            logger.error(&format!("transaction '{}' doesn't exist.", &args.target).to_string());
                            std::process::exit(1)
                        }
                    }
                },
                Err(_) => {
                    logger.error(&format!("failed to fetch calldata from '{}' .", &args.target).to_string());
                    std::process::exit(1)
                }
            };

            return raw_transaction.input.to_string().replace("0x", "")
        });
    }
    else {
        calldata = args.target.clone().replace("0x", "");
    }

    // check if the calldata length is a standard length
    if calldata[8..].len() % 64 != 0 {
        println!("{}", calldata[..8].len());
        logger.warn("calldata is not a standard size. decoding may fail since each word is not exactly 32 bytes long.");
    }

    let function_signature = calldata[0..8].to_owned();
    let args = calldata[8..].to_owned().chars().collect::<Vec<char>>().chunks(64).map(|c| c.iter().collect::<String>()).collect::<Vec<String>>();

    logger.info(&function_signature);
    println!("{:#?}", args);
    

}