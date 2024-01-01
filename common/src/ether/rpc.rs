use crate::{debug_max, error::Error, utils::io::logging::Logger};
use backoff::ExponentialBackoff;
use ethers::{
    core::types::Address,
    providers::{Http, Middleware, Provider},
    types::{
        BlockNumber::{self},
        BlockTrace, Filter, FilterBlockOption, StateDiff, TraceType, Transaction, H256,
    },
};
use heimdall_cache::{read_cache, store_cache};
use std::{str::FromStr, time::Duration};

/// Get the chainId of the provided RPC URL
///
/// ```no_run
/// use heimdall_common::ether::rpc::chain_id;
///
/// // let chain_id = chain_id("https://eth.llamarpc.com").await?;
/// //assert_eq!(chain_id, 1);
/// ```
pub async fn chain_id(rpc_url: &str) -> Result<u64, Error> {
    backoff::future::retry(
        ExponentialBackoff {
            max_elapsed_time: Some(Duration::from_secs(10)),
            ..ExponentialBackoff::default()
        },
    || async {
        // get a new logger
        let logger = Logger::default();

        debug_max!(&format!("checking chain id for rpc url: '{}'", &rpc_url));

        // check the cache for a matching rpc url
        let cache_key = format!("chain_id.{}", &rpc_url.replace('/', "").replace(['.', ':'], "-"));
        if let Some(chain_id) = read_cache(&cache_key)
            .map_err(|_| logger.error(&format!("failed to read cache for rpc url: {:?}", &rpc_url)))?
        {
            logger.debug(&format!("found cached chain id for rpc url: {:?}", &rpc_url));
            return Ok(chain_id)
        }

        // make sure the RPC provider isn't empty
        if rpc_url.is_empty() {
            logger.error("reading on-chain data requires an RPC provider. Use `heimdall --help` for more information.");
            return Err(backoff::Error::Permanent(()))
        }

        // create new provider
        let provider = match Provider::<Http>::try_from(rpc_url) {
            Ok(provider) => provider,
            Err(_) => {
                logger.error(&format!("failed to connect to RPC provider '{}' .", &rpc_url));
                return Err(backoff::Error::Permanent(()))
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
        store_cache(&cache_key, chain_id.as_u64(), None)
            .map_err(|_| logger.error(&format!("failed to cache chain id for rpc url: {:?}", &rpc_url)))?;

        debug_max!(&format!("chain_id is '{}'", &chain_id));

        Ok(chain_id.as_u64())
    })
    .await
    .map_err(|e| Error::Generic(format!("failed to get chain id: {:?}", e)))
}

/// Get the bytecode of the provided contract address
///
/// ```no_run
/// use heimdall_common::ether::rpc::get_code;
///
/// // let bytecode = get_code("0x0", "https://eth.llamarpc.com").await;
/// // assert!(bytecode.is_ok());
/// ```
pub async fn get_code(contract_address: &str, rpc_url: &str) -> Result<String, Error> {
    backoff::future::retry(
        ExponentialBackoff {
            max_elapsed_time: Some(Duration::from_secs(10)),
            ..ExponentialBackoff::default()
        },
    || async {
        // get a new logger
        let logger = Logger::default();

        // get chain_id
        let chain_id = chain_id(rpc_url).await.unwrap_or(1);

        // check the cache for a matching address
        if let Some(bytecode) = read_cache(&format!("contract.{}.{}", &chain_id, &contract_address))
            .map_err(|_| logger.error(&format!("failed to read cache for contract: {:?}", &contract_address)))?
        {
            logger.debug(&format!("found cached bytecode for '{}' .", &contract_address));
            return Ok(bytecode)
        }

        debug_max!("fetching bytecode from node for contract: '{}' .", &contract_address);

        // make sure the RPC provider isn't empty
        if rpc_url.is_empty() {
            logger.error("reading on-chain data requires an RPC provider. Use `heimdall --help` for more information.");
            return Err(backoff::Error::Permanent(()))
        }

        // create new provider
        let provider = match Provider::<Http>::try_from(rpc_url) {
            Ok(provider) => provider,
            Err(_) => {
                logger.error(&format!("failed to connect to RPC provider '{}' .", &rpc_url));
                return Err(backoff::Error::Permanent(()))
            }
        };

        // safely unwrap the address
        let address = match contract_address.parse::<Address>() {
            Ok(address) => address,
            Err(_) => {
                logger.error(&format!("failed to parse address '{}' .", &contract_address));
                return Err(backoff::Error::Permanent(()))
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
            &format!("contract.{}.{}", &chain_id, &contract_address),
            bytecode_as_bytes.to_string().replacen("0x", "", 1),
            None,
        )
        .map_err(|_| logger.error(&format!("failed to cache bytecode for contract: {:?}", &contract_address)))?;

        Ok(bytecode_as_bytes.to_string().replacen("0x", "", 1))
    })
    .await
    .map_err(|_| Error::Generic(format!("failed to get bytecode for contract: {:?}", &contract_address)))
}

/// Get the raw transaction data of the provided transaction hash
///
/// ```no_run
/// use heimdall_common::ether::rpc::get_code;
///
/// // let bytecode = get_code("0x0", "https://eth.llamarpc.com").await;
/// // assert!(bytecode.is_ok());
/// ```
/// TODO: check for caching
pub async fn get_transaction(transaction_hash: &str, rpc_url: &str) -> Result<Transaction, Error> {
    backoff::future::retry(
        ExponentialBackoff {
            max_elapsed_time: Some(Duration::from_secs(10)),
            ..ExponentialBackoff::default()
        },
    || async {
        // get a new logger
        let logger = Logger::default();

        debug_max!(&format!(
            "fetching calldata from node for transaction: '{}' .",
            &transaction_hash
        ));

        // make sure the RPC provider isn't empty
        if rpc_url.is_empty() {
            logger.error("reading on-chain data requires an RPC provider. Use `heimdall --help` for more information.");
            return Err(backoff::Error::Permanent(()));
        }

        // create new provider
        let provider = match Provider::<Http>::try_from(rpc_url) {
            Ok(provider) => provider,
            Err(_) => {
                logger.error(&format!("failed to connect to RPC provider '{}' .", &rpc_url));
                return Err(backoff::Error::Permanent(()))
            }
        };

        // safely unwrap the transaction hash
        let transaction_hash_hex = match H256::from_str(transaction_hash) {
            Ok(transaction_hash) => transaction_hash,
            Err(_) => {
                logger.error(&format!("failed to parse transaction hash '{}' .", &transaction_hash));
                return Err(backoff::Error::Permanent(()))
            }
        };

        // get the transaction
        let tx = match provider.get_transaction(transaction_hash_hex).await {
            Ok(tx) => match tx {
                Some(tx) => tx,
                None => {
                    logger.error(&format!("transaction '{}' doesn't exist.", &transaction_hash));
                    return Err(backoff::Error::Permanent(()))
                }
            },
            Err(_) => {
                logger.error(&format!("failed to fetch calldata from '{}' .", &transaction_hash));
                return Err(backoff::Error::Transient { err: (), retry_after: Some(Duration::from_secs(1)) })
            }
        };

        Ok(tx)
    })
    .await
    .map_err(|_| Error::Generic(format!("failed to get transaction: {:?}", &transaction_hash)))
}

/// Get the storage diff of the provided transaction hash
///
/// ```no_run
/// use heimdall_common::ether::rpc::get_storage_diff;
///
/// // let storage_diff = get_storage_diff("0x0", "https://eth.llamarpc.com").await;
/// // assert!(storage_diff.is_ok());
/// ```
pub async fn get_storage_diff(
    transaction_hash: &str,
    rpc_url: &str,
) -> Result<Option<StateDiff>, Error> {
    backoff::future::retry(
        ExponentialBackoff {
            max_elapsed_time: Some(Duration::from_secs(10)),
            ..ExponentialBackoff::default()
        },
        || async {
            // create new logger
            let logger = Logger::default();

            // get chain_id
            let chain_id = chain_id(rpc_url).await
                .map_err(|_| logger.error(&format!("failed to get chain id for rpc url: {:?}", &rpc_url)))?;

            // check the cache for a matching address
            if let Some(state_diff) =
                read_cache(&format!("diff.{}.{}", &chain_id, &transaction_hash))
                .map_err(|_| logger.error(&format!("failed to read cache for transaction: {:?}", &transaction_hash)))?
            {
                debug_max!("found cached state diff for transaction '{}' .", &transaction_hash);
                return Ok(state_diff)
            }

            debug_max!(&format!(
                "fetching storage diff from node for transaction: '{}' .",
                &transaction_hash
            ));

            // create new provider
            let provider = match Provider::<Http>::try_from(rpc_url) {
                Ok(provider) => provider,
                Err(_) => {
                    logger.error(&format!("failed to connect to RPC provider '{}' .", &rpc_url));
                    return Err(backoff::Error::Permanent(()))
                }
            };

            // safely unwrap the transaction hash
            let transaction_hash_hex = match H256::from_str(transaction_hash) {
                Ok(transaction_hash) => transaction_hash,
                Err(_) => {
                    logger.error(&format!(
                        "failed to parse transaction hash '{}' .",
                        &transaction_hash
                    ));
                    return Err(backoff::Error::Permanent(()))
                }
            };

            // fetch the state diff for the transaction
            let state_diff = provider
                .trace_replay_transaction(transaction_hash_hex, vec![TraceType::StateDiff])
                .await
                .map_err(|_| {
                    logger.error(&format!(
                        "failed to replay and trace transaction '{}' . does your RPC provider support it?",
                        &transaction_hash
                    ))
                })?
                .state_diff;

            // write the state diff to the cache
            store_cache(
                &format!("diff.{}.{}", &chain_id, &transaction_hash),
                &state_diff,
                None,
            )
            .map_err(|_| {
                logger.error(&format!(
                    "failed to cache state diff for transaction: {:?}",
                    &transaction_hash
                ))
            })?;

            debug_max!("fetched state diff for transaction '{}' .", &transaction_hash);

            Ok(state_diff)
        },
    )
    .await
    .map_err(|_| Error::Generic(format!("failed to get storage diff for transaction: {:?}", &transaction_hash)))
}

/// Get the raw trace data of the provided transaction hash
///
/// ```no_run
/// use heimdall_common::ether::rpc::get_trace;
///
/// // let trace = get_trace("0x0", "https://eth.llamarpc.com").await;
/// // assert!(trace.is_ok());
/// ```
/// TODO: check for caching
pub async fn get_trace(transaction_hash: &str, rpc_url: &str) -> Result<BlockTrace, Error> {
    backoff::future::retry(
        ExponentialBackoff {
            max_elapsed_time: Some(Duration::from_secs(10)),
            ..ExponentialBackoff::default()
        },
        || async {
            // create new logger
            let logger = Logger::default();

            debug_max!(&format!(
                "fetching trace from node for transaction: '{}' .",
                &transaction_hash
            ));

            // create new provider
            let provider = match Provider::<Http>::try_from(rpc_url) {
                Ok(provider) => provider,
                Err(_) => {
                    logger.error(&format!("failed to connect to RPC provider '{}' .", &rpc_url));
                    return Err(backoff::Error::Permanent(()))
                }
            };

            // safely unwrap the transaction hash
            let transaction_hash_hex = match H256::from_str(transaction_hash) {
                Ok(transaction_hash) => transaction_hash,
                Err(_) => {
                    logger.error(&format!(
                        "failed to parse transaction hash '{}' .",
                        &transaction_hash
                    ));
                    return Err(backoff::Error::Permanent(()))
                }
            };

            // fetch the trace for the transaction
            let block_trace = match provider
                .trace_replay_transaction(
                    transaction_hash_hex,
                    vec![TraceType::StateDiff, TraceType::VmTrace, TraceType::Trace],
                )
                .await
            {
                Ok(traces) => traces,
                Err(e) => {
                    logger.error(&format!(
                        "failed to replay and trace transaction '{}' . does your RPC provider support it?",
                        &transaction_hash
                    ));
                    logger.error(&format!("error: '{e}' ."));
                    return Err(backoff::Error::Permanent(()))
                }
            };

            debug_max!("fetched trace for transaction '{}' .", &transaction_hash);

            Ok(block_trace)
        },
    )
    .await
    .map_err(|_| Error::Generic(format!("failed to get trace for transaction: {:?}", &transaction_hash)))
}

/// Get all logs for the given block number
///
/// ```no_run
/// use heimdall_common::ether::rpc::get_block_logs;
///
/// // let logs = get_block_logs(1, "https://eth.llamarpc.com").await;
/// // assert!(logs.is_ok());
/// ```
pub async fn get_block_logs(
    block_number: u64,
    rpc_url: &str,
) -> Result<Vec<ethers::core::types::Log>, Error> {
    backoff::future::retry(
        ExponentialBackoff {
            max_elapsed_time: Some(Duration::from_secs(10)),
            ..ExponentialBackoff::default()
        },
        || async {
            // create new logger
            let logger = Logger::default();

            debug_max!(&format!("fetching logs from node for block: '{}' .", &block_number));

            // create new provider
            let provider = match Provider::<Http>::try_from(rpc_url) {
                Ok(provider) => provider,
                Err(_) => {
                    logger.error(&format!("failed to connect to RPC provider '{}' .", &rpc_url));
                    return Err(backoff::Error::Permanent(()))
                }
            };

            // fetch the logs for the block
            let logs = provider
                .get_logs(&Filter {
                    block_option: FilterBlockOption::Range {
                        from_block: Some(BlockNumber::from(block_number)),
                        to_block: Some(BlockNumber::from(block_number)),
                    },
                    address: None,
                    topics: [None, None, None, None],
                })
                .await
                .map_err(|_| {
                    logger.error(&format!(
                        "failed to fetch logs for block '{}' . does your RPC provider support it?",
                        &block_number
                    ))
                })?;

            debug_max!("fetched logs for block '{}' .", &block_number);

            Ok(logs)
        },
    )
    .await
    .map_err(|_| Error::Generic(format!("failed to get logs for block: {:?}", &block_number)))
}

// TODO: add tests
#[cfg(test)]
pub mod tests {}
