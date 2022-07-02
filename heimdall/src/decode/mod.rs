use std::{
    str::FromStr
};

use clap::{AppSettings, Parser};
use ethers::{
    core::types::{H256},
    providers::{Middleware, Provider, Http},
    abi::{ParamType, decode as decode_abi},
};

use heimdall_common::{
    io::logging::Logger,
    consts::TRANSACTION_HASH_REGEX,
    utils::{
        strings::{
            replace_last, decode_hex
        },
        http::{
            get_json_from_url,
        }
    }, ether::evm::types::to_abi_type
};


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


#[derive(Debug, Clone)]
pub struct Function {
    pub name: String,
    pub signature: String,
    pub inputs: Vec<String>
}


pub fn resolve_signature(signature: &String) -> Option<Vec<Function>> {

    // get function possibilities from 4byte
    let signatures = match get_json_from_url(format!("https://www.4byte.directory/api/v1/signatures/?format=json&hex_signature=0x{}", &signature)) {
        Some(signatures) => signatures,
        None => return None
    };

    // convert the serde value into a vec of possible functions
    let results = match signatures.get("results") {
        Some(results) => match results.as_array() {
            Some(results) => results,
            None => return None
        },
        None => return None
    };

    let mut signature_list: Vec<Function> = Vec::new();

    for signature in results {

        // get the function text signature and unwrap it into a string
        let text_signature = match signature.get("text_signature") {
            Some(text_signature) => text_signature.to_string().replace("\"", ""),
            None => continue
        };
        
        // safely split the text signature into name and inputs
        let function_parts = match text_signature.split_once("(") {
            Some(function_parts) => function_parts,
            None => continue
        };

        signature_list.push(Function {
            name: function_parts.0.to_string(),
            signature: text_signature.to_string(),
            inputs: replace_last(function_parts.1.to_string(), ")", "").split(",").map(|input| input.to_string()).collect()
        });

    }

    return match signature_list.len() {
        0 => None,
        _ => Some(signature_list)
    }

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
    if calldata.len() % 2 != 0 {
        logger.error("calldata is not a valid hex string.");
        std::process::exit(1);
    }

    if calldata[8..].len() % 64 != 0 {
        logger.warn("calldata is not a standard size. decoding may fail since each word is not exactly 32 bytes long.");
    }

    let function_signature = calldata[0..8].to_owned();
    let args = match decode_hex(&calldata[8..]) {
        Ok(args) => args,
        Err(_) => {
            logger.error("failed to parse bytearray from calldata.");
            std::process::exit(1)
        }
    };

    // get the function signature possibilities
    let potential_matches = match resolve_signature(&function_signature) {
        Some(signatures) => signatures,
        None => Vec::new()
    };

    let mut matches: Vec<Function> = Vec::new();

    for potential_match in &potential_matches {        
        let mut inputs: Vec<ParamType> = Vec::new();
        for input in &potential_match.inputs {
            match to_abi_type(input.to_owned()) {
                Some(type_) => inputs.push(type_),
                None => continue
            }
        }
        println!("{:#?}", inputs);
        match decode_abi(&inputs, &args) {
            Ok(result) => {
                if result.len() == potential_match.inputs.len() {
                    println!("{:#?}", result);
                    matches.push(potential_match.to_owned());
                }
            },
            Err(_) => continue
        }
    }

    if matches.len() == 0 {
        logger.warn("couldn't find any matches for the given function signature.");

        //TODO: return
        
    }
    else {

    }

}