use alloy::primitives::Address;
use clap::Parser;
use derive_builder::Builder;
use eyre::Result;
use heimdall_common::ether::bytecode::get_bytecode_from_target;
use heimdall_config::parse_url_arg;
use heimdall_vm::core::hardfork::HardFork;

/// Arguments for the CFG subcommand
#[derive(Debug, Clone, Parser, Builder)]
#[clap(
    about = "Generate a visual control flow graph for EVM bytecode",
    after_help = "For more information, read the wiki: https://jbecker.dev/r/heimdall-rs/wiki",
    override_usage = "heimdall cfg <TARGET> [OPTIONS]"
)]
pub struct CfgArgs {
    /// The target to generate a Cfg for, either a file, bytecode, contract address, or ENS name.
    #[clap(required = true)]
    pub target: String,

    /// The RPC provider to use for fetching target bytecode.
    /// This can be an explicit URL or a reference to a MESC endpoint.
    #[clap(long, short, value_parser = parse_url_arg, default_value = "", hide_default_value = true)]
    pub rpc_url: String,

    /// When prompted, always select the default value.
    #[clap(long, short)]
    pub default: bool,

    /// Color the edges of the graph based on the JUMPI condition.
    /// This is useful for visualizing the flow of if statements.
    #[clap(long = "color-edges", short)]
    pub color_edges: bool,

    /// The output directory to write the output to or 'print' to print to the console
    #[clap(long = "output", short = 'o', default_value = "output", hide_default_value = true)]
    pub output: String,

    /// The name for the output file
    #[clap(long, short, default_value = "", hide_default_value = true)]
    pub name: String,

    /// Timeout for symbolic execution
    #[clap(long, short, default_value = "10000", hide_default_value = true)]
    pub timeout: u64,

    /// The hardfork to use for opcode recognition. Opcodes introduced after this hardfork
    /// will be treated as unknown. Defaults to 'latest'.
    #[clap(long, short = 'f', default_value = "latest")]
    pub hardfork: HardFork,

    /// Etherscan API key for fetching contract creation block when using auto hardfork detection.
    #[clap(long, short = 'e', default_value = "", hide_default_value = true)]
    pub etherscan_api_key: String,
}

impl CfgArgs {
    /// Get the bytecode for the target
    pub async fn get_bytecode(&self) -> Result<Vec<u8>> {
        get_bytecode_from_target(&self.target, &self.rpc_url, "").await
    }

    /// Gets the hardfork to use for CFG generation.
    ///
    /// If `hardfork` is set to `Auto`, attempts to detect the hardfork based on the
    /// contract's creation block. If detection fails, falls back to `Latest`.
    pub async fn get_hardfork(&self) -> HardFork {
        if self.hardfork != HardFork::Auto {
            return self.hardfork;
        }

        match self.detect_hardfork_from_creation_block().await {
            Some(fork) => fork,
            None => HardFork::Latest,
        }
    }

    /// Attempts to detect the hardfork based on the contract's creation block.
    async fn detect_hardfork_from_creation_block(&self) -> Option<HardFork> {
        if self.rpc_url.is_empty() {
            return None;
        }

        let address: Address = self.target.parse().ok()?;
        let chain_id = heimdall_common::ether::rpc::chain_id(&self.rpc_url).await.ok()?;
        let creation_block = self.get_creation_block(address, chain_id).await?;

        Some(HardFork::from_chain(chain_id, creation_block, None))
    }

    /// Gets the creation block for a contract address.
    async fn get_creation_block(&self, address: Address, chain_id: u64) -> Option<u64> {
        if !self.etherscan_api_key.is_empty() &&
            heimdall_common::ether::etherscan::is_supported_chain(chain_id)
        {
            if let Ok(block) = heimdall_common::ether::etherscan::get_contract_creation_block(
                address,
                &self.rpc_url,
                chain_id,
                &self.etherscan_api_key,
            )
            .await
            {
                return Some(block);
            }
        }

        heimdall_common::ether::rpc::get_contract_creation_block(address, &self.rpc_url).await.ok()
    }
}

impl CfgArgsBuilder {
    /// Create a new instance of the [`CfgArgsBuilder`]
    pub fn new() -> Self {
        Self {
            target: Some(String::new()),
            rpc_url: Some(String::new()),
            default: Some(true),
            color_edges: Some(false),
            output: Some(String::new()),
            name: Some(String::new()),
            timeout: Some(10000),
            hardfork: Some(HardFork::Latest),
            etherscan_api_key: Some(String::new()),
        }
    }
}
