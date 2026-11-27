use alloy::{
    primitives::{Address, FixedBytes},
    rpc::types::trace::parity::Delta,
};
use eyre::eyre;
use futures::{stream, StreamExt, TryStreamExt};
use hashbrown::HashMap;
use heimdall_common::{
    ether::rpc::{get_block_state_diff, latest_block_number},
    utils::time::{calculate_eta, format_eta},
};
use std::{future::Future, ops::RangeInclusive, time::Instant};
use tracing::{debug, info};

use crate::{error::Error, interfaces::DumpArgs};

/// Dumps the storage slots for a contract over the requested block range.
pub async fn dump(args: DumpArgs) -> Result<HashMap<FixedBytes<32>, FixedBytes<32>>, Error> {
    let start_time = Instant::now();
    let target =
        args.target.parse::<Address>().map_err(|e| eyre!("invalid target address: {e}"))?;
    if args.threads == 0 {
        return Err(eyre!("threads must be greater than zero").into());
    }
    let to_block = match args.to_block {
        Some(to_block) => to_block,
        None => latest_block_number(&args.rpc_url).await.map_err(|e| eyre!("rpc error: {e}"))?,
    };
    let block_range = validate_block_range(args.from_block, to_block)?;
    let block_count = u128::from(*block_range.end()) - u128::from(*block_range.start()) + 1;
    debug!("dumping storage from block range: {:?}", block_range);

    let mut storage = HashMap::new();
    let rpc_url = &args.rpc_url;
    let mut completed_count = 0_u128;
    fold_blocks(
        block_range,
        args.threads,
        |block_number| async move {
            get_block_state_diff(block_number, rpc_url)
                .await
                .map_err(|e| eyre!("failed to fetch state diff for block {block_number}: {e}"))
        },
        |block_trace| {
            for trace in block_trace {
                if let Some(diff) = trace.full_trace.state_diff {
                    if let Some(account) = diff.0.get(&target) {
                        apply_storage_changes(&mut storage, account.storage.iter());
                    }
                }
            }
            completed_count += 1;
            let remaining = block_count - completed_count;
            let completed_per_second = completed_count as f64 / start_time.elapsed().as_secs_f64();
            info!(
                "completed={}  remaining={}  eta={}",
                completed_count,
                remaining,
                format_eta(calculate_eta(completed_per_second, remaining as usize))
            );
        },
    )
    .await?;

    debug!("storage dump took {:?}", start_time.elapsed());
    Ok(storage)
}

fn validate_block_range(start: u128, end: u128) -> Result<RangeInclusive<u64>, Error> {
    if start > end {
        return Err(eyre!("starting block must not exceed ending block").into());
    }
    let start = u64::try_from(start).map_err(|_| eyre!("starting block exceeds u64"))?;
    let end = u64::try_from(end).map_err(|_| eyre!("ending block exceeds u64"))?;
    Ok(start..=end)
}

async fn fold_blocks<F, Fut, T, E>(
    range: RangeInclusive<u64>,
    concurrency: usize,
    fetch: F,
    mut apply: impl FnMut(T),
) -> Result<(), E>
where
    F: FnMut(u64) -> Fut,
    Fut: Future<Output = Result<T, E>>, {
    let mut blocks = stream::iter(range).map(fetch).buffered(concurrency);
    while let Some(block) = blocks.try_next().await? {
        apply(block);
    }
    Ok(())
}

fn apply_storage_changes<'a>(
    storage: &mut HashMap<FixedBytes<32>, FixedBytes<32>>,
    changes: impl IntoIterator<Item = (&'a FixedBytes<32>, &'a Delta<FixedBytes<32>>)>,
) {
    for (slot, diff) in changes {
        match diff {
            Delta::Added(value) => {
                storage.insert(*slot, *value);
            }
            Delta::Changed(value) => {
                storage.insert(*slot, value.to);
            }
            Delta::Removed(_) => {
                storage.remove(slot);
            }
            Delta::Unchanged => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::oneshot;

    #[tokio::test]
    async fn applies_blocks_in_chain_order_even_when_later_fetch_finishes_first() {
        let slot = FixedBytes::<32>::from([1; 32]);
        let first = FixedBytes::<32>::from([2; 32]);
        let second = FixedBytes::<32>::from([3; 32]);
        let (sender, receiver) = oneshot::channel();
        let mut sender = Some(sender);
        let mut receiver = Some(receiver);
        let mut storage = HashMap::new();
        fold_blocks(
            1..=2,
            2,
            |block| {
                let wait = if block == 1 { receiver.take() } else { None };
                let signal = if block == 2 { sender.take() } else { None };
                async move {
                    if let Some(signal) = signal {
                        signal.send(()).unwrap();
                    }
                    if let Some(wait) = wait {
                        wait.await.unwrap();
                    }
                    Ok::<_, ()>((slot, Delta::Added(if block == 1 { first } else { second })))
                }
            },
            |(slot, delta)| apply_storage_changes(&mut storage, [(&slot, &delta)]),
        )
        .await
        .unwrap();
        assert_eq!(storage.get(&slot), Some(&second));
    }

    #[tokio::test]
    async fn stops_and_returns_fetch_errors() {
        let mut applied = Vec::new();
        let result = fold_blocks(
            1..=3,
            2,
            |block| async move {
                if block == 2 {
                    Err("RPC failed")
                } else {
                    Ok(block)
                }
            },
            |block| applied.push(block),
        )
        .await;
        assert_eq!(result, Err("RPC failed"));
        assert_eq!(applied, [1]);
    }

    #[test]
    fn validates_block_ranges() {
        assert!(validate_block_range(2, 1).is_err());
        assert!(validate_block_range(0, u128::from(u64::MAX) + 1).is_err());
        assert_eq!(validate_block_range(1, 2).unwrap(), 1..=2);
    }

    #[tokio::test]
    async fn rejects_zero_threads_before_fetching() {
        let args = crate::DumpArgsBuilder::new()
            .target("0x0000000000000000000000000000000000000000".to_string())
            .threads(0)
            .build()
            .unwrap();
        let error = dump(args).await.unwrap_err();
        assert!(error.to_string().contains("threads must be greater than zero"));
    }
}
