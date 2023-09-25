pub mod csv;
pub mod table;
pub mod threads;

use std::{io, str::FromStr};

use crossterm::{
    event::DisableMouseCapture,
    execute,
    terminal::{disable_raw_mode, LeaveAlternateScreen},
};
use ethers::{
    providers::{Http, Middleware, Provider},
    types::{StateDiff, TraceType, H256},
};
use heimdall_cache::{read_cache, store_cache};
use heimdall_common::io::logging::Logger;
use tui::{backend::CrosstermBackend, Terminal};

use super::{structures::transaction::Transaction, DumpArgs};

// cleanup the terminal, disable raw mode, and leave the alternate screen
pub fn cleanup_terminal() {
    let stdout = io::stdout();
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).unwrap();
    disable_raw_mode().unwrap();
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture).unwrap();
    terminal.show_cursor().unwrap();
}

// get the state diff for the given transaction
pub fn get_storage_diff(tx: &Transaction, args: &DumpArgs) -> Option<StateDiff> {
    // create new logger
    let (logger, _) = Logger::new(match args.verbose.log_level() {
        Some(level) => level.as_str(),
        None => "SILENT",
    });

    // create new runtime block
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();

    rt.block_on(async {

        // check the cache for a matching address
        if let Some(state_diff) = read_cache(&format!("diff.{}", &tx.hash)) {
            return state_diff;
        }

        // make sure the RPC provider isn't empty
        if args.rpc_url.is_empty() {
            cleanup_terminal();
            logger.error("fetching an on-chain transaction requires an RPC provider. Use `heimdall dump --help` for more information.");
            std::process::exit(1);
        }

        // create new provider
        let provider = match Provider::<Http>::try_from(&args.rpc_url) {
            Ok(provider) => provider,
            Err(_) => {
                cleanup_terminal();
                logger.error(&format!("failed to connect to RPC provider '{}' .", &args.rpc_url));
                std::process::exit(1)
            }
        };

        // safely unwrap the transaction hash
        let transaction_hash = match H256::from_str(&tx.hash) {
            Ok(transaction_hash) => transaction_hash,
            Err(_) => {
                cleanup_terminal();
                logger.error(&format!("failed to parse transaction hash '{}' .", &tx.hash));
                std::process::exit(1)
            }
        };

        // fetch the state diff for the transaction
        let state_diff = match provider.trace_replay_transaction(transaction_hash, vec![TraceType::StateDiff]).await {
            Ok(traces) => traces.state_diff,
            Err(e) => {
                cleanup_terminal();
                logger.error(&format!("failed to replay and trace transaction '{}' . does your RPC provider support it?", &tx.hash));
                logger.error(&format!("error: '{e}' ."));
                std::process::exit(1)
            }
        };

        // write the state diff to the cache
        let expiry = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() + 60 * 60 * 24 * 7;
        store_cache(&format!("diff.{}", &tx.hash), &state_diff, Some(expiry));

        state_diff
    })
}
