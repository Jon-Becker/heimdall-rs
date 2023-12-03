use clap::{AppSettings, Parser};
use derive_builder::Builder;

use heimdall_common::{
    ether::rpc::{get_trace, get_transaction},
    utils::io::logging::Logger,
};

use crate::error::Error;

#[derive(Debug, Clone, Parser, Builder)]
#[clap(
    about = "Detailed inspection of Ethereum transactions, including calldata & trace decoding, log visualization, and more.",
    after_help = "For more information, read the wiki: https://jbecker.dev/r/heimdall-rs/wiki",
    global_setting = AppSettings::DeriveDisplayOrder,
    override_usage = "heimdall inspect <TARGET> [OPTIONS]"
)]
pub struct InspectArgs {
    /// The target transaction hash to inspect.
    #[clap(required = true)]
    pub target: String,

    /// Set the output verbosity level, 1 - 5.
    #[clap(flatten)]
    pub verbose: clap_verbosity_flag::Verbosity,

    /// The RPC provider to use for fetching target calldata.
    #[clap(long = "rpc-url", short, default_value = "", hide_default_value = true)]
    pub rpc_url: String,

    /// When prompted, always select the default value.
    #[clap(long, short)]
    pub default: bool,
}

impl InspectArgsBuilder {
    pub fn new() -> Self {
        Self {
            target: Some(String::new()),
            verbose: Some(clap_verbosity_flag::Verbosity::new(0, 1)),
            rpc_url: Some(String::new()),
            default: Some(true),
        }
    }
}

/// The entrypoint for the inspect module. This function will analyze the given transaction and
/// provide a detailed inspection of the transaction, including calldata & trace decoding, log
/// visualization, and more.
#[allow(deprecated)]
pub async fn inspect(args: InspectArgs) -> Result<(), Error> {
    // set logger environment variable if not already set
    // TODO: abstract this to a heimdall_common util
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var(
            "RUST_LOG",
            match args.verbose.log_level() {
                Some(level) => level.as_str(),
                None => "SILENT",
            },
        );
    }

    // get a new logger and trace
    let (_logger, _trace) = Logger::new(match args.verbose.log_level() {
        Some(level) => level.as_str(),
        None => "SILENT",
    });

    // get calldata from RPC
    let transaction = get_transaction(&args.target, &args.rpc_url)
        .await
        .map_err(|e| Error::RpcError(e.to_string()))?;

    // get trace
    let block_trace =
        get_trace(&args.target, &args.rpc_url).await.map_err(|e| Error::RpcError(e.to_string()))?;

    println!("transaction: {:?}", transaction);
    println!("output: {:?}", block_trace.output);
    println!("trace: {:#?}", block_trace.trace);
    println!("state diff: {:#?}", block_trace.state_diff);

    Ok(())
}
