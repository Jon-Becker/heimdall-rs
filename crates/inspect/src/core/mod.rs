use alloy::{
    consensus::Transaction,
    network::TransactionResponse,
    primitives::TxHash,
    rpc::types::{trace::parity::TransactionTrace, Log},
};
use eyre::eyre;
use futures::future::try_join_all;
use std::{collections::VecDeque, time::Instant};
use tracing::{debug, info, trace, warn};

use heimdall_common::{
    ether::{
        rpc::{get_block_logs, get_trace, get_transaction},
        signatures::cache_signatures_from_abi,
    },
    utils::{env::set_env, hex::ToLowerHex, io::logging::TraceFactory},
};

use crate::{
    error::Error,
    interfaces::{Contracts, DecodedLog, DecodedTransactionTrace, InspectArgs},
};

#[derive(Debug, Clone)]
/// Result of a successful inspect operation
///
/// Contains the decoded transaction trace with all function calls, logs,
/// and state changes, as well as a trace factory for displaying the result
/// in a formatted way.
pub struct InspectResult {
    /// The decoded transaction trace containing all the execution steps
    pub decoded_trace: DecodedTransactionTrace,
    _trace: TraceFactory,
}

impl InspectResult {
    /// Displays the decoded transaction trace in a formatted way
    pub fn display(&self) {
        self._trace.display();
    }
}

/// Inspects a transaction by decoding its trace and associated logs
///
/// This function retrieves transaction execution data from the blockchain and
/// decodes it into a human-readable format, showing function calls, events,
/// and state changes that occurred during the transaction's execution.
///
/// # Arguments
///
/// * `args` - Configuration parameters for the inspect operation
///
/// # Returns
///
/// An InspectResult containing the decoded transaction trace
pub async fn inspect(args: InspectArgs) -> Result<InspectResult, Error> {
    // init
    let start_time = Instant::now();
    set_env("SKIP_RESOLVING", &args.skip_resolving.to_string());

    // parse and cache signatures from the ABI, if provided
    if let Some(abi_path) = args.abi.as_ref() {
        cache_signatures_from_abi(abi_path.into())
            .map_err(|e| Error::Eyre(eyre!("caching signatures from ABI failed: {}", e)))?;
    }

    // get calldata from RPC
    let start_fetch_time = Instant::now();
    let transaction = get_transaction(
        args.target
            .parse::<TxHash>()
            .map_err(|_| eyre!("invalid transaction hash: '{}'", args.target))?,
        &args.rpc_url,
    )
    .await
    .map_err(|e| Error::Eyre(eyre!("fetching transaction failed: {}", e)))?;
    debug!("fetching transaction took {:?}", start_fetch_time.elapsed());

    let block_number = transaction.block_number.unwrap_or(0);

    // get block traces
    let start_fetch_time = Instant::now();
    let block_trace = get_trace(&args.target, &args.rpc_url)
        .await
        .map_err(|e| Error::Eyre(eyre!("fetching block trace failed: {}", e)))?;
    debug!("fetching block trace took {:?}", start_fetch_time.elapsed());

    // get transaction logs
    let start_fetch_time = Instant::now();
    let transaction_logs = get_block_logs(block_number, &args.rpc_url)
        .await
        .map_err(|e| Error::Eyre(eyre!("fetching block logs failed: {}", e)))?
        .into_iter()
        .filter(|log| log.transaction_hash == Some(transaction.tx_hash()))
        .collect::<Vec<_>>();
    debug!("fetching transaction logs took {:?}", start_fetch_time.elapsed());

    // convert Vec<Log> to Vec<DecodedLog>
    let decode_log_time = Instant::now();
    let handles =
        transaction_logs.into_iter().map(<DecodedLog as async_convert::TryFrom<Log>>::try_from);
    let mut decoded_logs = try_join_all(handles).await?;
    decoded_logs
        .sort_by(|a, b| a.log_index.unwrap_or_default().cmp(&b.log_index.unwrap_or_default()));
    let mut decoded_logs = VecDeque::from(decoded_logs);
    info!("decoded {} logs successfully", decoded_logs.len());
    debug!("decoding logs took {:?}", decode_log_time.elapsed());

    // convert Vec<TransactionTrace> to DecodedTransactionTrace
    let _start_decode_time = Instant::now();
    let mut decoded_trace = <DecodedTransactionTrace as async_convert::TryFrom<
        Vec<TransactionTrace>,
    >>::try_from(block_trace.trace)
    .await?;

    trace!("resolving address contract labels");

    // get contracts client
    let mut contracts = Contracts::new(&args);
    contracts
        .extend(decoded_trace.addresses(true, true).into_iter().collect())
        .await
        .map_err(|e| Error::Eyre(eyre!("fetching contracts failed: {}", e)))?;

    // extend with addresses from state diff
    if let Some(state_diff) = block_trace.state_diff {
        contracts
            .extend(state_diff.0.keys().cloned().collect())
            .await
            .map_err(|e| Error::Eyre(eyre!("fetching contracts failed: {}", e)))?;
    } else {
        warn!("no state diff found for transaction. skipping state diff label resolution");
    }

    trace!("joining {} decoded logs to trace", decoded_logs.len());

    if let Some(vm_trace) = block_trace.vm_trace {
        // join logs to trace
        let _ = decoded_trace.join_logs(&mut decoded_logs, &vm_trace, Vec::new()).await;
        // build state diffs within trace
        let _ = decoded_trace.build_state_diffs(vm_trace, Vec::new()).await;
    } else {
        warn!("no vm trace found for transaction. skipping joining logs");
    }

    // build trace
    let mut trace = TraceFactory::default();
    let inspect_call = trace.add_call(
        0,
        transaction.inner.gas_limit().try_into().unwrap_or_default(),
        "heimdall".to_string(),
        "inspect".to_string(),
        vec![transaction.tx_hash().to_lower_hex()],
        "()".to_string(),
    );
    decoded_trace.add_to_trace(&contracts, &mut trace, inspect_call);

    info!("decoded raw trace successfully");
    debug!("inspection took {:?}", start_time.elapsed());

    Ok(InspectResult { decoded_trace, _trace: trace })
}
