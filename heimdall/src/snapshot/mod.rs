use clap::{AppSettings, Parser};
use heimdall_common::io::logging::*;
#[derive(Debug, Clone, Parser)]
#[clap(about = "Infer function information from bytecode, including access control, gas consumption, storage accesses, event emissions, and more",
       after_help = "For more information, read the wiki: https://jbecker.dev/r/heimdall-rs/wiki",
       global_setting = AppSettings::DeriveDisplayOrder,
       override_usage = "heimdall snapshot <TARGET> [OPTIONS]")]
pub struct SnapshotArgs {
    /// The target to analyze. This may be a file, bytecode, or contract address.
    #[clap(required = true)]
    pub target: String,

    /// Set the output verbosity level, 1 - 5.
    #[clap(flatten)]
    pub verbose: clap_verbosity_flag::Verbosity,

    /// The RPC provider to use for fetching target bytecode.
    #[clap(long = "rpc-url", short, default_value = "", hide_default_value = true)]
    pub rpc_url: String,

    /// When prompted, always select the default value.
    #[clap(long, short)]
    pub default: bool,
}

pub fn snapshot(args: SnapshotArgs) {
    use std::time::Instant;
    let now = Instant::now();

    let (logger, mut trace) = Logger::new(match args.verbose.log_level() {
        Some(level) => level.as_str(),
        None => "SILENT",
    });
}
