use alloy::{
    primitives::{Address, Bytes, B256},
    rpc::types::Log,
};
use async_convert::{async_trait, TryFrom};
use heimdall_common::{
    ether::signatures::{ResolveSelector, ResolvedLog},
    utils::hex::ToLowerHex,
};
use serde::{Deserialize, Serialize};
use std::future::Future;
use tracing::{trace, warn};

/// Represents a decoded log
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DecodedLog {
    /// H160. the contract that emitted the log
    pub address: Address,

    /// topics: Array of 0 to 4 32 Bytes of indexed log arguments.
    /// (In solidity: The first topic is the hash of the signature of the event
    /// (e.g. `Deposit(address,bytes32,uint256)`), except you declared the event
    /// with the anonymous specifier.)
    pub topics: Vec<B256>,

    /// Data
    pub data: Bytes,

    /// Resolved Event
    #[serde(rename = "resolvedEvent")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved_event: Option<ResolvedLog>,

    /// Block Hash
    #[serde(rename = "blockHash")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block_hash: Option<B256>,

    /// Block Number
    #[serde(rename = "blockNumber")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block_number: Option<u64>,

    /// Transaction Hash
    #[serde(rename = "transactionHash")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transaction_hash: Option<B256>,

    /// Transaction Index
    #[serde(rename = "transactionIndex")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transaction_index: Option<u64>,

    /// Integer of the log index position in the block. None if it's a pending log.
    #[serde(rename = "logIndex")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub log_index: Option<u64>,

    /// True when the log was removed, due to a chain reorganization.
    /// false if it's a valid log.
    pub removed: bool,
}

#[async_trait]
impl TryFrom<Log> for DecodedLog {
    type Error = eyre::Report;

    async fn try_from(value: Log) -> Result<Self, Self::Error> {
        Self::decode_with_options(value, false).await
    }
}

impl DecodedLog {
    pub(crate) async fn decode_with_options(
        value: Log,
        skip_resolving: bool,
    ) -> Result<Self, eyre::Report> {
        Self::decode_with_resolver(value, skip_resolving, |signature| async move {
            ResolvedLog::resolve(&signature).await
        })
        .await
    }

    async fn decode_with_resolver<F, Fut>(
        value: Log,
        skip_resolving: bool,
        resolve: F,
    ) -> Result<Self, eyre::Report>
    where
        F: FnOnce(String) -> Fut,
        Fut: Future<Output = Result<Option<Vec<ResolvedLog>>, eyre::Report>>, {
        let resolved_logs = if !skip_resolving {
            let signature = match value.topics().first() {
                Some(topic) => {
                    let topic = topic.to_lower_hex();
                    Some(topic)
                }
                None => None,
            };

            match signature {
                Some(signature) => {
                    trace!("resolving signature: {}", signature.to_string().to_lowercase());
                    match resolve(signature.clone()).await {
                        Ok(results) => results.unwrap_or_default(),
                        Err(error) => {
                            warn!("failed to resolve event signature {signature}: {error}");
                            Vec::new()
                        }
                    }
                }
                None => Vec::new(),
            }
        } else {
            Vec::new()
        };

        Ok(Self {
            address: value.address(),
            topics: value.topics().to_vec(),
            data: value.data().clone().data,
            block_hash: value.block_hash,
            block_number: value.block_number,
            transaction_hash: value.transaction_hash,
            transaction_index: value.transaction_index,
            log_index: value.log_index,
            removed: value.removed,
            resolved_event: resolved_logs.first().cloned(),
        })
    }
}

#[cfg(test)]
mod option_tests {
    use super::*;

    #[tokio::test]
    async fn log_decoding_survives_signature_service_errors() {
        let topic = B256::from([0x42; 32]);
        let log = Log {
            inner: alloy::primitives::Log::new_unchecked(Address::ZERO, vec![topic], Bytes::new()),
            ..Default::default()
        };
        let decoded = DecodedLog::decode_with_resolver(log, false, |_| async {
            Err(eyre::eyre!("signature service unavailable"))
        })
        .await
        .unwrap();
        assert_eq!(decoded.topics, [topic]);
        assert!(decoded.resolved_event.is_none());
    }

    #[tokio::test]
    async fn log_decoding_skips_signature_resolution_when_requested() {
        let topic = B256::from([0x42; 32]);
        let log = Log {
            inner: alloy::primitives::Log::new_unchecked(Address::ZERO, vec![topic], Bytes::new()),
            ..Default::default()
        };
        let decoded = DecodedLog::decode_with_options(log, true).await.unwrap();
        assert_eq!(decoded.topics, [topic]);
        assert!(decoded.resolved_event.is_none());
    }
}
