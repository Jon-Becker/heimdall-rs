mod core;

use clap::{AppSettings, Parser};

use derive_builder::Builder;

use ethers::types::TransactionTrace;
use heimdall_common::{
    ether::rpc::{get_trace, get_transaction},
    utils::io::logging::Logger,
};

use crate::error::Error;

use self::core::{contracts::Contracts, tracing::DecodedTransactionTrace};

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

    /// Your OPTIONAL Transpose.io API Key, used for labeling contract addresses.
    #[clap(long = "transpose-api-key", short, hide_default_value = true)]
    pub transpose_api_key: Option<String>,
}

impl InspectArgsBuilder {
    pub fn new() -> Self {
        Self {
            target: Some(String::new()),
            verbose: Some(clap_verbosity_flag::Verbosity::new(0, 1)),
            rpc_url: Some(String::new()),
            default: Some(true),
            transpose_api_key: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct InspectResult {
    pub decoded_trace: Option<DecodedTransactionTrace>,
}
/// The entrypoint for the inspect module. This function will analyze the given transaction and
/// provide a detailed inspection of the transaction, including calldata & trace decoding, log
/// visualization, and more.
#[allow(deprecated)]
pub async fn inspect(args: InspectArgs) -> Result<InspectResult, Error> {
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
    let (logger, _trace) = Logger::new(match args.verbose.log_level() {
        Some(level) => level.as_str(),
        None => "SILENT",
    });

    // get calldata from RPC
    let _transaction = get_transaction(&args.target, &args.rpc_url)
        .await
        .map_err(|e| Error::RpcError(e.to_string()))?;

    // get trace
    let block_trace =
        get_trace(&args.target, &args.rpc_url).await.map_err(|e| Error::RpcError(e.to_string()))?;

    let decoded_trace =
        match block_trace.trace {
            Some(trace) => <DecodedTransactionTrace as async_convert::TryFrom<
                Vec<TransactionTrace>,
            >>::try_from(trace)
            .await
            .ok(),
            None => None,
        };
    if decoded_trace.is_none() {
        logger.warn("no trace found for transaction");
    }

    // get contracts client and extend with addresses from trace
    let mut contracts = Contracts::new(&args);
    if let Some(decoded_trace) = decoded_trace.clone() {
        contracts
            .extend(decoded_trace.addresses(true, true).into_iter().collect())
            .await
            .map_err(|e| Error::GenericError(e.to_string()))?;
    };

    println!("{:#?}", contracts);

    Ok(InspectResult { decoded_trace })
}
