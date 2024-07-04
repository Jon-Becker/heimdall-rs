use crate::{error::Error, ether::provider::MultiTransportProvider};
use alloy::rpc::types::Transaction;
use backoff::ExponentialBackoff;
use ethers::{
    core::types::Address,
    providers::{Middleware, Provider},
    types::{
        BlockNumber::{self},
        BlockTrace, Filter, FilterBlockOption, TraceType, H256,
    },
};
use heimdall_cache::{read_cache, store_cache, with_cache};
use std::{str::FromStr, time::Duration};
use tracing::{debug, error, trace};

/// Get the chainId of the provided RPC URL
///
/// ```no_run
/// use heimdall_common::ether::rpc::chain_id;
///
/// // let chain_id = chain_id("https://eth.llamarpc.com").await?;
/// // assert_eq!(chain_id, 1);
/// ```
pub async fn chain_id(rpc_url: &str) -> Result<u64, Error> {
    let provider = MultiTransportProvider::connect(&rpc_url)
        .await
        .map_err(|_| Error::RpcError(format!("failed to connect to provider '{}'", &rpc_url)))?;
    provider
        .get_chainid()
        .await
        .map_err(|e| Error::RpcError(format!("failed to get chain id: {e}")))
}

/// Get the latest block number of the provided RPC URL
///
/// ```no_run
/// use heimdall_common::ether::rpc::latest_block_number;
/// // let block_number = latest_block_number("https://eth.llamarpc.com").await?;
/// // assert!(block_number > 0);
/// ```
pub async fn latest_block_number(rpc_url: &str) -> Result<u128, Error> {
    let provider = MultiTransportProvider::connect(&rpc_url)
        .await
        .map_err(|_| Error::RpcError(format!("failed to connect to provider '{}'", &rpc_url)))?;
    provider
        .get_block_number()
        .await
        .map(|n| n as u128)
        .map_err(|e| Error::RpcError(format!("failed to get block number: {e}")))
}

/// Get the bytecode of the provided contract address
///
/// ```no_run
/// use heimdall_common::ether::rpc::get_code;
///
/// // let bytecode = get_code("0x0", "https://eth.llamarpc.com").await;
/// // assert!(bytecode.is_ok());
/// ```
pub async fn get_code(contract_address: &str, rpc_url: &str) -> Result<Vec<u8>, Error> {
    let provider = MultiTransportProvider::connect(&rpc_url)
        .await
        .map_err(|_| Error::RpcError(format!("failed to connect to provider '{}'", &rpc_url)))?;
    provider
        .get_code_at(contract_address)
        .await
        .map_err(|e| Error::RpcError(format!("failed to get account code: {e}")))
}

/// Get the raw transaction data of the provided transaction hash
///
/// ```no_run
/// use heimdall_common::ether::rpc::get_code;
///
/// // let bytecode = get_code("0x0", "https://eth.llamarpc.com").await;
/// // assert!(bytecode.is_ok());
/// ```
pub async fn get_transaction(transaction_hash: &str, rpc_url: &str) -> Result<Transaction, Error> {
    let provider = MultiTransportProvider::connect(&rpc_url)
        .await
        .map_err(|_| Error::RpcError(format!("failed to connect to provider '{}'", &rpc_url)))?;
    provider
        .get_transaction_by_hash(transaction_hash)
        .await
        .map_err(|e| Error::RpcError(format!("failed to get account code: {e}")))?
        .ok_or_else(|| Error::RpcError("transaction not found".to_string()))
}

