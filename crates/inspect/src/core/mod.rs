use alloy::{consensus::Transaction, network::TransactionResponse, primitives::TxHash};
use eyre::eyre;
use futures::future::try_join_all;
use std::{collections::VecDeque, time::Instant};
use tracing::{debug, info, trace, warn};

use heimdall_common::{
    ether::{
        rpc::{get_block_logs, get_trace, get_transaction},
        signatures::cache_signatures_from_abi,
    },
    utils::{hex::ToLowerHex, io::logging::TraceFactory},
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
    let start_time = Instant::now();

    if let Some(abi_path) = args.abi.as_ref() {
        cache_signatures_from_abi(abi_path.into())
            .map_err(|e| Error::Eyre(eyre!("caching signatures from ABI failed: {}", e)))?;
    }

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

    let start_fetch_time = Instant::now();
    let block_trace = get_trace(&args.target, &args.rpc_url)
        .await
        .map_err(|e| Error::Eyre(eyre!("fetching block trace failed: {}", e)))?;
    debug!("fetching block trace took {:?}", start_fetch_time.elapsed());

    let start_fetch_time = Instant::now();
    let transaction_logs = get_block_logs(block_number, &args.rpc_url)
        .await
        .map_err(|e| Error::Eyre(eyre!("fetching block logs failed: {}", e)))?
        .into_iter()
        .filter(|log| log.transaction_hash == Some(transaction.tx_hash()))
        .collect::<Vec<_>>();
    debug!("fetching transaction logs took {:?}", start_fetch_time.elapsed());

    let decode_log_time = Instant::now();
    let handles = transaction_logs
        .into_iter()
        .map(|log| DecodedLog::decode_with_options(log, args.skip_resolving));
    let mut decoded_logs = try_join_all(handles).await?;
    decoded_logs
        .sort_by(|a, b| a.log_index.unwrap_or_default().cmp(&b.log_index.unwrap_or_default()));
    let mut decoded_logs = VecDeque::from(decoded_logs);
    info!("decoded {} logs successfully", decoded_logs.len());
    debug!("decoding logs took {:?}", decode_log_time.elapsed());

    let _start_decode_time = Instant::now();
    let mut decoded_trace =
        DecodedTransactionTrace::decode_with_options(block_trace.trace, args.skip_resolving)
            .await?;

    trace!("resolving address contract labels");

    let mut contracts = Contracts::new(&args);
    contracts
        .extend(decoded_trace.addresses(true, true).into_iter().collect())
        .await
        .map_err(|e| Error::Eyre(eyre!("fetching contracts failed: {}", e)))?;

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
        if let Err(error) = decoded_trace.join_logs(&mut decoded_logs, &vm_trace, Vec::new()).await
        {
            warn!("failed to join all logs to the trace: {error}");
        }
        if let Err(error) = decoded_trace.build_state_diffs(vm_trace, Vec::new()).await {
            warn!("failed to attach all state diffs to the trace: {error}");
        }
    } else {
        warn!("no vm trace found for transaction. skipping joining logs");
    }

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
