use alloy::primitives::Address;
use clap::Parser;
use eyre::Result;
use heimdall_common::ether::bytecode::get_bytecode_from_target;
use heimdall_config::parse_url_arg;
use heimdall_vm::core::hardfork::HardFork;

#[derive(Debug, Clone, Parser)]
#[clap(
    about = "Disassembles EVM bytecode to assembly",
    after_help = "For more information, read the wiki: https://jbecker.dev/r/heimdall-rs/wiki",
    override_usage = "heimdall disassemble <TARGET> [OPTIONS]"
)]
/// Arguments for the disassembly operation
///
/// This struct contains all the configuration parameters needed to disassemble
/// a contract's bytecode into human-readable assembly.
pub struct DisassemblerArgs {
    /// The target to disassemble, either a file, bytecode, contract address, or ENS name.
    #[clap(required = true)]
    pub target: String,

    /// The RPC provider to use for fetching target bytecode.
    /// This can be an explicit URL or a reference to a MESC endpoint.
    #[clap(long, short, value_parser = parse_url_arg, default_value = "", hide_default_value = true)]
    pub rpc_url: String,

    /// Whether to use base-10 for the program counter.
    #[clap(long = "decimal-counter", short = 'd')]
    pub decimal_counter: bool,

    /// Name of the output file.
    #[clap(long, short, default_value = "", hide_default_value = true)]
    pub name: String,

    /// The output directory to write the output to or 'print' to print to the console
    #[clap(long = "output", short = 'o', default_value = "output", hide_default_value = true)]
    pub output: String,

    /// The hardfork to use for opcode recognition. Opcodes introduced after this hardfork
    /// will be shown as 'unknown'. Defaults to 'latest'.
    #[clap(long, short = 'f', default_value = "latest")]
    pub hardfork: HardFork,

    /// Etherscan API key for fetching contract creation block.
    /// If provided, uses Etherscan API instead of binary search.
    #[clap(long, short = 'e', default_value = "", hide_default_value = true)]
    pub etherscan_api_key: String,
}

#[derive(Debug, Clone)]
/// Builder for DisassemblerArgs
///
/// This struct provides a builder pattern for creating DisassemblerArgs instances
/// with a fluent API.
pub struct DisassemblerArgsBuilder {
    /// The target to disassemble, either a file, bytecode, contract address, or ENS name.
    target: Option<String>,

    /// The RPC provider to use for fetching target bytecode.
    rpc_url: Option<String>,

    /// Whether to use base-10 for the program counter.
    decimal_counter: Option<bool>,

    /// Name of the output file.
    name: Option<String>,

    /// The output directory to write the output to or 'print' to print to the console
    output: Option<String>,

    /// The hardfork to use for opcode recognition.
    hardfork: Option<HardFork>,

    /// Etherscan API key for fetching contract creation block.
    etherscan_api_key: Option<String>,
}

impl DisassemblerArgs {
    /// Retrieves the bytecode for the specified target
    ///
    /// This method fetches the bytecode from a file, address, or directly from a hex string,
    /// depending on the target type provided in the arguments.
    ///
    /// # Returns
    /// The raw bytecode as a vector of bytes
    pub async fn get_bytecode(&self) -> Result<Vec<u8>> {
        get_bytecode_from_target(&self.target, &self.rpc_url, "").await
    }

    /// Gets the hardfork to use for disassembly.
    ///
    /// If `hardfork` is set to `Auto`, attempts to detect the hardfork based on the
    /// contract's creation block. If detection fails, falls back to `Latest`.
    ///
    /// If `etherscan_api_key` is provided and the chain is supported, uses Etherscan API
    /// for faster creation block lookup. Otherwise, uses binary search via RPC.
    pub async fn get_hardfork(&self) -> HardFork {
        if self.hardfork != HardFork::Auto {
            return self.hardfork;
        }

        // Try to auto-detect hardfork from creation block
        match self.detect_hardfork_from_creation_block().await {
            Some(fork) => fork,
            None => HardFork::Latest,
        }
    }

