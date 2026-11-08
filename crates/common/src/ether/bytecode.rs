//! Functions for working with Ethereum bytecode.

use crate::utils::strings::decode_hex;

use super::{etherscan::get_creation_bytecode, rpc::get_code};
use alloy::primitives::{bytes::Bytes, Address};
use eyre::{eyre, Result};
use std::fs;
use tracing::{debug, info, warn};

/// Classify a bytecode target as a contract address when an RPC URL is configured.
///
/// Without an RPC URL, an address-length hex value is treated as raw bytecode.
pub fn contract_address_target(target: &str, rpc_url: &str) -> Option<Address> {
    if rpc_url.is_empty() {
        None
    } else {
        target.parse().ok()
    }
}

/// Given a target, return bytecode of the target.
///
/// This function supports multiple input types:
/// - Contract address: fetches bytecode via RPC, or creation bytecode for self-destructed contracts
/// - Raw bytecode: hex-encoded bytecode string (with or without 0x prefix)
/// - File path: path to a file containing hex-encoded bytecode
///
/// For self-destructed contracts, if an Etherscan API key is configured and the chain is supported,
/// this function will attempt to fetch the creation bytecode from the deployment transaction.
pub async fn get_bytecode_from_target(
    target: &str,
    rpc_url: &str,
    etherscan_api_key: &str,
) -> Result<Vec<u8>> {
    if let Some(address) = contract_address_target(target, rpc_url) {
        let bytecode = get_code(address, rpc_url).await?;
        if !bytecode.is_empty() {
            return Ok(bytecode);
        }

        // Bytecode is empty - this could be a self-destructed contract
        debug!(
            "contract bytecode is empty at address {}. checking for self-destructed contract",
            address
        );

        if !etherscan_api_key.is_empty() {
            if let Ok(chain_id) = super::rpc::chain_id(rpc_url).await {
                info!(
                    "attempting to fetch creation bytecode for self-destructed contract {}",
                    address
                );
                match get_creation_bytecode(address, rpc_url, chain_id, etherscan_api_key).await {
                    Ok(creation_bytecode) => {
                        info!(
                            "successfully fetched creation bytecode ({} bytes)",
                            creation_bytecode.len()
                        );
                        return Ok(creation_bytecode);
                    }
                    Err(e) => {
                        warn!("failed to fetch creation bytecode: {}", e);
                    }
                }
            }
        }

        return Ok(bytecode);
    }

    if let Ok(bytecode) = decode_hex(target) {
        return Ok(bytecode);
    }

    match fs::read_to_string(target) {
        Ok(contents) => {
            let cleaned_contents = contents.replace('\n', "");
            decode_hex(&cleaned_contents)
                .map_err(|_| eyre!("invalid target: file does not contain valid bytecode"))
        }
        Err(_) => Err(eyre!("invalid target")),
    }
}

/// Removes pushed bytes from the bytecode, leaving only the instructions
/// themselves.
///
/// For example:
///   0x6060 (PUSH1 0x60) would become 0x60 (PUSH1).
///   0x60806040 (PUSH1 0x60 PUSH1 0x40) would become 0x60 0x60 (PUSH1 PUSH1).
pub fn remove_pushbytes_from_bytecode(bytecode: alloy::primitives::Bytes) -> Result<Bytes> {
    let push_range = 0x5f..=0x7f;
    let mut pruned = Vec::new();

    let mut i = 0;
    while i < bytecode.len() {
        pruned.push(bytecode[i]);

        if push_range.contains(&bytecode[i]) {
            i += bytecode[i] as usize - 0x5f + 1;
        } else {
            i += 1;
        }
    }

    Ok(Bytes::from(pruned))
}

#[cfg(test)]
mod tests {
    use alloy::hex::FromHex;

    use super::*;
    use std::fs;

    #[test]
    fn test_remove_pushbytes_from_bytecode() {
        let bytecode = alloy::primitives::Bytes::from_hex("0x6040").expect("invalid");
        let pruned = remove_pushbytes_from_bytecode(bytecode).unwrap();
        assert_eq!(pruned.to_vec(), alloy::primitives::Bytes::from_hex("0x60").expect("invalid"));

        let bytecode = alloy::primitives::Bytes::from_hex("0x60406080").expect("invalid");
        let pruned = remove_pushbytes_from_bytecode(bytecode).unwrap();
        assert_eq!(pruned.to_vec(), alloy::primitives::Bytes::from_hex("0x6060").expect("invalid"));

        let bytecode = alloy::primitives::Bytes::from_hex(
            "0x604060807f2222222222222222222222222222222222222222222222222222222222222222",
        )
        .expect("invalid");
        let pruned = remove_pushbytes_from_bytecode(bytecode).unwrap();
        assert_eq!(
            pruned.to_vec(),
            alloy::primitives::Bytes::from_hex("0x60607f").expect("invalid")
        );
    }

