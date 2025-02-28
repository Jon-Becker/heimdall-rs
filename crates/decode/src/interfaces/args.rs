use clap::Parser;
use derive_builder::Builder;
use eyre::Result;
use heimdall_common::ether::calldata::get_calldata_from_target;
use heimdall_config::parse_url_arg;

#[derive(Debug, Clone, Parser, Builder)]
#[clap(
    about = "Decodes raw/arbitrary calldata into readable types",
    after_help = "For more information, read the wiki: https://jbecker.dev/r/heimdall-rs/wiki",
    override_usage = "heimdall decode <TARGET> [OPTIONS]"
)]
/// Arguments for the decode operation
///
/// This struct contains all the configuration parameters needed to decode
/// calldata into human-readable function signatures and parameters.
pub struct DecodeArgs {
    /// The target to decode, either a transaction hash or string of bytes.
    #[clap(required = true)]
    pub target: String,

    /// The RPC provider to use for fetching target calldata.
    /// This can be an explicit URL or a reference to a MESC endpoint.
    #[clap(long, short, value_parser = parse_url_arg, default_value = "", hide_default_value = true)]
    pub rpc_url: String,

    /// Your OpenAI API key, used for explaining calldata.
    #[clap(long, default_value = "", hide_default_value = true)]
    pub openai_api_key: String,

    /// Whether to explain the decoded calldata using OpenAI.
    #[clap(long)]
    pub explain: bool,

    /// When prompted, always select the default value.
    #[clap(long, short)]
    pub default: bool,

    /// Whether constructor bytecode has been provided.
    #[clap(long, short)]
    pub constructor: bool,

    /// Whether to truncate nonstandard sized calldata.
    #[clap(long, short)]
    pub truncate_calldata: bool,

    /// Whether to skip resolving selectors. Heimdall will attempt to guess types.
    #[clap(long = "skip-resolving")]
    pub skip_resolving: bool,

    /// Whether to treat the target as a raw calldata string. Useful if the target is exactly 32
    /// bytes.
    #[clap(long)]
    pub raw: bool,

    /// Path to an optional ABI file to use for resolving errors, functions, and events.
    #[clap(long, short, default_value = None, hide_default_value = true)]
    pub abi: Option<String>,

    /// The output directory to write the output to or 'print' to print to the console
    #[clap(long = "output", short = 'o', default_value = "print", hide_default_value = true)]
    pub output: String,
}

impl DecodeArgs {
    /// Retrieves the calldata from the specified target
    ///
    /// This method fetches the calldata from a transaction hash, raw hex string,
    /// or directly from the provided target, depending on the configuration options.
    ///
    /// # Returns
    /// The raw calldata as a vector of bytes
    pub async fn get_calldata(&self) -> Result<Vec<u8>> {
        get_calldata_from_target(&self.target, self.raw, &self.rpc_url).await
    }
}

impl DecodeArgsBuilder {
    /// Creates a new DecodeArgsBuilder with default values
    pub fn new() -> Self {
        Self {
            target: Some(String::new()),
            rpc_url: Some(String::new()),
            openai_api_key: Some(String::new()),
            explain: Some(false),
            default: Some(true),
            constructor: Some(false),
            truncate_calldata: Some(false),
            skip_resolving: Some(false),
            raw: Some(false),
            abi: Some(None),
            output: Some(String::from("print")),
        }
    }
}
