use std::{str::FromStr, time::Duration};

use crate::utils::io::logging::Logger;
use backoff::ExponentialBackoff;
use ethers::{
    core::types::Address,
    providers::{Http, Middleware, Provider},
    types::{Transaction, H256},
};
use heimdall_cache::{read_cache, store_cache};

/// Get the chainId of the provided RPC URL
///
/// ```no_run
/// use heimdall_common::ether::rpc::chain_id;
///
/// // let chain_id = chain_id("https://eth.llamarpc.com").await.unwrap();
/// //assert_eq!(chain_id, 1);
/// ```
pub async fn chain_id(rpc_url: &str) -> Result<u64, Box<dyn std::error::Error>> {
    backoff::future::retry(
        ExponentialBackoff {
            max_elapsed_time: Some(Duration::from_secs(10)),
            ..ExponentialBackoff::default()
        },
    || async {
        // get a new logger
        let logger = Logger::default();

        logger.debug_max(&format!("checking chain id for rpc url: '{}'", &rpc_url));

        // check the cache for a matching rpc url
        let cache_key = format!("chain_id.{}", &rpc_url.replace('/', "").replace(['.', ':'], "-"));
        if let Some(chain_id) = read_cache(&cache_key) {
            logger.debug(&format!("found cached chain id for rpc url: {:?}", &rpc_url));
            return Ok(chain_id)
        }

        // make sure the RPC provider isn't empty
        if rpc_url.is_empty() {
            logger.error("reading on-chain data requires an RPC provider. Use `heimdall --help` for more information.");
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

        // fetch the chain id from the node
        let chain_id = match provider.get_chainid().await {
            Ok(chain_id) => chain_id,
            Err(_) => {
                logger.error(&format!("failed to fetch chain id from '{}' .", &rpc_url));
                return Err(backoff::Error::Transient { err: (), retry_after: Some(Duration::from_secs(1)) })
            }
        };

        // cache the results
        store_cache(&cache_key, chain_id.as_u64(), None);

        logger.debug_max(&format!("chain_id is '{}'", &chain_id));

        Ok(chain_id.as_u64())
    })
    .await
    .map_err(|_| Box::from("failed to fetch chain id"))
}

/// Get the bytecode of the provided contract address
///
/// ```no_run
/// use heimdall_common::ether::rpc::get_code;
///
/// // let bytecode = get_code("0x0", "https://eth.llamarpc.com").await;
/// // assert!(bytecode.is_ok());
/// ```
pub async fn get_code(
    contract_address: &str,
    rpc_url: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    backoff::future::retry(
        ExponentialBackoff {
            max_elapsed_time: Some(Duration::from_secs(10)),
            ..ExponentialBackoff::default()
        },
    || async {
        // get a new logger
        let logger = Logger::default();

        // get chain_id
        let _chain_id = chain_id(rpc_url).await.unwrap_or(1);

        logger
            .debug_max(&format!("fetching bytecode from node for contract: '{}' .", &contract_address));

        // check the cache for a matching address
        if let Some(bytecode) = read_cache(&format!("contract.{}.{}", &_chain_id, &contract_address)) {
            logger.debug(&format!("found cached bytecode for '{}' .", &contract_address));
            return Ok(bytecode)
        }

        // make sure the RPC provider isn't empty
        if rpc_url.is_empty() {
            logger.error("reading on-chain data requires an RPC provider. Use `heimdall --help` for more information.");
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
                return Err(backoff::Error::Transient { err: (), retry_after: Some(Duration::from_secs(1)) })
            }
        };

        // cache the results
        store_cache(
            &format!("contract.{}.{}", &_chain_id, &contract_address),
            bytecode_as_bytes.to_string().replacen("0x", "", 1),
            None,
        );

        Ok(bytecode_as_bytes.to_string())
    })
    .await
    .map_err(|_| Box::from("failed to fetch bytecode"))
}

/// Get the raw transaction data of the provided transaction hash
///
/// ```no_run
/// use heimdall_common::ether::rpc::get_code;
///
/// // let bytecode = get_code("0x0", "https://eth.llamarpc.com").await;
/// // assert!(bytecode.is_ok());
/// ```
pub async fn get_transaction(
    transaction_hash: &str,
    rpc_url: &str,
) -> Result<Transaction, Box<dyn std::error::Error>> {
    backoff::future::retry(
        ExponentialBackoff {
            max_elapsed_time: Some(Duration::from_secs(10)),
            ..ExponentialBackoff::default()
        },
    || async {
        // get a new logger
        let logger = Logger::default();

        logger.debug_max(&format!(
            "fetching calldata from node for transaction: '{}' .",
            &transaction_hash
        ));

        // make sure the RPC provider isn't empty
        if rpc_url.is_empty() {
            logger.error("reading on-chain data requires an RPC provider. Use `heimdall --help` for more information.");
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
        Ok(match provider.get_transaction(transaction_hash).await {
            Ok(tx) => match tx {
                Some(tx) => tx,
                None => {
                    logger.error(&format!("transaction '{}' doesn't exist.", &transaction_hash));
                    std::process::exit(1)
                }
            },
            Err(_) => {
                logger.error(&format!("failed to fetch calldata from '{}' .", &transaction_hash));
                return Err(backoff::Error::Transient { err: (), retry_after: Some(Duration::from_secs(1)) })
            }
        })
    })
    .await
    .map_err(|_| Box::from("failed to fetch calldata"))
}
