use super::rpc::get_transaction;
use crate::{
    constants::{CALLDATA_REGEX, TRANSACTION_HASH_REGEX},
    utils::strings::decode_hex,
    Error,
};

pub async fn get_calldata_from_target(target: &str, rpc_url: &str) -> Result<Vec<u8>, Error> {
    if TRANSACTION_HASH_REGEX.is_match(target).unwrap_or(false) {
        // Target is a contract address, so we need to fetch the bytecode from the RPC provider.
        let raw_transaction = get_transaction(target, rpc_url).await.map_err(|_| {
            Error::Generic("failed to fetch transaction from RPC provider.".to_string())
        })?;

        Ok(raw_transaction.input.to_vec())
    } else if CALLDATA_REGEX.is_match(target).unwrap_or(false) {
        Ok(decode_hex(target)?)
    } else {
        Err(Error::Generic(
            "invalid target. must be a transaction hash or calldata (bytes).".to_string(),
        ))
    }
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
