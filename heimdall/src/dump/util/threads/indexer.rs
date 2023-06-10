use std::time::Duration;

use ethers::types::{Diff, H160};
use heimdall_common::{io::logging::Logger, utils::threading::task_pool};
use indicatif::ProgressBar;

use crate::dump::{
    constants::DUMP_STATE, structures::storage_slot::StorageSlot, util::get_storage_diff,
};

pub fn handle(addr_hash: H160) {
    let state = DUMP_STATE.lock().unwrap();
    let transactions = state.transactions.clone();
    let args = state.args.clone();
    drop(state);

    // the number of threads cannot exceed the number of transactions
    let num_indexing_threads = std::cmp::min(transactions.len(), args.threads);

    // get a new logger
    let (logger, _) = Logger::new(match args.verbose.log_level() {
        Some(level) => level.as_str(),
        None => "SILENT",
    });

    // get a new progress bar
    let transaction_list_progress = ProgressBar::new_spinner();
    transaction_list_progress.enable_steady_tick(Duration::from_millis(100));
    transaction_list_progress.set_style(logger.info_spinner());

    if !args.no_tui {
        transaction_list_progress.finish_and_clear();
    }

    task_pool(transactions, num_indexing_threads, move |tx| {
        // get the storage diff for this transaction
        let state_diff = get_storage_diff(&tx, &args);
        // unlock state
        let mut state = DUMP_STATE.lock().unwrap();

        // find the transaction in the state
        let all_txs = state.transactions.clone();
        let txs = state.transactions.iter_mut().find(|t| t.hash == tx.hash).unwrap();
        let block_number = tx.block_number;

        if args.no_tui {
            let num_done = all_txs.iter().filter(|t| t.indexed).count();
            let total = all_txs.len();
            transaction_list_progress.set_message(format!(
                "dumping storage. Progress {}/{} ({:.2}%)",
                num_done,
                total,
                (num_done as f64 / total as f64) * 100.0
            ));

            if num_done == total - 1 {
                transaction_list_progress.finish_and_clear();
            }
        }
        txs.indexed = true;

        // unwrap the state diff
        if let Some(state_diff) = state_diff {
            // get diff for this address
            if let Some(diff) = state_diff.0.get(&addr_hash) {
                // build diff of StorageSlots and append to state
                for (slot, diff_type) in &diff.storage {
                    // parse value from diff type
                    let value = match diff_type {
                        Diff::Born(value) => value,
                        Diff::Changed(changed) => &changed.to,
                        Diff::Died(_) => {
                            state.storage.remove(slot);
                            continue
                        }
                        _ => continue,
                    };

                    // get the slot from the state
                    match state.storage.get_mut(slot) {
                        Some(slot) => {
                            // update value if newest modifier
                            if slot.modifiers.iter().all(|m| m.0 < block_number) {
                                slot.value = *value;
                            }

                            slot.modifiers.push((block_number, tx.hash.clone().to_owned()));
                        }
                        None => {
                            // insert into state
                            state.storage.insert(
                                *slot,
                                StorageSlot {
                                    value: *value,
                                    modifiers: vec![(block_number, tx.hash.clone().to_owned())],
                                    alias: None,
                                    decode_as_type_index: 0,
                                },
                            );
                        }
                    }
                }
            }
        }

        // drop state
        drop(state);
    });
}
