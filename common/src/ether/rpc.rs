use std::str::FromStr;

use crate::io::logging::Logger;
use ethers::{
    core::types::Address,
    providers::{Http, Middleware, Provider},
    types::{Transaction, H256},
};
use heimdall_cache::{read_cache, store_cache};

pub fn get_code(contract_address: &str, rpc_url: &str) -> String {
    // create new runtime block
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();

    // get a new logger
    let level = std::env::var("RUST_LOG").unwrap_or_else(|_| "INFO".into());
    let (logger, _) = Logger::new(&level);

    logger
        .debug_max(&format!("fetching bytecode from node for contract: '{}' .", &contract_address));

    rt.block_on(async {

        // check the cache for a matching address
        if let Some(bytecode) = read_cache(&format!("contract.{}", &contract_address)) {
            logger.debug(&format!("found cached bytecode for '{}' .", &contract_address));
            return bytecode;
        }

        // make sure the RPC provider isn't empty
        if rpc_url.is_empty() {
            logger.error("disassembling an on-chain contract requires an RPC provider. Use `heimdall disassemble --help` for more information.");
            std::process::exit(1);
        }

        // create new provider
        let provider = match Provider::<Http>::try_from(rpc_url) {
            Ok(provider) => provider,
            Err(_) => {
                logger.error(&format!("failed to connect to RPC provider '{}' .", &rpc_url));
                std::process::exit(1)
            }
        };

        // safely unwrap the address
        let address = match contract_address.parse::<Address>() {
            Ok(address) => address,
            Err(_) => {
                logger.error(&format!("failed to parse address '{}' .", &contract_address));
                std::process::exit(1)
            }
        };

        // fetch the bytecode at the address
        let bytecode_as_bytes = match provider.get_code(address, None).await {
            Ok(bytecode) => bytecode,
            Err(_) => {
                logger.error(&format!("failed to fetch bytecode from '{}' .", &contract_address));
                std::process::exit(1)
            }
        };

        // cache the results
        store_cache(&format!("contract.{}", &contract_address), bytecode_as_bytes.to_string().replacen("0x", "", 1), None);

        bytecode_as_bytes.to_string()
    })
}

pub fn get_transaction(transaction_hash: &str, rpc_url: &str) -> Transaction {
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();

    // get a new logger
    let level = std::env::var("RUST_LOG").unwrap_or_else(|_| "INFO".into());
    let (logger, _) = Logger::new(&level);

    logger.debug_max(&format!(
        "fetching calldata from node for transaction: '{}' .",
        &transaction_hash
    ));

    // We are decoding a transaction hash, so we need to fetch the calldata from the RPC provider.
    rt.block_on(async {

        // make sure the RPC provider isn't empty
        if rpc_url.is_empty() {
            logger.error("decoding an on-chain transaction requires an RPC provider. Use `heimdall decode --help` for more information.");
            std::process::exit(1);
        }

        // create new provider
        let provider = match Provider::<Http>::try_from(rpc_url) {
            Ok(provider) => provider,
            Err(_) => {
                logger.error(&format!("failed to connect to RPC provider '{}' .", &rpc_url));
                std::process::exit(1)
            }
        };

        // safely unwrap the transaction hash
        let transaction_hash = match H256::from_str(transaction_hash) {
            Ok(transaction_hash) => transaction_hash,
            Err(_) => {
                logger.error(&format!("failed to parse transaction hash '{}' .", &transaction_hash));
                std::process::exit(1)
            }
        };

        // fetch the transaction from the node
        match provider.get_transaction(transaction_hash).await {
            Ok(tx) => {
                match tx {
                    Some(tx) => tx,
                    None => {
                        logger.error(&format!("transaction '{}' doesn't exist.", &transaction_hash));
                        std::process::exit(1)
                    }
                }
            },
            Err(_) => {
                logger.error(&format!("failed to fetch calldata from '{}' .", &transaction_hash));
                std::process::exit(1)
            }
        }
    })
}
