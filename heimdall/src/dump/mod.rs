mod tests;
mod tui_views;

use std::sync::{Arc, Mutex};
use std::time::Instant;
use std::{io};
use clap::{AppSettings, Parser};
use crossterm::event::{EnableMouseCapture, DisableMouseCapture};
use crossterm::execute;
use crossterm::terminal::{enable_raw_mode, EnterAlternateScreen, disable_raw_mode, LeaveAlternateScreen};
use ethers::types::U256;
use heimdall_common::resources::transpose::get_transaction_list;
use heimdall_common::{
    io::{ logging::* },
};
use tui::backend::Backend;
use tui::{Frame, backend::CrosstermBackend, Terminal};

use tui_views::main::render_tui_view_main;


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
}

#[derive(Debug, Clone)]
pub struct StorageSlot {
    pub slot: U256,
    pub alias: Option<String>,
    pub value: Option<U256>
}

#[derive(Debug, Clone)]
pub struct Transaction {
    pub indexed: bool,
    pub hash: String,
    pub block: u128,
}

#[derive(Debug, Clone)]
pub struct DumpState {
    pub args: DumpArgs,
    pub transactions: Vec<Transaction>,
    pub storage: Vec<StorageSlot>,
    pub view: TUIView,
    pub start_time: Instant,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum TUIView {
    Main,
    CommandPalette,
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
    use std::time::Instant;
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

    // fetch transactions
    let transaction_list = get_transaction_list(&args.target, &args.transpose_api_key, &logger);

    // convert to vec of Transaction
    let mut transactions: Vec<Transaction> = Vec::new();
    for transaction in transaction_list {
        transactions.push(Transaction {
            indexed: false,
            hash: transaction.1,
            block: transaction.0
        });
    }

    // create new state
    let mut state = DumpState {
        args: args.clone(),
        transactions: transactions,
        storage: Vec::new(),
        view: TUIView::Main,
        start_time: Instant::now(),
    };

    // in a new thread, start the TUI
    let tui_thread = std::thread::spawn(move || {

        // create new TUI terminal
        enable_raw_mode().unwrap();
        let mut stdout = io::stdout();
        execute!(
            stdout,
            EnterAlternateScreen,
            EnableMouseCapture
        ).unwrap();
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend).unwrap();

        // while user does not click CTRL+C
        loop {
            terminal.draw(|f| { render_ui(f, &mut state); }).unwrap();

            // check for user input
            if let Ok(event) = crossterm::event::read() {
                match event {
                    crossterm::event::Event::Key(key) => {
                        match key.code {
                            crossterm::event::KeyCode::Char('q') => {
                                break;
                            },
                            _ => {}
                        }
                    },
                    _ => {}
                }
            }
        }

        // cleanup
        disable_raw_mode().unwrap();
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        ).unwrap();
        terminal.show_cursor().unwrap();
    });

    for tx in state.transactions.iter_mut() {
        tx.indexed = true;
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    // wait for the TUI thread to finish
    tui_thread.join().unwrap();

    logger.debug(&format!("Dumped storage slots in {:?}.", now.elapsed()));
}