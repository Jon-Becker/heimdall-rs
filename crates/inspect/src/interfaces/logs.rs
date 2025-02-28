use alloy::{
    primitives::{Address, Bytes, B256},
    rpc::types::Log,
};
use async_convert::{async_trait, TryFrom};
use heimdall_common::{
    ether::signatures::{ResolveSelector, ResolvedLog},
    utils::{env::get_env, hex::ToLowerHex},
};
use serde::{Deserialize, Serialize};
use tracing::trace;

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
        let mut resolved_logs = Vec::new();
        let skip_resolving = get_env("SKIP_RESOLVING")
            .unwrap_or_else(|| "false".to_string())
            .parse::<bool>()
            .unwrap_or(false);

        if !skip_resolving {
            let signature = match value.topics().first() {
                Some(topic) => {
                    let topic = topic.to_lower_hex();
                    Some(topic)
                }
                None => None,
            };

            resolved_logs = match signature {
                Some(signature) => {
                    trace!("resolving signature: {}", signature.to_string().to_lowercase());
                    ResolvedLog::resolve(&signature)
                        .await
                        .map_err(|e| eyre::eyre!("failed to resolve signature: {}", e.to_string()))?
                        .unwrap_or(Vec::new())
                }
                None => Vec::new(),
            };
        }

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
