use ethers::types::{Log, TransactionTrace, U256, U64};
use eyre::eyre;
use futures::future::try_join_all;
use std::{collections::VecDeque, time::Instant};
use tracing::{debug, info, trace, warn};

use heimdall_common::{
    ether::rpc::{get_block_logs, get_trace, get_transaction},
    utils::{hex::ToLowerHex, io::logging::TraceFactory},
};

use crate::{
    error::Error,
    interfaces::{Contracts, DecodedLog, DecodedTransactionTrace, InspectArgs},
};

#[derive(Debug, Clone)]
pub struct InspectResult {
    pub decoded_trace: DecodedTransactionTrace,
    _trace: TraceFactory,
}

impl InspectResult {
    pub fn display(&self) {
        self._trace.display();
    }
}

pub async fn inspect(args: InspectArgs) -> Result<InspectResult, Error> {
    // init
    let start_time = Instant::now();

    // get calldata from RPC
    let start_fetch_time = Instant::now();
    let transaction = get_transaction(&args.target, &args.rpc_url)
        .await
        .map_err(|e| Error::Eyre(eyre!("fetching transaction failed: {}", e)))?;
    debug!("fetching transaction took {:?}", start_fetch_time.elapsed());

    let block_number = transaction.block_number.unwrap_or(U64::zero()).as_u64();

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
        .filter(|log| log.transaction_hash == Some(transaction.hash))
        .collect::<Vec<_>>();
    debug!("fetching transaction logs took {:?}", start_fetch_time.elapsed());

    // convert Vec<Log> to Vec<DecodedLog>
    let decode_log_time = Instant::now();
    let handles =
        transaction_logs.into_iter().map(<DecodedLog as async_convert::TryFrom<Log>>::try_from);
    let mut decoded_logs = try_join_all(handles).await?;
    decoded_logs.sort_by(|a, b| {
        a.log_index.unwrap_or(U256::zero()).cmp(&b.log_index.unwrap_or(U256::zero()))
    });
    let mut decoded_logs = VecDeque::from(decoded_logs);
    info!("decoded {} logs successfully", decoded_logs.len());
    debug!("decoding logs took {:?}", decode_log_time.elapsed());

    // convert Vec<TransactionTrace> to DecodedTransactionTrace
    let _start_decode_time = Instant::now();
    let mut decoded_trace =
        match block_trace.trace {
            Some(trace) => <DecodedTransactionTrace as async_convert::TryFrom<
                Vec<TransactionTrace>,
            >>::try_from(trace)
            .await
            .ok(),
            None => None,
        };

    let decoded_trace =
        decoded_trace.as_mut().ok_or(Error::Eyre(eyre!("no trace found for transaction")))?;
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
        let _ = decoded_trace.join_logs(&mut decoded_logs, vm_trace.clone(), Vec::new()).await;
        // build state diffs within trace
        let _ = decoded_trace.build_state_diffs(vm_trace, Vec::new()).await;
    } else {
        warn!("no vm trace found for transaction. skipping joining logs");
    }

    // build trace
    let mut trace = TraceFactory::default();
    let inspect_call = trace.add_call(
        0,
        transaction.gas.as_u32(),
        "heimdall".to_string(),
        "inspect".to_string(),
        vec![transaction.hash.to_lower_hex()],
        "()".to_string(),
    );
    decoded_trace.add_to_trace(&contracts, &mut trace, inspect_call);

    info!("decoded raw trace successfully");
    debug!("inspection took {:?}", start_time.elapsed());

    Ok(InspectResult { decoded_trace: decoded_trace.clone(), _trace: trace })
}
