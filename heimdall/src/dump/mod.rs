mod tests;
mod util;
mod tui_views;

use std::collections::HashMap;
use std::str::FromStr;
use std::sync::{Mutex};
use std::time::{Instant, Duration};
use std::{io};
use clap::{AppSettings, Parser};
use crossterm::event::{EnableMouseCapture};
use crossterm::execute;
use crossterm::terminal::{enable_raw_mode, EnterAlternateScreen};
use ethers::types::{H256, H160, Diff};
use heimdall_common::resources::transpose::get_transaction_list;
use heimdall_common::{
    io::{ logging::* },
    utils::{ threading::task_pool }
};
use tui::backend::Backend;
use tui::{Frame, backend::CrosstermBackend, Terminal};

use tui_views::main::render_tui_view_main;
use lazy_static::lazy_static;

use self::util::{get_storage_diff, cleanup_terminal};

#[derive(Debug, Clone, Parser)]
#[clap(about = "Dump the value of all storage slots accessed by a contract",
       after_help = "For more information, read the wiki: https://jbecker.dev/r/heimdall-rs/wiki",
       global_setting = AppSettings::DeriveDisplayOrder,
       override_usage = "heimdall dump <TARGET> [OPTIONS]")]
pub struct DumpArgs {

    /// The target to find and dump the storage slots of.
    #[clap(required=true)]
    pub target: String,

    /// Set the output verbosity level, 1 - 5.
    #[clap(flatten)]
    pub verbose: clap_verbosity_flag::Verbosity,

    /// The output directory to write the output to
    #[clap(long="output", short, default_value = "", hide_default_value = true)]
    pub output: String,

    /// The RPC provider to use for fetching on-chain data.
    #[clap(long="rpc-url", short, default_value = "", hide_default_value = true)]
    pub rpc_url: String,

    /// Your Transpose.io API Key.
    #[clap(long="transpose-api-key", short, default_value = "", hide_default_value = true)]
    pub transpose_api_key: String,

    /// When prompted, always select the default value.
    #[clap(long, short)]
    pub default: bool,

    /// The number of threads to use
    #[clap(long, short, default_value = "4", hide_default_value = true)]
    pub threads: usize,
}

#[derive(Debug, Clone)]
pub struct StorageSlot {
    pub alias: Option<String>,
    pub modified_at: u128,
    pub value: H256,
}

#[derive(Debug, Clone)]
pub struct Transaction {
    pub indexed: bool,
    pub hash: String,
    pub block_number: u128,
}

#[derive(Debug, Clone)]
pub struct DumpState {
    pub args: DumpArgs,
    pub scroll_index: usize,
    pub transactions: Vec<Transaction>,
    pub storage: HashMap<H256, StorageSlot>,
    pub view: TUIView,
    pub start_time: Instant,
}