    /// Attempts to detect the hardfork based on the contract's creation block.
    ///
    /// Returns `None` if the target is not a contract address, no RPC URL is available,
    /// or the creation block cannot be determined.
    async fn detect_hardfork_from_creation_block(&self) -> Option<HardFork> {
        // Need RPC URL to get chain_id and creation block
        if self.rpc_url.is_empty() {
            return None;
        }

        // Try to parse target as an address
        let address: Address = self.target.parse().ok()?;

        // Get the chain_id first (needed for both etherscan and hardfork detection)
        let chain_id = heimdall_common::ether::rpc::chain_id(&self.rpc_url).await.ok()?;

        // Try to get the creation block
        let creation_block = self.get_creation_block(address, chain_id).await?;

        // Determine hardfork from chain and block number
        // Note: For post-merge forks we don't have timestamp, so this will return Paris
        // for post-merge blocks. This is conservative and safe for opcode recognition.
        Some(HardFork::from_chain(chain_id, creation_block, None))
    }

    /// Gets the creation block for a contract address.
    ///
    /// Uses Etherscan API if available and supported, otherwise falls back to binary search.
    async fn get_creation_block(&self, address: Address, chain_id: u64) -> Option<u64> {
        // If etherscan_api_key is provided and chain is supported, use Etherscan API
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

        // Fall back to binary search
        heimdall_common::ether::rpc::get_contract_creation_block(address, &self.rpc_url).await.ok()
    }
}

impl Default for DisassemblerArgsBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl DisassemblerArgsBuilder {
    /// Creates a new DisassemblerArgsBuilder with default values
    pub fn new() -> Self {
        Self {
            target: Some(String::new()),
            rpc_url: Some(String::new()),
            decimal_counter: Some(false),
            name: Some(String::new()),
            output: Some(String::new()),
            hardfork: Some(HardFork::Latest),
            etherscan_api_key: Some(String::new()),
        }
    }

    /// Sets the target for disassembly (address, file, or bytecode)
    pub fn target(&mut self, target: String) -> &mut Self {
        self.target = Some(target);
        self
    }

    /// Sets the RPC URL for fetching bytecode if the target is an address
    pub fn rpc_url(&mut self, rpc_url: String) -> &mut Self {
        self.rpc_url = Some(rpc_url);
        self
    }

    /// Sets whether to use decimal (true) or hexadecimal (false) for program counter
    pub fn decimal_counter(&mut self, decimal_counter: bool) -> &mut Self {
        self.decimal_counter = Some(decimal_counter);
        self
    }

    /// Sets the name for the output file
    pub fn name(&mut self, name: String) -> &mut Self {
        self.name = Some(name);
        self
    }

    /// Sets the output directory or 'print' to print to console
    pub fn output(&mut self, output: String) -> &mut Self {
        self.output = Some(output);
        self
    }

    /// Sets the hardfork for opcode recognition
    pub fn hardfork(&mut self, hardfork: HardFork) -> &mut Self {
        self.hardfork = Some(hardfork);
        self
    }

    /// Sets the Etherscan API key for fetching contract creation block
    pub fn etherscan_api_key(&mut self, etherscan_api_key: String) -> &mut Self {
        self.etherscan_api_key = Some(etherscan_api_key);
        self
    }

    /// Builds the DisassemblerArgs from the builder
    ///
    /// # Returns
    /// A Result containing the built DisassemblerArgs or an error if required fields are missing
    pub fn build(&self) -> eyre::Result<DisassemblerArgs> {
        Ok(DisassemblerArgs {
            target: self.target.clone().ok_or_else(|| eyre::eyre!("target is required"))?,
            rpc_url: self.rpc_url.clone().ok_or_else(|| eyre::eyre!("rpc_url is required"))?,
            decimal_counter: self
                .decimal_counter
                .ok_or_else(|| eyre::eyre!("decimal_counter is required"))?,
            name: self.name.clone().ok_or_else(|| eyre::eyre!("name is required"))?,
            output: self.output.clone().ok_or_else(|| eyre::eyre!("output is required"))?,
            hardfork: self.hardfork.ok_or_else(|| eyre::eyre!("hardfork is required"))?,
            etherscan_api_key: self
                .etherscan_api_key
                .clone()
                .ok_or_else(|| eyre::eyre!("etherscan_api_key is required"))?,
        })
    }
}
