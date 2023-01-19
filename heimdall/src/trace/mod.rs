mod tests;

use std::{
    str::FromStr,
};

use clap::{AppSettings, Parser};
use ethers::{
    core::types::{H256},
    providers::{Middleware, Provider, Http}, types::{U256},
};

use heimdall_common::{
    io::logging::Logger,
    constants::TRANSACTION_HASH_REGEX,
    ether::{evm::{vm::{Block}}, util::simulate}, utils::strings::encode_hex
};

#[derive(Debug, Clone, Parser)]
#[clap(about = "Trace a contract interaction, revealing the internals of the transaction.",
       after_help = "For more information, read the wiki: https://jbecker.dev/r/heimdall-rs/wiki",
       global_setting = AppSettings::DeriveDisplayOrder, 
       override_usage = "heimdall trace <TARGET> [OPTIONS]")]
pub struct TraceArgs {
    
    /// The transaction hash of the target to trace.
    #[clap(required=true)]
    pub target: String,

    /// Set the output verbosity level, 1 - 5.
    #[clap(flatten)]
    pub verbose: clap_verbosity_flag::Verbosity,

    /// The RPC provider to use for fetching target bytecode.
    #[clap(long="rpc-url", short, default_value = "", hide_default_value = true)]
    pub rpc_url: String,

    /// When prompted, always select the default value.
    #[clap(long, short)]
    pub default: bool,

}


#[allow(deprecated)]
pub fn trace(args: TraceArgs) {
    let (logger, mut trace)= Logger::new(args.verbose.log_level().unwrap().as_str());
    let raw_transaction;
    let raw_block;
    let calldata;
    let interacted_with;
    let contract_bytecode;
    let block_number;

    // determine whether or not the target is a transaction hash
    if TRANSACTION_HASH_REGEX.is_match(&args.target).unwrap() {

        // create new runtime block
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        
        // We are decoding a transaction hash, so we need to fetch the calldata from the RPC provider.
        raw_transaction = rt.block_on(async {

            // make sure the RPC provider isn't empty
            if &args.rpc_url.len() <= &0 {
                logger.error("tracing an on-chain transaction requires an RPC provider. Use `heimdall decode --help` for more information.");
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

            return raw_transaction;
        });

        calldata = raw_transaction.input.to_string();
        interacted_with = match raw_transaction.to {
            Some(to) => to,
            None => {
                logger.error(&format!("transaction '{}' is a contract creation.", &args.target).to_string());
                std::process::exit(1)
            }
        };

        // get the bytecode of the contract that was interacted with
        contract_bytecode = rt.block_on(async {

            // create new provider
            let provider = match Provider::<Http>::try_from(&args.rpc_url) {
                Ok(provider) => provider,
                Err(_) => {
                    logger.error(&format!("failed to connect to RPC provider '{}' .", &args.rpc_url).to_string());
                    std::process::exit(1)
                }
            };

            // fetch the bytecode at the address
            let bytecode_as_bytes = match provider.get_code(interacted_with, None).await {
                Ok(bytecode) => bytecode,
                Err(_) => {
                    logger.error(&format!("failed to fetch bytecode from '{:?}' .", &interacted_with.to_string()).to_string());
                    std::process::exit(1)
                }
            };

            return bytecode_as_bytes.to_string().replacen("0x", "", 1);
        });

        // get the block number of the transaction
        block_number = match raw_transaction.block_number {
            Some(block_number) => block_number.as_u64(),
            None => {
                logger.error(&format!("transaction '{}' is pending.", &args.target).to_string());
                std::process::exit(1)
            }
        };

        // get the timestamp of the block
        raw_block = rt.block_on(async {

            // create new provider
            let provider = match Provider::<Http>::try_from(&args.rpc_url) {
                Ok(provider) => provider,
                Err(_) => {
                    logger.error(&format!("failed to connect to RPC provider '{}' .", &args.rpc_url).to_string());
                    std::process::exit(1)
                }
            };

            // fetch the block
            let block = match provider.get_block(block_number).await {
                Ok(block) => match block {
                    Some(block) => block,
                    None => {
                        logger.error(&format!("block '{}' doesn't exist.", &block_number).to_string());
                        std::process::exit(1)
                    }
                }
                Err(_) => {
                    logger.error(&format!("failed to fetch block '{}' .", &block_number).to_string());
                    std::process::exit(1)
                }
            };

            return block;
        });

        println!("{:#?}", raw_transaction);
    }
    else {
        logger.error(&format!("'{}' is not a valid transaction hash.", &args.target));
        std::process::exit(1);
    }

    // check if calldata is present
    if calldata.len() == 0 {
        logger.error(&format!("empty calldata found at '{}' .", &args.target));
        std::process::exit(1);
    }

    // check if bytecode is present
    if contract_bytecode.len() == 0 {
        logger.error(&format!("empty calldata found at '{}' .", &args.target));
        std::process::exit(1);
    }

    let raw_trace = simulate(
        args.rpc_url.clone(),
        format!("0x{}", encode_hex(interacted_with.as_bytes().to_vec())),
        calldata.clone(),
        contract_bytecode.clone(),
        format!("0x{}", encode_hex(raw_transaction.from.as_bytes().to_vec())),
        raw_transaction.value.as_u128(),
        raw_transaction.gas.as_u128(),
        Block {
            number: U256::from_str(raw_block.number.unwrap().to_string().as_str()).unwrap(),
            hash: U256::from_str(&encode_hex(raw_block.hash.unwrap().as_bytes().to_vec())).unwrap(),
            timestamp: raw_block.timestamp,
            coinbase: U256::from_str(&encode_hex(raw_block.author.unwrap().as_bytes().to_vec())).unwrap(),
            difficulty: raw_block.difficulty,
            gas_limit: raw_block.gas_limit,
            base_fee: raw_block.base_fee_per_gas.unwrap(),
        },
        0,
        &mut trace,
        0
    );

    println!("{:#?}", raw_trace);

    // force the trace to display
    trace.level = 4;
    trace.display();

}