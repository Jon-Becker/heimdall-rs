use clap::{AppSettings, Parser};


#[derive(Debug, Clone, Parser)]
#[clap(about = "Decompile EVM bytecode to Solidity",
       after_help = "For more information, read the wiki: https://jbecker.dev/r/heimdall-rs/wiki",
       global_setting = AppSettings::DeriveDisplayOrder, 
       override_usage = "heimdall decompile <TARGET> [OPTIONS]")]
pub struct DecompilerArgs {
    
    /// The target to decompile, either a file, contract address, or ENS name.
    #[clap(required=true)]
    target: String,

    /// Set the output verbosity level, 1 - 5.
    #[clap(flatten)]
    verbose: clap_verbosity_flag::Verbosity,
    
    /// The output directory to write the decompiled files to
    #[clap(long="output", short, default_value = "./output/", hide_default_value = true)]
    output: String,

    /// The RPC provider to use for fetching target bytecode.
    #[clap(long="rpc-url", short, default_value = "http://localhost:8545", hide_default_value = true)]
    rpc_url: String,

    /// When prompted, always select the default value.
    #[clap(long, short)]
    default: bool,

}
