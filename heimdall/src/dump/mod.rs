mod constants;
mod structures;
mod tui_views;
mod util;

use clap::{AppSettings, Parser};
use ethers::types::H160;
use heimdall_common::{
    io::logging::*,
    resources::transpose::{get_contract_creation, get_transaction_list},
};
use std::{collections::HashMap, env, str::FromStr, time::Instant};

use self::{
    constants::DUMP_STATE,
    structures::{dump_state::DumpState, transaction::Transaction},
    tui_views::TUIView,
    util::csv::write_storage_to_csv,
};

#[derive(Debug, Clone, Parser)]
#[clap(
    about = "Dump the value of all storage slots accessed by a contract",
    after_help = "For more information, read the wiki: https://jbecker.dev/r/heimdall-rs/wiki",
    global_setting = AppSettings::DeriveDisplayOrder,
    global_setting = AppSettings::ColoredHelp,
    override_usage = "heimdall dump <TARGET> [OPTIONS]"
)]
pub struct DumpArgs {
    /// The target to find and dump the storage slots of.
    #[clap(required = true)]
    pub target: String,

    /// Set the output verbosity level, 1 - 5.
    #[clap(flatten)]
    pub verbose: clap_verbosity_flag::Verbosity,

    /// The output directory to write the output to
    #[clap(long = "output", short, default_value = "", hide_default_value = true)]
    pub output: String,

    /// The RPC URL to use for fetching data.
    #[clap(long = "rpc-url", short, default_value = "", hide_default_value = true)]
    pub rpc_url: String,

    /// Your Transpose.io API Key
    #[clap(long = "transpose-api-key", short, default_value = "", hide_default_value = true)]
    pub transpose_api_key: String,

    /// The number of threads to use when fetching data.
    #[clap(long, default_value = "4", hide_default_value = true)]
    pub threads: usize,

    /// The block number to start dumping from.
    #[clap(long, default_value = "0", hide_default_value = true)]
    pub from_block: u128,

    /// The block number to stop dumping at.
    #[clap(long, default_value = "9999999999", hide_default_value = true)]
    pub to_block: u128,

    /// Whether to skip opening the TUI.
    #[clap(long)]
    pub no_tui: bool,

    /// The chain of the target. Valid chains are ethereum, polygon, goerli, canto, and arbitrum.
    #[clap(long, default_value = "ethereum", hide_default_value = true)]
    pub chain: String,
}

pub fn dump(args: DumpArgs) {
    let (logger, _) = Logger::new(match args.verbose.log_level() {
        Some(level) => level.as_str(),
        None => "SILENT",
    });

    // parse the output directory
    let mut output_dir = args.output.clone();
    if args.output.is_empty() {
        output_dir = match env::current_dir() {
            Ok(dir) => dir.into_os_string().into_string().unwrap(),
            Err(_) => {
                logger.error("failed to get current directory.");
                std::process::exit(1);
            }
        };
        output_dir.push_str("/output");
    }

    // check if transpose api key is set
    if args.transpose_api_key.is_empty() {
        logger.error("you must provide a Transpose API key, which is used to fetch all normal and internal transactions for your target.");
        logger.info("you can get a free API key at https://app.transpose.io/?utm_medium=organic&utm_source=heimdall-rs");
        std::process::exit(1);
    }

    // get the contract creation tx
    let contract_creation_tx =
        match get_contract_creation(&args.chain, &args.target, &args.transpose_api_key, &logger) {
            Some(tx) => tx,
            None => {
                logger.error(
                "failed to get contract creation transaction. Is the target a contract address?",
            );
                std::process::exit(1);
            }
        };

    // add the contract creation tx to the transactions list to be indexed
    let mut transactions: Vec<Transaction> = Vec::new();
    transactions.push(Transaction {
        indexed: false,
        hash: contract_creation_tx.1,
        block_number: contract_creation_tx.0,
    });

    // convert the target to an H160
    let addr_hash = match H160::from_str(&args.target) {
        Ok(addr) => addr,
        Err(_) => {
            logger.error(&format!("failed to parse target '{}' .", &args.target));
            std::process::exit(1);
        }
    };

    // push the address to the output directory
    if output_dir != args.output {
        output_dir.push_str(&format!("/{}", &args.target));
    }

    // fetch transactions
    let transaction_list = get_transaction_list(
        &args.chain,
        &args.target,
        &args.transpose_api_key,
        (&args.from_block, &args.to_block),
        &logger,
    );

    // convert to vec of Transaction
    for transaction in transaction_list {
        transactions.push(Transaction {
            indexed: false,
            hash: transaction.1,
            block_number: transaction.0,
        });
    }

    // update state
    let mut state = DUMP_STATE.lock().unwrap();
    *state = DumpState {
        args: args.clone(),
        transactions: transactions,
        scroll_index: 0,
        selection_size: 1,
        storage: HashMap::new(),
        view: TUIView::Main,
        start_time: Instant::now(),
        input_buffer: String::new(),
        filter: String::new(),
    };
    drop(state);

    let _output_dir = output_dir.clone();
    let _args = args.clone();

    // in a new thread, start the TUI
    let tui_thread = std::thread::spawn(move || {
        util::threads::tui::handle(args, output_dir);
    });

    // index transactions in a new thread
    let dump_thread = std::thread::spawn(move || {
        util::threads::indexer::handle(addr_hash);
    });

    // if no-tui flag is set, wait for the indexing thread to finish
    if _args.no_tui {
        match dump_thread.join() {
            Ok(_) => {}
            Err(e) => {
                logger.error("failed to join indexer thread.");
                logger.error(&format!("{e:?}"));
                std::process::exit(1);
            }
        }
    } else {
        // wait for the TUI thread to finish
        match tui_thread.join() {
            Ok(_) => {}
            Err(e) => {
                logger.error("failed to join TUI thread.");
                logger.error(&format!("{e:?}"));
                std::process::exit(1);
            }
        }
    }

    // write storage slots to csv
    let state = DUMP_STATE.lock().unwrap();
    write_storage_to_csv(&_output_dir, &"storage_dump.csv".to_string(), &state);
    logger.success(&format!("Wrote storage dump to '{_output_dir}/storage_dump.csv'."));
    logger.info(&format!(
        "Dumped {} storage values from '{}' .",
        state.storage.len(),
        &_args.target
    ));
}
