use clap::{AppSettings, Parser};
use derive_builder::Builder;
use heimdall_config::parse_url_arg;

#[derive(Debug, Clone, Parser, Builder)]
#[clap(about = "Disassembles EVM bytecode to assembly",
       after_help = "For more information, read the wiki: https://jbecker.dev/r/heimdall-rs/wiki",
       global_setting = AppSettings::DeriveDisplayOrder,
       override_usage = "heimdall disassemble <TARGET> [OPTIONS]")]
pub struct DisassemblerArgs {
    /// The target to disassemble, either a file, bytecode, contract address, or ENS name.
    #[clap(required = true)]
    pub target: String,

    /// The RPC provider to use for fetching target bytecode.
    /// This can be an explicit URL or a reference to a MESC endpoint.
    #[clap(long, short, parse(try_from_str = parse_url_arg), default_value = "", hide_default_value = true)]
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
}

impl DisassemblerArgsBuilder {
    pub fn new() -> Self {
        Self {
            target: Some(String::new()),
            rpc_url: Some(String::new()),
            decimal_counter: Some(false),
            name: Some(String::new()),
            output: Some(String::new()),
        }
    }
}
