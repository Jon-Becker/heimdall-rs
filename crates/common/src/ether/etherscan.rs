//! Etherscan API utilities for fetching contract information.

use super::rpc::get_transaction;
use crate::constants::ETHERSCAN_SUPPORTED_CHAIN_IDS;
use alloy::{
    consensus::Transaction,
    primitives::{Address, TxHash},
};
use eyre::{eyre, Result};
use heimdall_cache::with_cache;
use serde::Deserialize;
use tracing::debug;

/// Etherscan API response for contract creation transaction lookup
#[derive(Debug, Deserialize)]
struct EtherscanContractCreationResponse {
    status: String,
    result: Option<Vec<EtherscanContractCreation>>,
}

/// Etherscan contract creation result entry
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EtherscanContractCreation {
    #[allow(dead_code)]
    contract_creator: String,
    tx_hash: String,
}

/// Check if the chain ID is supported by Etherscan V2 API
pub fn is_supported_chain(chain_id: u64) -> bool {
    ETHERSCAN_SUPPORTED_CHAIN_IDS.contains(&chain_id)
}

/// Fetch the contract creation transaction hash from Etherscan V2 API.
///
/// This is useful for self-destructed contracts where `eth_getCode` returns empty bytecode.
/// Uses the unified Etherscan V2 API endpoint which supports multiple chains.
async fn get_contract_creation_tx(
    address: Address,
    chain_id: u64,
    api_key: &str,
) -> Result<TxHash> {
    if !is_supported_chain(chain_id) {
        return Err(eyre!("etherscan API not supported for chain ID {}", chain_id));
    }

    // Use Etherscan V2 API - unified endpoint for all supported chains
    let url = format!(
        "https://api.etherscan.io/v2/api?chainid={}&module=contract&action=getcontractcreation&contractaddresses={}&apikey={}",
        chain_id, address, api_key
    );

    let response: EtherscanContractCreationResponse = reqwest::get(&url).await?.json().await?;

    if response.status != "1" {
        return Err(eyre!("etherscan API returned error status"));
    }

    let result = response.result.ok_or_else(|| eyre!("etherscan API returned no results"))?;

    let creation = result.first().ok_or_else(|| eyre!("no contract creation transaction found"))?;

    creation.tx_hash.parse().map_err(|_| eyre!("invalid transaction hash from etherscan"))
}

/// Fetch the creation bytecode from a contract's deployment transaction.
///
/// This function queries Etherscan to find the creation transaction, then fetches
/// the transaction input data which contains the creation bytecode.
pub async fn get_creation_bytecode(
    address: Address,
    rpc_url: &str,
    chain_id: u64,
    api_key: &str,
) -> Result<Vec<u8>> {
    with_cache(&format!("creation_bytecode.{}.{}", chain_id, address), || async {
        let tx_hash = get_contract_creation_tx(address, chain_id, api_key).await?;
        debug!("found creation transaction: {}", tx_hash);

        let transaction = get_transaction(tx_hash, rpc_url).await?;
        let input = transaction.inner.input();

        if input.is_empty() {
            return Err(eyre!("creation transaction has empty input data"));
        }

        Ok(input.to_vec())
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::{CHAIN_ID_ARBITRUM, CHAIN_ID_BASE, CHAIN_ID_ETHEREUM, CHAIN_ID_POLYGON};

    #[test]
    fn test_is_supported_chain() {
        assert!(is_supported_chain(CHAIN_ID_ETHEREUM));
        assert!(is_supported_chain(CHAIN_ID_POLYGON));
        assert!(is_supported_chain(CHAIN_ID_ARBITRUM));
        assert!(is_supported_chain(CHAIN_ID_BASE));
        assert!(!is_supported_chain(999999));
    }

    #[test]
    fn test_supported_chain_ids_array_length() {
        assert_eq!(ETHERSCAN_SUPPORTED_CHAIN_IDS.len(), 20);
    }
}
