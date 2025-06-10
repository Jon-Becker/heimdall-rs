//! RPC utilities for interacting with Ethereum nodes

use crate::ether::provider::MultiTransportProvider;
use alloy::{
    eips::BlockNumberOrTag,
    primitives::{Address, TxHash},
    rpc::types::{
        trace::parity::{TraceResults, TraceResultsWithTransactionHash, TraceType},
        Filter, FilterBlockOption, FilterSet, Log, Transaction,
    },
};
use eyre::{bail, OptionExt, Result};
use heimdall_cache::with_cache;
use tokio_retry::{strategy::ExponentialBackoff, Retry};

/// Get the chainId of the provided RPC URL
///
/// ```no_run
/// use heimdall_common::ether::rpc::chain_id;
///
/// // let chain_id = chain_id("https://eth.llamarpc.com").await?;
/// // assert_eq!(chain_id, 1);
/// ```
pub async fn chain_id(rpc_url: &str) -> Result<u64> {
    Retry::spawn(ExponentialBackoff::from_millis(50).take(2), || async {
        with_cache(
            &format!("chain_id.{}", &rpc_url.replace('/', "").replace(['.', ':'], "-")),
            || async {
                let provider = MultiTransportProvider::connect(rpc_url).await?;
                provider.get_chainid().await
            },
        )
        .await
    })
    .await
}

/// Get the latest block number of the provided RPC URL
///
/// ```no_run
/// use heimdall_common::ether::rpc::latest_block_number;
/// // let block_number = latest_block_number("https://eth.llamarpc.com").await?;
/// // assert!(block_number > 0);
/// ```
pub async fn latest_block_number(rpc_url: &str) -> Result<u128> {
    Retry::spawn(ExponentialBackoff::from_millis(50).take(2), || async {
        let provider = MultiTransportProvider::connect(rpc_url).await?;
        provider.get_block_number().await.map(|n| n as u128)
    })
    .await
}

/// Get the bytecode of the provided contract address
///
/// ```no_run
/// use heimdall_common::ether::rpc::get_code;
///
/// // let bytecode = get_code("0x0", "https://eth.llamarpc.com").await;
/// // assert!(bytecode.is_ok());
/// ```
pub async fn get_code(contract_address: Address, rpc_url: &str) -> Result<Vec<u8>> {
    // if rpc_url is empty, return an error
    if rpc_url.is_empty() {
        bail!("cannot get_code, rpc_url is empty");
    }

    Retry::spawn(ExponentialBackoff::from_millis(50).take(2), || async {
        let chain_id = chain_id(rpc_url).await.unwrap_or(1);
        with_cache(&format!("contract.{}.{}", &chain_id, &contract_address), || async {
            let provider = MultiTransportProvider::connect(rpc_url).await?;
            provider.get_code_at(contract_address).await
        })
        .await
    })
    .await
}

/// Get the raw transaction data of the provided transaction hash \
///
/// ```no_run
/// use heimdall_common::ether::rpc::get_code;
///
/// // let bytecode = get_code("0x0", "https://eth.llamarpc.com").await;
/// // assert!(bytecode.is_ok());
/// ```
///
/// Note: [`Transaction`] is un-cacheable
pub async fn get_transaction(transaction_hash: TxHash, rpc_url: &str) -> Result<Transaction> {
    Retry::spawn(ExponentialBackoff::from_millis(50).take(2), || async {
        let provider = MultiTransportProvider::connect(rpc_url).await?;
        provider
            .get_transaction_by_hash(transaction_hash)
            .await?
            .ok_or_eyre("transaction not found")
    })
    .await
}

/// Get the raw trace data of the provided transaction hash
///
/// ```no_run
/// use heimdall_common::ether::rpc::get_trace;
///
/// // let trace = get_trace("0x0", "https://eth.llamarpc.com").await;
/// // assert!(trace.is_ok());
/// ```
///
/// Note: [`TraceResults`] is un-cacheable
pub async fn get_trace(transaction_hash: &str, rpc_url: &str) -> Result<TraceResults> {
    Retry::spawn(ExponentialBackoff::from_millis(50).take(2), || async {
        let provider = MultiTransportProvider::connect(rpc_url).await?;
        provider
            .trace_replay_transaction(
                transaction_hash,
                &[TraceType::Trace, TraceType::VmTrace, TraceType::StateDiff],
            )
            .await
    })
    .await
}

/// Get all logs for the given block number
///
/// ```no_run
/// use heimdall_common::ether::rpc::get_block_logs;
///
/// // let logs = get_block_logs(1, "https://eth.llamarpc.com").await;
/// // assert!(logs.is_ok());
/// ```
///
/// Note: [`Log`] is un-cacheable
pub async fn get_block_logs(block_number: u64, rpc_url: &str) -> Result<Vec<Log>> {
    Retry::spawn(ExponentialBackoff::from_millis(50).take(2), || async {
        let provider = MultiTransportProvider::connect(rpc_url).await?;
        provider
            .get_logs(&Filter {
                block_option: FilterBlockOption::Range {
                    from_block: Some(BlockNumberOrTag::from(block_number)),
                    to_block: Some(BlockNumberOrTag::from(block_number)),
                },
                address: FilterSet::default(),
                topics: [
                    FilterSet::default(),
                    FilterSet::default(),
                    FilterSet::default(),
                    FilterSet::default(),
                ],
            })
            .await
    })
    .await
}

/// Get all traces for the given block number
///
/// ```no_run
/// use heimdall_common::ether::rpc::get_block_state_diff;
///
/// // let traces = get_block_state_diff(1, "https://eth.llamarpc.com").await;
/// // assert!(traces.is_ok());
/// ```
///
/// Note: [`TraceResultsWithTransactionHash`] is un-cacheable
pub async fn get_block_state_diff(
    block_number: u64,
    rpc_url: &str,
) -> Result<Vec<TraceResultsWithTransactionHash>> {
    Retry::spawn(ExponentialBackoff::from_millis(50).take(2), || async {
        let provider = MultiTransportProvider::connect(rpc_url).await?;
        provider.trace_replay_block_transactions(block_number, &[TraceType::StateDiff]).await
    })
    .await
}

/// Tests for RPC functionality.
#[cfg(test)]
pub mod tests {
    use alloy::{network::TransactionResponse, primitives::address};

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

        let contract_address = address!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2");
        let bytecode =
            get_code(contract_address, &rpc_url).await.expect("get_code() returned an error!");

        assert!(!bytecode.is_empty());
    }

    #[tokio::test]
    async fn test_get_transaction() {
        let rpc_url = std::env::var("RPC_URL").unwrap_or_else(|_| {
            println!("RPC_URL not set, skipping test");
            std::process::exit(0);
        });

        let transaction_hash = "0x9a5f4ef7678a94dd87048eeec931d30af21b1f4cecbf7e850a531d2bb64a54ac";
        let transaction = get_transaction(transaction_hash.parse().expect("invalid"), &rpc_url)
            .await
            .expect("get_transaction() returned an error!");

        assert_eq!(transaction.tx_hash().to_lower_hex(), transaction_hash);
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
