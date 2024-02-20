mod constants;
mod menus;
mod structures;
mod util;

use clap::{AppSettings, Parser};
use derive_builder::Builder;
use ethers::types::H160;
use heimdall_common::{
    error, info,
    resources::transpose::{get_contract_creation, get_transaction_list},
    utils::io::logging::*,
};
use heimdall_config::parse_url_arg;
use std::{collections::HashMap, env, str::FromStr, time::Instant};

use crate::error::Error;

use self::{
    constants::DUMP_STATE,
    menus::TUIView,
    structures::{dump_state::DumpState, transaction::Transaction},
    util::csv::{build_csv, DumpRow},
};

#[derive(Debug, Clone, Parser, Builder)]
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

    /// The output directory to write the output to or 'print' to print to the console
    #[clap(long = "output", short, default_value = "output", hide_default_value = true)]
    pub output: String,

    /// The RPC URL to use for fetching data.
    /// This can be an explicit URL or a reference to a MESC endpoint.
    #[clap(long, short, parse(try_from_str = parse_url_arg), default_value = "", hide_default_value = true)]
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

    /// The name for the output file
    #[clap(long, short, default_value = "", hide_default_value = true)]
    pub name: String,
}

impl DumpArgsBuilder {
    pub fn new() -> Self {
        Self {
            target: Some(String::new()),
            verbose: Some(clap_verbosity_flag::Verbosity::new(0, 1)),
            output: Some(String::new()),
            rpc_url: Some(String::new()),
            transpose_api_key: Some(String::new()),
            threads: Some(8),
            from_block: Some(0),
            to_block: Some(9999999999),
            no_tui: Some(true),
            chain: Some(String::from("ethereum")),
            name: Some(String::new()),
        }
    }
}

/// entry point for the dump module. Will fetch all storage slots accessed by the target contract,
/// and dump them to a CSV file or the TUI.
pub async fn dump(args: DumpArgs) -> Result<Vec<DumpRow>, Error> {
    set_logger_env(&args.verbose);

    // parse the output directory
    let mut output_dir = args.output.clone();
    if args.output.is_empty() {
        output_dir = env::current_dir()
            .map_err(|_| Error::Generic("failed to get current directory".to_string()))?
            .into_os_string()
            .into_string()
            .map_err(|_| {
                Error::Generic("failed to convert output directory to string".to_string())
            })?;
        output_dir.push_str("/output");
    }

    // check if transpose api key is set
    if args.transpose_api_key.is_empty() {
        error!("you must provide a Transpose API key, which is used to fetch all normal and internal transactions for your target.");
        info!("you can get a free API key at https://app.transpose.io/?utm_medium=organic&utm_source=heimdall-rs");
        return Err(Error::Generic("failed to get Transpose API key".to_string()));
    }

    // get the contract creation tx
    let contract_creation_tx =
        get_contract_creation(&args.chain, &args.target, &args.transpose_api_key).await.ok_or(
            Error::Generic(
                "failed to get contract creation transaction. Is the target a contract address?"
                    .to_string(),
            ),
        )?;

    // add the contract creation tx to the transactions list to be indexed
    let mut transactions: Vec<Transaction> = Vec::new();
    transactions.push(Transaction {
        indexed: false,
        hash: contract_creation_tx.1,
        block_number: contract_creation_tx.0,
    });

    // convert the target to an H160
    let addr_hash = H160::from_str(&args.target)
        .map_err(|e| Error::Generic(format!("failed to parse target: {}", e)))?;

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
    )
    .await
    .map_err(|e| Error::Generic(format!("failed to get transaction list: {}", e)))?;

    // convert to vec of Transaction
    for transaction in transaction_list {
        transactions.push(Transaction {
            indexed: false,
            hash: transaction.1,
            block_number: transaction.0,
        });
    }

    // update state
    let mut state = DUMP_STATE
        .lock()
        .map_err(|e| Error::Generic(format!("failed to obtain lock on DUMP_STATE: {}", e)))?;
    *state = DumpState {
        args: args.clone(),
        transactions,
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
        let _ = util::threads::tui::handle(&args, &output_dir);
    });

    // index transactions in a new thread
    let dump_thread = std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
        rt.block_on(util::threads::indexer::handle(addr_hash))
    });

    // if no-tui flag is set, wait for the indexing thread to finish
    if _args.no_tui {
        let _ = dump_thread
            .join()
            .map_err(|e| Error::Generic(format!("failed to join dump thread: {:?}", e)))?;
    } else {
        // wait for the TUI thread to finish
        tui_thread
            .join()
            .map_err(|e| Error::Generic(format!("failed to join TUI thread: {:?}", e)))?;
    }

    // write storage slots to csv
    let state = DUMP_STATE
        .lock()
        .map_err(|e| Error::Generic(format!("failed to obtain lock on DUMP_STATE: {}", e)))?;
    let csv = build_csv(&state);
    info!(&format!("Dumped {} storage values from '{}' .", state.storage.len(), &_args.target));
    Ok(csv)
}
