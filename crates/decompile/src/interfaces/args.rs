use clap::{AppSettings, Parser};
use derive_builder::Builder;
use heimdall_config::parse_url_arg;

#[derive(Debug, Clone, Parser, Builder)]
#[clap(
    about = "Decompiles EVM bytecode to human-readable representations",
    after_help = "For more information, read the wiki: https://jbecker.dev/r/heimdall-rs/wiki",
    global_setting = AppSettings::DeriveDisplayOrder,
    override_usage = "heimdall decompile <TARGET> [OPTIONS]"
)]
pub struct DecompilerArgs {
    /// The target to decompile, either a file, bytecode, contract address, or ENS name.
    #[clap(required = true)]
    pub target: String,

    /// The RPC provider to use for fetching target bytecode.
    /// This can be an explicit URL or a reference to a MESC endpoint.
    #[clap(long, short, parse(try_from_str = parse_url_arg), default_value = "", hide_default_value = true)]
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
}

impl DecompilerArgsBuilder {
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
        }
    }
}
