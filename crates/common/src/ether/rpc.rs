//! RPC utilities for interacting with Ethereum nodes

use crate::ether::provider::MultiTransportProvider;
use alloy::{
    eips::{BlockId, BlockNumberOrTag},
    primitives::{keccak256, Address, TxHash},
    rpc::types::{
        trace::parity::{TraceResults, TraceResultsWithTransactionHash, TraceType},
        Filter, FilterBlockOption, FilterSet, Log, Transaction,
    },
};
use eyre::{bail, OptionExt, Result};
use heimdall_cache::with_cache;
use tokio_retry::{strategy::ExponentialBackoff, Retry};

fn chain_id_cache_key(rpc_url: &str) -> String {
    format!("chain_id.{}", keccak256(rpc_url.as_bytes()))
}

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
        with_cache(&chain_id_cache_key(rpc_url), || async {
            let provider = MultiTransportProvider::connect(rpc_url).await?;
            provider.get_chainid().await
        })
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
    if rpc_url.is_empty() {
        bail!("cannot get_code, rpc_url is empty");
    }

    let chain_id = chain_id(rpc_url).await?;
    Retry::spawn(ExponentialBackoff::from_millis(50).take(2), || async {
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

/// Get the block number at which a contract was created using binary search.
///
/// This function performs a binary search to find the earliest block at which
/// the contract code exists at the given address.
///
/// ```no_run
/// use heimdall_common::ether::rpc::get_contract_creation_block;
/// use alloy::primitives::address;
///
/// // let block = get_contract_creation_block(address!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"), "https://eth.llamarpc.com").await?;
/// ```
pub async fn get_contract_creation_block(contract_address: Address, rpc_url: &str) -> Result<u64> {
    if rpc_url.is_empty() {
        bail!("cannot get_contract_creation_block, rpc_url is empty");
    }

    let chain_id = chain_id(rpc_url).await?;

    with_cache(&format!("contract_creation_block.{}.{}", chain_id, contract_address), || async {
        let provider = MultiTransportProvider::connect(rpc_url).await?;

        let latest_code = provider.get_code_at(contract_address).await?;
        if latest_code.is_empty() {
            bail!("contract does not exist at address {}", contract_address);
        }

        let latest_block = provider.get_block_number().await?;

        let mut low: u64 = 0;
        let mut high: u64 = latest_block;

        while low < high {
            let mid = low + (high - low) / 2;

            let code =
                provider.get_code_at_block(contract_address, BlockId::Number(mid.into())).await?;

            if code.is_empty() {
                low = mid + 1;
            } else {
                high = mid;
            }
        }

        Ok(low)
    })
    .await
}

/// Tests for RPC functionality.
#[cfg(test)]
pub mod tests {
    use alloy::{network::TransactionResponse, primitives::address};

    use crate::{ether::rpc::*, utils::hex::ToLowerHex};

    fn isolated_live_test(name: &str) -> bool {
        const CHILD: &str = "HEIMDALL_LIVE_RPC_TEST_CHILD";
        if std::env::var(CHILD).as_deref() == Ok(name) {
            return true;
        }
        let home = std::env::temp_dir().join(format!(
            "heimdall-live-rpc-{}-{}",
            std::process::id(),
            name.replace("::", "-")
        ));
        std::fs::create_dir_all(&home).unwrap();
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--ignored", "--exact", name])
            .env(CHILD, name)
            .env("HOME", &home)
            .status()
            .unwrap();
        std::fs::remove_dir_all(home).unwrap();
        assert!(status.success());
        false
    }

    #[test]
    fn unknown_chain_does_not_read_mainnet_cache() {
        const CHILD: &str = "HEIMDALL_RPC_CACHE_TEST_CHILD";
        if std::env::var_os(CHILD).is_some() {
            let address = address!("1234567890123456789012345678901234567890");
            heimdall_cache::store_cache(&format!("contract.1.{address}"), vec![0x60u8, 0x00], None)
                .unwrap();
            let runtime = tokio::runtime::Runtime::new().unwrap();
            assert!(runtime.block_on(get_code(address, "http://127.0.0.1:0")).is_err());
            return;
        }
        let home = std::env::temp_dir().join(format!("heimdall-rpc-cache-{}", std::process::id()));
        std::fs::create_dir_all(&home).unwrap();
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "ether::rpc::tests::unknown_chain_does_not_read_mainnet_cache"])
            .env(CHILD, "1")
            .env("HOME", &home)
            .status()
            .unwrap();
        std::fs::remove_dir_all(home).unwrap();
        assert!(status.success());
    }

    #[test]
    fn rpc_urls_with_distinct_punctuation_have_distinct_chain_keys() {
        assert_ne!(
            chain_id_cache_key("http://example.test/a-b"),
            chain_id_cache_key("http://example.test/a:b")
        );
    }

    #[tokio::test]
    #[ignore = "requires a configured RPC endpoint"]
    async fn test_chain_id() {
        if !isolated_live_test("ether::rpc::tests::test_chain_id") {
            return;
        }
        let rpc_url = std::env::var("RPC_URL").expect("RPC_URL must be set for this test");

        let rpc_chain_id = chain_id(&rpc_url).await.expect("chain_id() returned an error!");

        assert_eq!(rpc_chain_id, 1);
    }

    #[tokio::test]
    #[ignore = "requires a configured RPC endpoint"]
    async fn test_get_code() {
        if !isolated_live_test("ether::rpc::tests::test_get_code") {
            return;
        }
        let rpc_url = std::env::var("RPC_URL").expect("RPC_URL must be set for this test");

        let contract_address = address!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2");
        let bytecode =
            get_code(contract_address, &rpc_url).await.expect("get_code() returned an error!");

        assert!(!bytecode.is_empty());
    }

    #[tokio::test]
    #[ignore = "requires a configured RPC endpoint"]
    async fn test_get_transaction() {
        if !isolated_live_test("ether::rpc::tests::test_get_transaction") {
            return;
        }
        let rpc_url = std::env::var("RPC_URL").expect("RPC_URL must be set for this test");

        let transaction_hash = "0x9a5f4ef7678a94dd87048eeec931d30af21b1f4cecbf7e850a531d2bb64a54ac";
        let transaction = get_transaction(transaction_hash.parse().expect("invalid"), &rpc_url)
            .await
            .expect("get_transaction() returned an error!");

        assert_eq!(transaction.tx_hash().to_lower_hex(), transaction_hash);
    }

    #[tokio::test]
    #[ignore = "requires a configured RPC endpoint"]
    async fn test_get_trace() {
        if !isolated_live_test("ether::rpc::tests::test_get_trace") {
            return;
        }
        let rpc_url = std::env::var("RPC_URL").expect("RPC_URL must be set for this test");

        let transaction_hash = "0x9a5f4ef7678a94dd87048eeec931d30af21b1f4cecbf7e850a531d2bb64a54ac";
        let trace = get_trace(transaction_hash, &rpc_url).await;

        assert!(trace.is_ok())
    }

    #[tokio::test]
    #[ignore = "requires a configured RPC endpoint"]
    async fn test_get_block_logs() {
        if !isolated_live_test("ether::rpc::tests::test_get_block_logs") {
            return;
        }
        let rpc_url = std::env::var("RPC_URL").expect("RPC_URL must be set for this test");

        let block_number = 18_000_000;
        let logs = get_block_logs(block_number, &rpc_url)
            .await
            .expect("get_block_logs() returned an error!");

        assert!(!logs.is_empty());
    }

    #[tokio::test]
    #[ignore = "requires a configured RPC endpoint"]
    async fn test_get_contract_creation_block() {
        if !isolated_live_test("ether::rpc::tests::test_get_contract_creation_block") {
            return;
        }
        let rpc_url = std::env::var("RPC_URL").expect("RPC_URL must be set for this test");

        // WETH contract on mainnet, deployed at block 4719568
        let contract_address = address!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2");
        let creation_block = get_contract_creation_block(contract_address, &rpc_url)
            .await
            .expect("get_contract_creation_block() returned an error!");

        assert_eq!(creation_block, 4719568);
    }

    #[tokio::test]
    #[ignore = "requires a configured RPC endpoint"]
    async fn test_chain_id_with_ws_rpc() {
        if !isolated_live_test("ether::rpc::tests::test_chain_id_with_ws_rpc") {
            return;
        }
        let rpc_url = std::env::var("WS_RPC_URL").expect("WS_RPC_URL must be set for this test");

        let rpc_chain_id = chain_id(&rpc_url).await.expect("chain_id() returned an error!");

        assert_eq!(rpc_chain_id, 42161);
    }
}
