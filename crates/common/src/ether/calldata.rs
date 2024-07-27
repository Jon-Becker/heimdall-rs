use super::rpc::get_transaction;
use crate::utils::strings::decode_hex;
use alloy::primitives::TxHash;
use eyre::{bail, eyre, Result};

/// Given a target, return calldata of the target.
pub async fn get_calldata_from_target(target: &str, rpc_url: &str) -> Result<Vec<u8>> {
    // If the target is a transaction hash, fetch the calldata from the RPC provider.
    if let Ok(address) = target.parse::<TxHash>() {
        return get_transaction(address, rpc_url)
            .await
            .map(|tx| tx.input.to_vec())
            .map_err(|_| eyre!("failed to fetch transaction from RPC provider"));
    }

    // If the target is not a transaction hash, it could be calldata.
    if let Ok(calldata) = decode_hex(target) {
        return Ok(calldata);
    }

    bail!("invalid target: must be a transaction hash or calldata (bytes)");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_calldata_when_target_is_txhash() {
        let rpc_url = std::env::var("RPC_URL").unwrap_or_else(|_| {
            println!("RPC_URL not set, skipping test");
            std::process::exit(0);
        });

        let calldata = get_calldata_from_target(
            "0x317907eeece00619fd4418c18a4ec4ebe5c87cdbff808f4b01cc2c6384799837",
            &rpc_url,
        )
        .await
        .expect("failed to get calldata from target");

        assert!(!calldata.is_empty());
    }

    #[tokio::test]
    async fn test_get_calldata_when_target_is_calldata() {
        let rpc_url = std::env::var("RPC_URL").unwrap_or_else(|_| {
            println!("RPC_URL not set, skipping test");
            std::process::exit(0);
        });

        let calldata = get_calldata_from_target(
            "0xf14fcbc8bf9eac48d61719f80efb268ef1099a248fa332ed639041337954647ec6583f2e",
            &rpc_url,
        )
        .await
        .expect("failed to get calldata from target");

        assert!(!calldata.is_empty());
    }

    #[tokio::test]
    async fn test_get_calldata_when_target_is_neither() {
        let rpc_url = std::env::var("RPC_URL").unwrap_or_else(|_| {
            println!("RPC_URL not set, skipping test");
            std::process::exit(0);
        });

        let calldata =
            get_calldata_from_target("asfnsdalkfasdlfnlasdkfnalkdsfndaskljfnasldkjfnasf", &rpc_url)
                .await;

        assert!(calldata.is_err());
    }
}