impl DumpState {
    pub fn new() -> Self {
        Self {
            args: DumpArgs {
                target: String::new(),
                verbose: clap_verbosity_flag::Verbosity::new(1, 0),
                output: String::new(),
                rpc_url: String::new(),
                transpose_api_key: String::new(),
                default: false,
                threads: 4,
            },
            scroll_index: 0,
            transactions: Vec::new(),
            storage: HashMap::new(),
            view: TUIView::Main,
            start_time: Instant::now(),
        }
    }
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum TUIView {
    Main,
    CommandPalette,
}

lazy_static! {
    static ref DUMP_STATE: Mutex<DumpState> = Mutex::new(DumpState::new());
}

fn render_ui<B: Backend>(
    f: &mut Frame<B>,
    state: &mut DumpState
) {
    match state.view {
        TUIView::Main => { render_tui_view_main(f, state) },
        _ => {}
    }
 }

pub fn dump(args: DumpArgs) {
    let now = Instant::now();

    let (logger, _)= Logger::new(args.verbose.log_level().unwrap().as_str());

    // check if transpose api key is set
    if &args.transpose_api_key.len() <= &0 {
        logger.error("you must provide a Transpose API key.");
        logger.info("you can get a free API key at https://app.transpose.io");
        std::process::exit(1);
    }

    // parse the output directory
    // let mut output_dir: String;
    // if &args.output.len() <= &0 {
    //     output_dir = match env::current_dir() {
    //         Ok(dir) => dir.into_os_string().into_string().unwrap(),
    //         Err(_) => {
    //             logger.error("failed to get current directory.");
    //             std::process::exit(1);
    //         }
    //     };
    //     output_dir.push_str("/output");
    // }
    // else {
    //     output_dir = args.output.clone();
    // }

    // convert the target to an H160
    let addr_hash = match H160::from_str(&args.target) {
        Ok(addr) => addr,
        Err(_) => {
            logger.error(&format!("failed to parse target '{}' .", &args.target));
            std::process::exit(1);
        }
    };

    // fetch transactions
    let transaction_list = get_transaction_list(&args.target, &args.transpose_api_key, &logger);

    // convert to vec of Transaction
    let mut transactions: Vec<Transaction> = Vec::new();
    for transaction in transaction_list {
        transactions.push(Transaction {
            indexed: false,
            hash: transaction.1,
            block_number: transaction.0
        });
    }

    // update state
    let mut state = DUMP_STATE.lock().unwrap();
    *state = DumpState {
        args: args.clone(),
        transactions: transactions,
        scroll_index: 0,
        storage: HashMap::new(),
        view: TUIView::Main,
        start_time: Instant::now(),
    };
    drop(state);


    // in a new thread, start the TUI
    let tui_thread = std::thread::spawn(move || {

        // create new TUI terminal
        enable_raw_mode().unwrap();
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture).unwrap();
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend).unwrap();

        loop {
            let mut state = DUMP_STATE.lock().unwrap();
            terminal.draw(|f| { render_ui(f, &mut state); }).unwrap();
            drop(state);

            // check for user input
            if crossterm::event::poll(Duration::from_millis(100)).unwrap() {
                if let Ok(event) = crossterm::event::read() {
                    match event {
                        crossterm::event::Event::Key(key) => {
                            match key.code {

                                // quit
                                crossterm::event::KeyCode::Char('q') => { break; },

                                // scroll down
                                crossterm::event::KeyCode::Down => {
                                    let mut state = DUMP_STATE.lock().unwrap();
                                    state.scroll_index += 1;
                                    drop(state);
                                },

                                // scroll up
                                crossterm::event::KeyCode::Up => {
                                    let mut state = DUMP_STATE.lock().unwrap();
                                    if state.scroll_index > 0 {
                                        state.scroll_index -= 1;
                                    }
                                    drop(state);
                                },

                                _ => {}
                            }
                        },
                        crossterm::event::Event::Mouse(mouse) => {
                            match mouse.kind {

                                // scroll down
                                crossterm::event::MouseEventKind::ScrollDown => {
                                    let mut state = DUMP_STATE.lock().unwrap();
                                    state.scroll_index += 1;
                                    drop(state);
                                },

                                // scroll up
                                crossterm::event::MouseEventKind::ScrollUp => {
                                    let mut state = DUMP_STATE.lock().unwrap();
                                    if state.scroll_index > 0 {
                                        state.scroll_index -= 1;
                                    }
                                    drop(state);
                                },
                                _ => {}
                            }
                        },
                        _ => {}
                    }
                }
            }
        }

        cleanup_terminal();
    });

    // index transactions in a new thread
    std::thread::spawn(move || {
        let state = DUMP_STATE.lock().unwrap();
        let transactions = state.transactions.clone();
        drop(state);
        
        task_pool(transactions, args.threads, move |tx| {

            // get the storage diff for this transaction
            let state_diff = get_storage_diff(&tx, &args);

            // unlock state
            let mut state = DUMP_STATE.lock().unwrap();
        
            // find the transaction in the state
            let tx = state.transactions.iter_mut().find(|t| t.hash == tx.hash).unwrap();
            let block_number = tx.block_number.clone();
            tx.indexed = true;


            // unwrap the state diff
            match state_diff {
                Some(state_diff) => {

                    // get diff for this address
                    match state_diff.0.get(&addr_hash) {
                        Some(diff) => {
                            
                            // build diff of StorageSlots and append to state
                            for (slot, diff_type) in &diff.storage {

                                // parse value from diff type
                                let value = match diff_type {
                                    Diff::Born(value) => value,
                                    Diff::Changed(changed) => &changed.to,
                                    Diff::Died(_) => {
                                        state.storage.remove(slot);
                                        continue;
                                    }
                                    _ => continue,
                                };

                                // get the slot from the state
                                match state.storage.get_mut(slot) {
                                    Some(slot) => {

                                        // update slot if it's newer
                                        if slot.modified_at > block_number {
                                            continue;
                                        }

                                        slot.value = *value;
                                        slot.modified_at = block_number;
                                    },
                                    None => {

                                        // insert into state
                                        state.storage.insert(
                                            *slot, 
                                            StorageSlot {
                                                value: *value,
                                                modified_at: block_number,
                                                alias: None,
                                            }
                                        );
                                    }
                                }
                            }

                        },
                        None => {}
                    }
                },
                None => {}
            }

            // drop state
            drop(state);
        });
    });

    // wait for the TUI thread to finish
    tui_thread.join().unwrap();

    logger.debug(&format!("Dumped storage slots in {:?}.", now.elapsed()));
}