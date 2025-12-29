use alloy::primitives::Address;
use clap::Parser;
use derive_builder::Builder;
use eyre::Result;
use heimdall_common::ether::bytecode::get_bytecode_from_target;
use heimdall_config::parse_url_arg;
use heimdall_vm::core::hardfork::HardFork;

#[derive(Debug, Clone, Parser, Builder)]
#[clap(
    about = "Decompiles EVM bytecode to human-readable representations",
    after_help = "For more information, read the wiki: https://jbecker.dev/r/heimdall-rs/wiki",
    override_usage = "heimdall decompile <TARGET> [OPTIONS]"
)]
/// Arguments for the decompile operation
///
/// This struct contains all the configuration parameters needed to decompile
/// bytecode into human-readable source code and ABI.
pub struct DecompilerArgs {
    /// The target to decompile, either a file, bytecode, contract address, or ENS name.
    #[clap(required = true)]
    pub target: String,

    /// The RPC provider to use for fetching target bytecode.
    /// This can be an explicit URL or a reference to a MESC endpoint.
    #[clap(long, short, value_parser = parse_url_arg, default_value = "", hide_default_value = true)]
    pub rpc_url: String,

    /// When prompted, always select the default value.
    #[clap(long, short)]
    pub default: bool,

    /// Whether to skip resolving function selectors.
    #[clap(long = "skip-resolving")]
    pub skip_resolving: bool,

    /// Whether to include solidity source code in the output (in beta).
    #[clap(long = "include-sol")]
    pub include_solidity: bool,

    /// Whether to include yul source code in the output (in beta).
    #[clap(long = "include-yul")]
    pub include_yul: bool,

    /// The output directory to write the output to or 'print' to print to the console
    #[clap(long = "output", short = 'o', default_value = "output", hide_default_value = true)]
    pub output: String,

    /// The name for the output file
    #[clap(long, short, default_value = "", hide_default_value = true)]
    pub name: String,

    /// The timeout for each function's symbolic execution in milliseconds.
    #[clap(long, short, default_value = "10000", hide_default_value = true)]
    pub timeout: u64,

    /// Path to an optional ABI file to use for resolving errors, functions, and events.
    #[clap(long, short, default_value = None, hide_default_value = true)]
    pub abi: Option<String>,

    /// Whether to post-process the output using a LLM.
    #[clap(long, short)]
    pub llm_postprocess: bool,

    /// Your OpenRouter API key, used for LLM post-processing.
    #[clap(long, default_value = "", hide_default_value = true)]
    pub openrouter_api_key: String,

    /// The model to use for LLM post-processing (e.g., "openai/gpt-4o-mini",
    /// "anthropic/claude-3-haiku").
    #[clap(long, short = 'm', default_value = "", hide_default_value = true)]
    pub model: String,

    /// Your Etherscan API key, used for fetching creation bytecode of self-destructed contracts.
    #[clap(long, default_value = "", hide_default_value = true)]
    pub etherscan_api_key: String,

    /// The hardfork to use for opcode recognition. Opcodes introduced after this hardfork
    /// will be treated as unknown. Defaults to 'latest'.
    #[clap(long, short = 'f', default_value = "latest")]
    pub hardfork: HardFork,
}

impl DecompilerArgs {
    /// Retrieves the bytecode for the specified target
    ///
    /// This method fetches the bytecode from a file, address, or directly from a hex string,
    /// depending on the target type provided in the arguments.
    ///
    /// For self-destructed contracts, if an Etherscan API key is configured and the chain is
    /// supported, this will attempt to fetch the creation bytecode from the deployment transaction.
    ///
    /// # Returns
    /// The raw bytecode as a vector of bytes
    pub async fn get_bytecode(&self) -> Result<Vec<u8>> {
        get_bytecode_from_target(&self.target, &self.rpc_url, &self.etherscan_api_key).await
    }

    /// Gets the hardfork to use for decompilation.
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

impl DecompilerArgsBuilder {
    /// Creates a new DecompilerArgsBuilder with default values
    pub fn new() -> Self {
        Self {
            target: Some(String::new()),
            rpc_url: Some(String::new()),
            default: Some(true),
            skip_resolving: Some(false),
            include_solidity: Some(false),
            include_yul: Some(false),
            output: Some(String::new()),
            name: Some(String::new()),
            timeout: Some(10000),
            abi: Some(None),
            llm_postprocess: Some(false),
            openrouter_api_key: Some(String::new()),
            model: Some(String::new()),
            etherscan_api_key: Some(String::new()),
            hardfork: Some(HardFork::Latest),
        }
    }
}
