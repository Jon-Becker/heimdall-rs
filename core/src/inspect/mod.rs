mod core;

use std::collections::VecDeque;

use clap::{AppSettings, Parser};

use derive_builder::Builder;

use ethers::types::{Log, TransactionTrace, U256, U64};
use futures::future::try_join_all;
use heimdall_common::{
    debug_max,
    ether::rpc::{get_block_logs, get_trace, get_transaction},
    utils::{
        hex::ToLowerHex,
        io::logging::{Logger, TraceFactory},
    },
};

use crate::error::Error;

use self::core::{contracts::Contracts, logs::DecodedLog, tracing::DecodedTransactionTrace};

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

    /// Name for the output files.
    #[clap(long, short, default_value = "", hide_default_value = true)]
    pub name: String,

    /// The output directory to write the output to, or 'print' to print to the console.
    #[clap(long = "output", short = 'o', default_value = "output", hide_default_value = true)]
    pub output: String,
}

impl InspectArgsBuilder {
    pub fn new() -> Self {
        Self {
            target: Some(String::new()),
            verbose: Some(clap_verbosity_flag::Verbosity::new(0, 1)),
            rpc_url: Some(String::new()),
            default: Some(true),
            transpose_api_key: None,
            name: Some(String::new()),
            output: Some(String::from("output")),
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
    let transaction = get_transaction(&args.target, &args.rpc_url)
        .await
        .map_err(|e| Error::RpcError(e.to_string()))?;
    let block_number = transaction.block_number.unwrap_or(U64::zero()).as_u64();

    // get trace
    let block_trace =
        get_trace(&args.target, &args.rpc_url).await.map_err(|e| Error::RpcError(e.to_string()))?;

    // get logs for this transaction
    let transaction_logs = get_block_logs(block_number, &args.rpc_url)
        .await
        .map_err(|e| Error::RpcError(e.to_string()))?
        .into_iter()
        .filter(|log| log.transaction_hash == Some(transaction.hash))
        .collect::<Vec<_>>();

    // convert Vec<Log> to Vec<DecodedLog>
    let handles =
        transaction_logs.into_iter().map(<DecodedLog as async_convert::TryFrom<Log>>::try_from);

    debug_max!(&format!("resolving event signatures for {} logs", handles.len()));

    // sort logs by log index
    let mut decoded_logs = try_join_all(handles).await?;
    decoded_logs.sort_by(|a, b| {
        a.log_index.unwrap_or(U256::zero()).cmp(&b.log_index.unwrap_or(U256::zero()))
    });
    let mut decoded_logs = VecDeque::from(decoded_logs);

    let mut decoded_trace =
        match block_trace.trace {
            Some(trace) => <DecodedTransactionTrace as async_convert::TryFrom<
                Vec<TransactionTrace>,
            >>::try_from(trace)
            .await
            .ok(),
            None => None,
        };
    if let Some(decoded_trace) = decoded_trace.as_mut() {
        debug_max!("resolving address contract labels");

        // get contracts client
        let mut contracts = Contracts::new(&args);
        contracts
            .extend(decoded_trace.addresses(true, true).into_iter().collect())
            .await
            .map_err(|e| Error::GenericError(e.to_string()))?;

        // extend with addresses from state diff
        if let Some(state_diff) = block_trace.state_diff {
            contracts
                .extend(state_diff.0.keys().cloned().collect())
                .await
                .map_err(|e| Error::GenericError(e.to_string()))?;
        } else {
            logger
                .warn("no state diff found for transaction. skipping state diff label resolution");
        }

        debug_max!(&format!("joining {} decoded logs to trace", decoded_logs.len()));

        if let Some(vm_trace) = block_trace.vm_trace {
            // join logs to trace
            let _ = decoded_trace.join_logs(&mut decoded_logs, vm_trace, Vec::new()).await;
        } else {
            logger.warn("no vm trace found for transaction. skipping joining logs");
        }

        let mut trace = TraceFactory::default();
        let inspect_call = trace.add_call(
            0,
            transaction.gas.as_u32(),
            "heimdall".to_string(),
            "inspect".to_string(),
            vec![transaction.hash.to_lower_hex()],
            "()".to_string(),
        );

        decoded_trace.add_to_trace(&mut trace, inspect_call);

        trace.display();
    } else {
        logger.warn("no trace found for transaction");
    }

    Ok(InspectResult { decoded_trace })
}