/// Get the raw trace data of the provided transaction hash
///
/// ```no_run
/// use heimdall_common::ether::rpc::get_trace;
///
/// // let trace = get_trace("0x0", "https://eth.llamarpc.com").await;
/// // assert!(trace.is_ok());
/// ```
pub async fn get_trace(transaction_hash: &str, rpc_url: &str) -> Result<BlockTrace, Error> {
    backoff::future::retry(
        ExponentialBackoff {
            max_elapsed_time: Some(Duration::from_secs(10)),
            ..ExponentialBackoff::default()
        },
        || async {
            trace!("fetching trace from node for transaction: '{}' .",
                &transaction_hash);

            // create new provider
            let provider = match get_provider(rpc_url).await {
                Ok(provider) => provider,
                Err(_) => {
                    error!("failed to connect to RPC provider '{}' .", &rpc_url);
                    return Err(backoff::Error::Permanent(()))
                }
            };

            // safely unwrap the transaction hash
            let transaction_hash_hex = match H256::from_str(transaction_hash) {
                Ok(transaction_hash) => transaction_hash,
                Err(_) => {
                    error!(
                        "failed to parse transaction hash '{}' .",
                        &transaction_hash
                    );
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
                Err(_) => {
                    error!(
                        "failed to replay and trace transaction '{}' . does your RPC provider support it?",
                        &transaction_hash
                    );

                    return Err(backoff::Error::Transient { err: (), retry_after: None })
                }
            };

            trace!("fetched trace for transaction '{}' .", &transaction_hash);

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
            trace!("fetching logs from node for block: '{}' .", &block_number);

            // create new provider
            let provider = match get_provider(rpc_url).await {
                Ok(provider) => provider,
                Err(_) => {
                    error!("failed to connect to RPC provider '{}' .", &rpc_url);
                    return Err(backoff::Error::Permanent(()));
                }
            };

            // fetch the logs for the block
            let logs = match provider
                .get_logs(&Filter {
                    block_option: FilterBlockOption::Range {
                        from_block: Some(BlockNumber::from(block_number)),
                        to_block: Some(BlockNumber::from(block_number)),
                    },
                    address: None,
                    topics: [None, None, None, None],
                })
                .await
            {
                Ok(logs) => logs,
                Err(_) => {
                    error!(
                        "failed to fetch logs for block '{}' . does your RPC provider support it?",
                        &block_number
                    );
                    return Err(backoff::Error::Transient { err: (), retry_after: None });
                }
            };

            trace!("fetched logs for block '{}' .", &block_number);

            Ok(logs)
        },
    )
    .await
    .map_err(|_| Error::Generic(format!("failed to get logs for block: {:?}", &block_number)))
}

/// Get all traces for the given block number
///
/// ```no_run
/// use heimdall_common::ether::rpc::get_block_state_diff;
///
/// // let traces = get_block_state_diff(1, "https://eth.llamarpc.com").await;
/// // assert!(traces.is_ok());
/// ```
pub async fn get_block_state_diff(
    block_number: u64,
    rpc_url: &str,
) -> Result<Vec<BlockTrace>, Error> {
    backoff::future::retry(
        ExponentialBackoff {
            max_elapsed_time: Some(Duration::from_secs(10)),
            ..ExponentialBackoff::default()
        },
        || async {
            trace!("fetching traces from node for block: '{}' .", &block_number);

            // create new provider
            let provider = match get_provider(rpc_url).await {
                Ok(provider) => provider,
                Err(_) => {
                    error!("failed to connect to RPC provider '{}' .", &rpc_url);
                    return Err(backoff::Error::Permanent(()));
                }
            };

            // fetch the logs for the block
            let trace = match provider
                .trace_replay_block_transactions(BlockNumber::from(block_number), vec![TraceType::StateDiff])
                .await
            {
                Ok(trace) => trace,
                Err(_) => {
                    error!(
                        "failed to fetch traces for block '{}' . does your RPC provider support it?",
                        &block_number
                    );
                    return Err(backoff::Error::Transient { err: (), retry_after: None });
                }
            };

            trace!("fetched traces for block '{}' .", &block_number);

            Ok(trace)
        },
    )
    .await
    .map_err(|_| Error::Generic(format!("failed to get traces for block: {:?}", &block_number)))
}

#[cfg(test)]
pub mod tests {
    use crate::{ether::rpc::*, utils::hex::ToLowerHex};

    #[tokio::test]
    async fn test_chain_id() {
        let rpc_url = std::env::var("RPC_URL").unwrap_or_else(|_| {
            println!("RPC_URL not set, skipping test");
            std::process::exit(0);
        });

        let rpc_chain_id = chain_id(&rpc_url).await.expect("chain_id() returned an error!");

        assert_eq!(rpc_chain_id, 1);
    }

    #[tokio::test]
    async fn test_chain_id_invalid_rpc_url() {
        let rpc_url = "https://none.llamarpc.com";
        let rpc_chain_id = chain_id(rpc_url).await;

        assert!(rpc_chain_id.is_err())
    }

    #[tokio::test]
    async fn test_get_code() {
        let rpc_url = std::env::var("RPC_URL").unwrap_or_else(|_| {
            println!("RPC_URL not set, skipping test");
            std::process::exit(0);
        });

        let contract_address = "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2";
        let bytecode =
            get_code(contract_address, &rpc_url).await.expect("get_code() returned an error!");

        assert!(!bytecode.is_empty());
    }

    #[tokio::test]
    async fn test_get_code_invalid_contract_address() {
        let rpc_url = std::env::var("RPC_URL").unwrap_or_else(|_| {
            println!("RPC_URL not set, skipping test");
            std::process::exit(0);
        });

        let contract_address = "0x0";
        let bytecode = get_code(contract_address, &rpc_url).await;

        assert!(bytecode.is_err())
    }

    #[tokio::test]
    async fn test_get_transaction() {
        let rpc_url = std::env::var("RPC_URL").unwrap_or_else(|_| {
            println!("RPC_URL not set, skipping test");
            std::process::exit(0);
        });

        let transaction_hash = "0x9a5f4ef7678a94dd87048eeec931d30af21b1f4cecbf7e850a531d2bb64a54ac";
        let transaction = get_transaction(transaction_hash, &rpc_url)
            .await
            .expect("get_transaction() returned an error!");

        assert_eq!(transaction.hash.to_lower_hex(), transaction_hash);
    }

    #[tokio::test]
    async fn test_get_transaction_invalid_transaction_hash() {
        let rpc_url = std::env::var("RPC_URL").unwrap_or_else(|_| {
            println!("RPC_URL not set, skipping test");
            std::process::exit(0);
        });

        let transaction_hash = "0x0";
        let transaction = get_transaction(transaction_hash, &rpc_url).await;

        assert!(transaction.is_err())
    }

    #[tokio::test]
    async fn test_get_trace() {
        let rpc_url = std::env::var("RPC_URL").unwrap_or_else(|_| {
            println!("RPC_URL not set, skipping test");
            std::process::exit(0);
        });

        let transaction_hash = "0x9a5f4ef7678a94dd87048eeec931d30af21b1f4cecbf7e850a531d2bb64a54ac";
        let trace = get_trace(transaction_hash, &rpc_url).await;

        assert!(trace.is_ok())
    }

    #[tokio::test]
    async fn test_get_trace_invalid_transaction_hash() {
        let rpc_url = std::env::var("RPC_URL").unwrap_or_else(|_| {
            println!("RPC_URL not set, skipping test");
            std::process::exit(0);
        });

        let transaction_hash = "0x0";
        let trace = get_trace(transaction_hash, &rpc_url).await;

        assert!(trace.is_err())
    }

    #[tokio::test]
    async fn test_get_block_logs() {
        let rpc_url = std::env::var("RPC_URL").unwrap_or_else(|_| {
            println!("RPC_URL not set, skipping test");
            std::process::exit(0);
        });

        let block_number = 18_000_000;
        let logs = get_block_logs(block_number, &rpc_url)
            .await
            .expect("get_block_logs() returned an error!");

        assert!(!logs.is_empty());
    }

    #[tokio::test]
    async fn test_chain_id_with_ws_rpc() {
        let rpc_url = std::env::var("WS_RPC_URL").unwrap_or_else(|_| {
            println!("WS_RPC_URL not set, skipping test");
            std::process::exit(0);
        });

        let rpc_chain_id = chain_id(&rpc_url).await.expect("chain_id() returned an error!");

        assert_eq!(rpc_chain_id, 42161);
    }
}
