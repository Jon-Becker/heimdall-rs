use clap::Parser;
use derive_builder::Builder;
use heimdall_config::parse_url_arg;

#[derive(Debug, Clone, Parser, Builder)]
#[clap(
    about = "Detailed inspection of Ethereum transactions, including calldata & trace decoding, log visualization, and more.",
    after_help = "For more information, read the wiki: https://jbecker.dev/r/heimdall-rs/wiki",
    override_usage = "heimdall inspect <TARGET> [OPTIONS]"
)]
/// Arguments for the inspect operation
///
/// This struct contains all the configuration parameters needed to inspect
/// a transaction and decode its trace, logs, and state changes.
pub struct InspectArgs {
    /// The target transaction hash to inspect.
    #[clap(required = true)]
    pub target: String,

    /// The RPC provider to use for fetching target calldata.
    /// This can be an explicit URL or a reference to a MESC endpoint.
    #[clap(long, short, value_parser = parse_url_arg, default_value = "", hide_default_value = true)]
    pub rpc_url: String,

    /// When prompted, always select the default value.
    #[clap(long, short)]
    pub default: bool,

    /// Your OPTIONAL Transpose.io API Key. Used to resolve contract labels.
    #[clap(long = "transpose-api-key", short, default_value = "", hide_default_value = true)]
    pub transpose_api_key: String,

    /// Name for the output files.
    #[clap(long, short, default_value = "", hide_default_value = true)]
    pub name: String,

    /// The output directory to write the output to, or 'print' to print to the console.
    #[clap(long = "output", short = 'o', default_value = "output", hide_default_value = true)]
    pub output: String,

    /// Whether to skip resolving function selectors and contract labels.
    #[clap(long = "skip-resolving")]
    pub skip_resolving: bool,

    /// Path to an optional ABI file to use for resolving errors, functions, and events.
    #[clap(long, short, default_value = None, hide_default_value = true)]
    pub abi: Option<String>,
}

impl InspectArgsBuilder {
    /// Creates a new InspectArgsBuilder with default values
    pub fn new() -> Self {
        Self {
            target: Some(String::new()),
            rpc_url: Some(String::new()),
            default: Some(true),
            transpose_api_key: Some(String::new()),
            name: Some(String::new()),
            output: Some(String::from("output")),
            skip_resolving: Some(false),
            abi: Some(None),
        }
    }
}
