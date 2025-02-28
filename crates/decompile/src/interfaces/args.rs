use clap::Parser;
use derive_builder::Builder;
use eyre::Result;
use heimdall_common::ether::bytecode::get_bytecode_from_target;
use heimdall_config::parse_url_arg;

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

    /// Your OpenAI API key, used for explaining calldata.
    #[clap(long, default_value = "", hide_default_value = true)]
    pub openai_api_key: String,
}

impl DecompilerArgs {
    /// Retrieves the bytecode for the specified target
    ///
    /// This method fetches the bytecode from a file, address, or directly from a hex string,
    /// depending on the target type provided in the arguments.
    ///
    /// # Returns
    /// The raw bytecode as a vector of bytes
    pub async fn get_bytecode(&self) -> Result<Vec<u8>> {
        get_bytecode_from_target(&self.target, &self.rpc_url).await
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
            openai_api_key: Some(String::new()),
        }
    }
}
