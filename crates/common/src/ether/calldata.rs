//! Module for fetching calldata from a target.
use super::rpc::get_transaction;
use crate::utils::strings::decode_hex;
use alloy::{consensus::Transaction, primitives::TxHash};
use eyre::{bail, eyre, Result};
/// Classify a calldata target as a transaction hash unless raw bytes were requested.
pub fn transaction_hash_target(target: &str, raw: bool) -> Option<TxHash> {
    if raw {
        None
    } else {
        target.parse().ok()
    }
}

/// Given a target, return calldata of the target.
pub async fn get_calldata_from_target(target: &str, raw: bool, rpc_url: &str) -> Result<Vec<u8>> {
    if let Some(hash) = transaction_hash_target(target, raw) {
        return get_transaction(hash, rpc_url)
            .await
            .map(|tx| tx.inner.input().to_vec())
            .map_err(|_| eyre!("failed to fetch transaction from RPC provider"));
    }

    if let Ok(calldata) = decode_hex(target) {
        return Ok(calldata);
    }

    bail!("invalid target: must be a transaction hash or calldata (bytes)");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calldata_hash_classification_respects_raw_input() {
        let target = "0x0000000000000000000000000000000000000000000000000000000000000000";
        assert_eq!(transaction_hash_target(target, false), Some(target.parse().unwrap()));
        assert_eq!(transaction_hash_target(target, true), None);
        assert_eq!(transaction_hash_target("0x6000", false), None);
    }

    #[tokio::test]
    #[ignore = "requires a configured Ethereum mainnet RPC_URL"]
    async fn test_get_calldata_when_target_is_txhash() {
        let rpc_url = std::env::var("RPC_URL").expect("RPC_URL must be set for this test");

        let calldata = get_calldata_from_target(
            "0x317907eeece00619fd4418c18a4ec4ebe5c87cdbff808f4b01cc2c6384799837",
            false,
            &rpc_url,
        )
        .await
        .expect("failed to get calldata from target");

        assert!(!calldata.is_empty());
    }

    #[tokio::test]
    async fn test_get_calldata_when_target_is_calldata() {
        let rpc_url = String::new();

        let calldata = get_calldata_from_target("0x68656c6c6f", false, &rpc_url)
            .await
            .expect("failed to get calldata from target");

        assert_eq!(calldata, b"hello");
    }

    #[tokio::test]
    async fn test_get_calldata_when_target_is_neither() {
        let rpc_url = String::new();

        let calldata = get_calldata_from_target(
            "asfnsdalkfasdlfnlasdkfnalkdsfndaskljfnasldkjfnasf",
            false,
            &rpc_url,
        )
        .await;

        assert!(calldata.is_err());
    }

    #[tokio::test]
    async fn test_get_calldata_when_target_is_calldata_that_is_exactly_32_bytes() {
        let rpc_url = String::new();

        let calldata = get_calldata_from_target(
            "0x317907eeece00619fd4418c18a4ec4ebe5c87cdbff808f4b01cc2c6384799837",
            true,
            &rpc_url,
        )
        .await
        .expect("failed to get calldata from target");

        assert!(calldata.len() == 32);
    }
}
