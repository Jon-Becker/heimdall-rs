//! Shared hardfork selection for bytecode analysis.

use alloy::primitives::Address;
use heimdall_common::ether::{etherscan, provider::MultiTransportProvider, rpc};

use crate::core::hardfork::HardFork;

/// Resolve an explicitly selected hardfork, or infer it from a contract's creation block.
///
/// Auto detection falls back to `Latest` when the target or RPC cannot be resolved.
/// If the block timestamp cannot be fetched, post-merge detection is conservative.
pub async fn resolve_hardfork(
    hardfork: HardFork,
    target: &str,
    rpc_url: &str,
    etherscan_api_key: &str,
) -> HardFork {
    if hardfork != HardFork::Auto {
        return hardfork;
    }

    detect_hardfork(target, rpc_url, etherscan_api_key).await.unwrap_or(HardFork::Latest)
}

async fn detect_hardfork(target: &str, rpc_url: &str, etherscan_api_key: &str) -> Option<HardFork> {
    if rpc_url.is_empty() {
        return None;
    }
    let address: Address = target.parse().ok()?;
    let chain_id = rpc::chain_id(rpc_url).await.ok()?;
    let block = if !etherscan_api_key.is_empty() && etherscan::is_supported_chain(chain_id) {
        etherscan::get_contract_creation_block(address, rpc_url, chain_id, etherscan_api_key)
            .await
            .ok()
    } else {
        None
    };
    let creation_block = match block {
        Some(block) => block,
        None => rpc::get_contract_creation_block(address, rpc_url).await.ok()?,
    };
    let timestamp = match MultiTransportProvider::connect(rpc_url).await {
        Ok(provider) => provider.get_block_timestamp(creation_block).await.ok().flatten(),
        Err(_) => None,
    };
    Some(HardFork::from_chain(chain_id, creation_block, timestamp))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn explicit_selection_does_not_require_a_target_or_rpc() {
        assert_eq!(resolve_hardfork(HardFork::Berlin, "", "", "").await, HardFork::Berlin);
    }

    #[tokio::test]
    async fn auto_selection_without_a_resolvable_target_uses_latest() {
        assert_eq!(resolve_hardfork(HardFork::Auto, "6000", "", "").await, HardFork::Latest);
        assert_eq!(
            resolve_hardfork(HardFork::Auto, "not-an-address", "http://localhost", "").await,
            HardFork::Latest
        );
    }
}