    #[test]
    fn bytecode_address_classification_requires_rpc_and_an_address() {
        let target = "0x0000000000000000000000000000000000000001";
        assert_eq!(contract_address_target(target, ""), None);
        assert_eq!(contract_address_target(target, "rpc"), Some(target.parse().unwrap()));
        assert_eq!(contract_address_target("0x6000", "rpc"), None);
    }

    #[tokio::test]
    async fn address_rpc_errors_do_not_become_bytecode() {
        let target = "0x0000000000000000000000000000000000000001";
        assert!(get_bytecode_from_target(target, "://invalid-rpc", "").await.is_err());
        assert_eq!(get_bytecode_from_target(target, "", "").await.unwrap().len(), 20);
    }

    #[tokio::test]
    #[ignore = "requires a live Ethereum RPC_URL and, for creation bytecode, ETHERSCAN_API_KEY"]
    async fn test_get_bytecode_when_target_is_address() {
        let rpc_url = std::env::var("RPC_URL").expect("RPC_URL is required for this live test");

        let bytecode =
            get_bytecode_from_target("0x9f00c43700bc0000Ff91bE00841F8e04c0495000", &rpc_url, "")
                .await
                .expect("failed to get bytecode from target");

        assert!(!bytecode.is_empty());
    }

    #[tokio::test]
    async fn test_get_bytecode_when_target_is_bytecode() {
        let rpc_url = String::new();

        let bytecode = get_bytecode_from_target(
            include_str!("../../../core/tests/testdata/common/zero_bytecode.hex"),
            &rpc_url,
            "",
        )
        .await
        .expect("failed to get bytecode from target");

        assert!(!bytecode.is_empty());
    }

    #[tokio::test]
    async fn test_get_bytecode_when_target_is_file_path() {
        let rpc_url = String::new();

        let directory = tempfile::tempdir().unwrap();
        let file_path = directory.path().join("bytecode.hex");
        let mock_bytecode = include_str!("../../../core/tests/testdata/common/zero_bytecode.hex");

        fs::write(&file_path, mock_bytecode).expect("failed to write mock bytecode to file");

        let bytecode = get_bytecode_from_target(file_path.to_str().unwrap(), &rpc_url, "")
            .await
            .expect("failed to get bytecode from target");

        assert_eq!(bytecode.len(), 52);
    }

    #[tokio::test]
    #[ignore = "requires a live Ethereum RPC_URL and, for creation bytecode, ETHERSCAN_API_KEY"]
    async fn test_get_creation_bytecode_for_self_destructed_contract() {
        let rpc_url = std::env::var("RPC_URL").expect("RPC_URL is required for this live test");
        let etherscan_api_key = std::env::var("ETHERSCAN_API_KEY")
            .expect("ETHERSCAN_API_KEY is required for this live test");

        // Self-destructed contract from yETH hack
        // https://etherscan.io/address/0xadbe952ebb9b3e247261d2e3b96835f00f721f8e
        let self_destructed_address = "0xadbe952ebb9b3e247261d2e3b96835f00f721f8e";

        let bytecode =
            get_bytecode_from_target(self_destructed_address, &rpc_url, &etherscan_api_key)
                .await
                .expect("failed to get creation bytecode for self-destructed contract");

        assert!(!bytecode.is_empty());
    }

    #[tokio::test]
    #[ignore = "requires a live Ethereum RPC_URL and, for creation bytecode, ETHERSCAN_API_KEY"]
    async fn test_get_bytecode_returns_empty_for_self_destructed_without_api_key() {
        let rpc_url = std::env::var("RPC_URL").expect("RPC_URL is required for this live test");

        let self_destructed_address = "0xadbe952ebb9b3e247261d2e3b96835f00f721f8e";

        let bytecode = get_bytecode_from_target(self_destructed_address, &rpc_url, "")
            .await
            .expect("failed to get bytecode");

        assert!(bytecode.is_empty());
    }
}
