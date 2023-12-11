// TODO: impl decodedlog for log

use async_convert::{async_trait, TryFrom};
use ethers::types::{Address, Bytes, Log, H256, U256, U64};
use heimdall_common::{
    debug_max,
    ether::signatures::{ResolveSelector, ResolvedLog},
    utils::hex::ToLowerHex,
};
use serde::{Deserialize, Serialize};

/// Represents a decoded log
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DecodedLog {
    /// H160. the contract that emitted the log
    pub address: Address,

    /// topics: Array of 0 to 4 32 Bytes of indexed log arguments.
    /// (In solidity: The first topic is the hash of the signature of the event
    /// (e.g. `Deposit(address,bytes32,uint256)`), except you declared the event
    /// with the anonymous specifier.)
    pub topics: Vec<H256>,

    /// Data
    pub data: Bytes,

    /// Resolved Event
    #[serde(rename = "resolvedEvent")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved_event: Option<ResolvedLog>,

    /// Block Hash
    #[serde(rename = "blockHash")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block_hash: Option<H256>,

    /// Block Number
    #[serde(rename = "blockNumber")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block_number: Option<U64>,

    /// Transaction Hash
    #[serde(rename = "transactionHash")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transaction_hash: Option<H256>,

    /// Transaction Index
    #[serde(rename = "transactionIndex")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transaction_index: Option<U64>,

    /// Integer of the log index position in the block. None if it's a pending log.
    #[serde(rename = "logIndex")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub log_index: Option<U256>,

    /// Integer of the transactions index position log was created from.
    /// None when it's a pending log.
    #[serde(rename = "transactionLogIndex")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transaction_log_index: Option<U256>,

    /// Log Type
    #[serde(rename = "logType")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub log_type: Option<String>,

    /// True when the log was removed, due to a chain reorganization.
    /// false if it's a valid log.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub removed: Option<bool>,
}

#[async_trait]
impl TryFrom<Log> for DecodedLog {
    type Error = crate::error::Error;

    async fn try_from(value: Log) -> Result<Self, Self::Error> {
        let signature = match value.topics.first() {
            Some(topic) => {
                let topic = topic.to_lower_hex();
                Some(topic)
            }
            None => None,
        };

        let resolved_logs = match signature {
            Some(signature) => {
                debug_max!("resolving signature: {}", signature.to_string().to_lowercase());
                ResolvedLog::resolve(&signature).await.unwrap_or(Vec::new())
            }
            None => Vec::new(),
        };

        Ok(Self {
            address: value.address,
            topics: value.topics,
            data: value.data,
            block_hash: value.block_hash,
            block_number: value.block_number,
            transaction_hash: value.transaction_hash,
            transaction_index: value.transaction_index,
            log_index: value.log_index,
            transaction_log_index: value.transaction_log_index,
            log_type: value.log_type,
            removed: value.removed,
            resolved_event: resolved_logs.first().cloned(),
        })
    }
}
