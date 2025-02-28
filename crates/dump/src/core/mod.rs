use alloy::{
    primitives::{Address, FixedBytes},
    rpc::types::trace::parity::Delta,
};
use eyre::eyre;
use futures::future::try_join_all;
use hashbrown::HashMap;
use heimdall_common::{
    ether::rpc::{get_block_state_diff, latest_block_number},
    utils::time::{calculate_eta, format_eta},
};

use std::{sync::Arc, time::Instant};
use tokio::sync::{Mutex, Semaphore};
use tracing::{debug, info};

use crate::{error::Error, interfaces::DumpArgs};

/// Dumps the storage slots for a contract
///
/// This function retrieves storage slots from a contract by analyzing state differences
/// across multiple blocks. It uses parallel processing to efficiently handle large block ranges.
///
/// # Arguments
///
/// * `args` - Configuration parameters for the dump operation
///
/// # Returns
///
/// A HashMap containing the storage slots (keys) and their values
pub async fn dump(args: DumpArgs) -> Result<HashMap<FixedBytes<32>, FixedBytes<32>>, Error> {
    let start_time = Instant::now();
    let storage = Arc::new(Mutex::new(HashMap::new()));
    let completed_count = Arc::new(Mutex::new(0));
    let target =
        args.target.parse::<Address>().map_err(|e| eyre!("invalid target address: {e}"))?;

    // build block range
    let start_block = args.from_block;
    let to_block = match args.to_block {
        Some(to_block) => to_block,
        None => latest_block_number(&args.rpc_url).await.map_err(|e| eyre!("rpc error: {e}"))?,
    };
    let block_range = start_block..=to_block;
    let block_count = block_range.end() - block_range.start() + 1;
    debug!("dumping storage from block range: {:?}", block_range);

    // a quick check to see if the rpc supports trace_ namespace
    // TODO: dump support via `debug_traceBlockByNumber` w/ prestateTracer as another option
    let _ = get_block_state_diff(
        (*block_range.start()).try_into().expect("block number overflow"),
        &args.rpc_url,
    )
    .await
    .map_err(|_| eyre!("failed to `trace_replayBlockTransactions`. does your rpc support it?"))?;

    // create a semaphore with the correct number of permits
    let semaphore = Arc::new(Semaphore::new(args.threads));
    let handles = block_range.map(|block_number| {
        let semaphore = semaphore.clone();
        let storage = storage.clone();
        let args = args.clone();
        let completed_count = completed_count.clone();
        tokio::spawn(async move {
            let _permit = semaphore.acquire().await.expect("failed to acquire semaphore permit");
            let block_trace = get_block_state_diff(block_number as u64, &args.rpc_url)
                .await
                .map_err(|e| eyre!("rpc error: {e}"))?;

            // update storage
            let mut storage = storage.lock().await;
            block_trace.iter().for_each(|trace| {
                if let Some(diff) = trace.full_trace.state_diff.as_ref() {
                    diff.0
                        .iter()
                        .filter(|(addr, _)| addr == &&target)
                        .flat_map(|(_, value)| value.storage.iter())
                        .for_each(|(slot, diff)| match diff {
                            Delta::Added(v) => {
                                storage.insert(*slot, v.to_owned());
                            }
                            Delta::Changed(v) => {
                                storage.insert(*slot, v.to);
                            }
                            Delta::Removed(_) => {
                                storage.remove(slot);
                            }
                            _ => {}
                        });
                }
            });

            // print progress
            let mut completed_count = completed_count.lock().await;
            *completed_count += 1;
            let remaining = block_count - *completed_count;
            let completed_per_second = *completed_count as f64 / start_time.elapsed().as_secs_f64();
            info!(
                "completed={}  remaining={}  eta={}",
                *completed_count,
                remaining,
                format_eta(calculate_eta(completed_per_second, remaining as usize))
            );
            Ok::<_, Error>(())
        })
    });

    // execute all the tasks
    try_join_all(handles).await.map_err(|e| eyre!("failed to join tasks: {e}"))?;

    debug!("storage dump took {:?}", start_time.elapsed());
    Ok(storage.to_owned().lock().await.to_owned())
}
