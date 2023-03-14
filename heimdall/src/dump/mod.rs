mod tests;
mod util;
mod constants;
mod tui_views;

use std::collections::HashMap;
use std::str::FromStr;
use std::time::{Instant, Duration};
use std::{io, env};
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

use self::constants::{DUMP_STATE, DECODE_AS_TYPES};
use self::tui_views::command_palette::render_tui_command_palette;
use self::tui_views::help::render_tui_help;
use self::util::csv::write_storage_to_csv;
use self::util::table::copy_selected;
use self::util::{get_storage_diff, cleanup_terminal};

#[derive(Debug, Clone, Parser)]
#[clap(about = "Dump the value of all storage slots accessed by a contract",
       after_help = "For more information, read the wiki: https://jbecker.dev/r/heimdall-rs/wiki",
       global_setting = AppSettings::DeriveDisplayOrder,
         global_setting = AppSettings::ColoredHelp,
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

    /// The RPC URL to use for fetching data.
    #[clap(long="rpc-url", short, default_value = "", hide_default_value = true)]
    pub rpc_url: String,

    /// Your Transpose.io API Key
    #[clap(long="transpose-api-key", short, default_value = "", hide_default_value = true)]
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
}

#[derive(Debug, Clone)]
pub struct StorageSlot {
    pub alias: Option<String>,
    pub value: H256,
    pub modifiers: Vec<(u128, String)>,
    pub decode_as_type_index: usize,
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
    pub selection_size: usize,
    pub transactions: Vec<Transaction>,
    pub storage: HashMap<H256, StorageSlot>,
    pub view: TUIView,
    pub start_time: Instant,
    pub input_buffer: String,
    pub filter: String,
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
                threads: 4,
                from_block: 0,
                to_block: 9999999999,
            },
            scroll_index: 0,
            selection_size: 1,
            transactions: Vec::new(),
            storage: HashMap::new(),
            view: TUIView::Main,
            start_time: Instant::now(),
            input_buffer: String::new(),
            filter: String::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub enum TUIView {
    Killed,
    Main,
    CommandPalette,
    Help,
}

#[allow(unreachable_patterns)]
fn render_ui<B: Backend>(
    f: &mut Frame<B>,
    state: &mut DumpState
) {
    match state.view {
        TUIView::Main => { render_tui_view_main(f, state) },
        TUIView::CommandPalette => { render_tui_command_palette(f, state) },
        TUIView::Help => { render_tui_help(f, state) },
        _ => {}
    }
 }

pub fn dump(args: DumpArgs) {
    let (logger, _)= Logger::new(args.verbose.log_level().unwrap().as_str());

    // check if transpose api key is set
    if &args.transpose_api_key.len() <= &0 {
        logger.error("you must provide a Transpose API key, which is used to fetch all normal and internal transactions for your target.");
        logger.info("you can get a free API key at https://app.transpose.io/?utm_medium=organic&utm_source=heimdall-rs");
        std::process::exit(1);
    }

    // check if the RPC url is set and supports trace_replayTransaction
    get_storage_diff(&Transaction {
        indexed: false,
        hash: String::from("0xb95343413e459a0f97461812111254163ae53467855c0d73e0f1e7c5b8442fa3"),
        block_number: 471968
    }, &args);

    // parse the output directory
    let mut output_dir = args.output.clone();
    if &args.output.len() <= &0 {
        output_dir = match env::current_dir() {
            Ok(dir) => dir.into_os_string().into_string().unwrap(),
            Err(_) => {
                logger.error("failed to get current directory.");
                std::process::exit(1);
            }
        };
        output_dir.push_str("/output");
    }

    // convert the target to an H160
    let addr_hash = match H160::from_str(&args.target) {
        Ok(addr) => addr,
        Err(_) => {
            logger.error(&format!("failed to parse target '{}' .", &args.target));
            std::process::exit(1);
        }
    };

    // push the address to the output directory
    if &output_dir != &args.output {
        output_dir.push_str(&format!("/{}", &args.target));
    }

    // fetch transactions
    let transaction_list = get_transaction_list(&args.target, &args.transpose_api_key, (&args.from_block, &args.to_block), &logger);

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
        selection_size: 1,
        storage: HashMap::new(),
        view: TUIView::Main,
        start_time: Instant::now(),
        input_buffer: String::new(),
        filter: String::new(),
    };
    drop(state);

    let _output_dir = output_dir.clone();

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
            if crossterm::event::poll(Duration::from_millis(10)).unwrap() {
                if let Ok(event) = crossterm::event::read() {
                    match event {
                        crossterm::event::Event::Key(key) => {
                            let mut state = DUMP_STATE.lock().unwrap();

                            // ignore key events if command palette is open
                            if state.view == TUIView::CommandPalette {
                                match key.code {

                                    // handle keys in command palette
                                    crossterm::event::KeyCode::Char(c) => {
                                        state.input_buffer.push(c);
                                    },

                                    // handle backspace
                                    crossterm::event::KeyCode::Backspace => {
                                        state.input_buffer.pop();
                                    },

                                    // enter command
                                    crossterm::event::KeyCode::Enter => {
                                        state.filter = String::new();
                                        let mut split = state.input_buffer.split(" ");
                                        let command = split.next().unwrap();
                                        let args = split.collect::<Vec<&str>>();

                                        match command {
                                            ":q" | ":quit" => {
                                                state.view = TUIView::Killed;
                                                break;
                                            }
                                            ":h" | ":help" => {
                                                state.view = TUIView::Help;
                                            }
                                            ":f" | ":find" => {
                                                if args.len() > 0 {
                                                    state.filter = args[0].to_string();
                                                }
                                                state.view = TUIView::Main;
                                            }
                                            ":e" | ":export" => {
                                                if args.len() > 0 {
                                                    write_storage_to_csv(&output_dir.clone(), &args[0].to_string(), &state);
                                                }
                                            }
                                            ":s" | ":seek" => {
                                                if args.len() > 1 {
                                                    let direction = args[0].to_lowercase();
                                                    let amount = args[1].parse::<usize>().unwrap_or(0);
                                                    match direction.as_str() {
                                                        "up" => {
                                                            if state.scroll_index >= amount {
                                                                state.scroll_index -= amount;
                                                            } else {
                                                                state.scroll_index = 0;
                                                            }
                                                        }
                                                        "down" => {
                                                            if state.scroll_index + amount < state.storage.len() {
                                                                state.scroll_index += amount;
                                                            } else {
                                                                state.scroll_index = state.storage.len() - 1;
                                                            }
                                                        }
                                                        _ => {}
                                                    }
                                                }
                                                state.view = TUIView::Main;
                                            }
                                            _ => {
                                                state.view = TUIView::Main;
                                            }
                                        }
                                    },

                                    // handle escape
                                    crossterm::event::KeyCode::Esc => {
                                        state.filter = String::new();
                                        state.view = TUIView::Main;
                                    }
                                    
                                    _ => {}
                                }

                                drop(state);
                                continue;
                            }

                            match key.code {

                                // copy value on MODIFIER + C
                                crossterm::event::KeyCode::Char('c') => {
                                    if crossterm::event::KeyModifiers::NONE != key.modifiers {
                                        copy_selected(&mut state)
                                    }
                                },

                                // main on escape
                                crossterm::event::KeyCode::Esc => {
                                    state.filter = String::new();
                                    state.view = TUIView::Main;
                                },

                                // select transaction
                                crossterm::event::KeyCode::Right => {

                                    // increment decode_as_type_index on all selected transactions
                                    let scroll_index = state.scroll_index.clone();
                                    let selection_size = state.selection_size.clone();
                                    let mut storage_iter = state.storage.iter_mut().collect::<Vec<_>>();
                                    storage_iter.sort_by_key(|(slot, _)| *slot);

                                    for (i, (_, value)) in storage_iter.iter_mut().enumerate() {
                                        if i >= scroll_index && i < scroll_index + selection_size {
                                            
                                            // saturating increment
                                            if value.decode_as_type_index + 1 >= DECODE_AS_TYPES.len() {
                                                value.decode_as_type_index = 0;
                                            } else {
                                                value.decode_as_type_index += 1;
                                            }

                                        }
                                        else if i >= scroll_index + selection_size {
                                            break;
                                        }
                                    }
                                },

                                // deselect transaction
                                crossterm::event::KeyCode::Left => {
                                    
                                    // decrement decode_as_type_index on all selected transactions
                                    let scroll_index = state.scroll_index.clone();
                                    let selection_size = state.selection_size.clone();
                                    let mut storage_iter = state.storage.iter_mut().collect::<Vec<_>>();
                                    storage_iter.sort_by_key(|(slot, _)| *slot);

                                    for (i, (_, value)) in storage_iter.iter_mut().enumerate() {
                                        if i >= scroll_index && i < scroll_index + selection_size {
                                            
                                            // saturating decrement
                                            if value.decode_as_type_index == 0 {
                                                value.decode_as_type_index = DECODE_AS_TYPES.len() - 1;
                                            } else {
                                                value.decode_as_type_index -= 1;
                                            }

                                        }
                                        else if i >= scroll_index + selection_size {
                                            break;
                                        }
                                    }
                                },

                                // scroll down
                                crossterm::event::KeyCode::Down => {
                                    state.selection_size = 1;
                                    state.scroll_index += 1;
                                },

                                // scroll up
                                crossterm::event::KeyCode::Up => {
                                    state.selection_size = 1;
                                    if state.scroll_index > 0 {
                                        state.scroll_index -= 1;
                                    }
                                },

                                // toggle command palette on ":"
                                crossterm::event::KeyCode::Char(':') => {
                                    match state.view {
                                        TUIView::CommandPalette => {
                                            state.view = TUIView::Main;
                                        }
                                        _ => {
                                            state.input_buffer = String::from(":");
                                            state.view = TUIView::CommandPalette;
                                        }
                                    }
                                },

                                _ => {}
                            }
                            drop(state)
                        },
                        crossterm::event::Event::Mouse(mouse) => {
                            let mut state = DUMP_STATE.lock().unwrap();
                            match mouse.kind {

                                // scroll down
                                crossterm::event::MouseEventKind::ScrollDown => {
                                    
                                    // if shift is held, increase selection size
                                    if mouse.modifiers.contains(crossterm::event::KeyModifiers::SHIFT) {
                                        state.selection_size += 1;
                                    }
                                    else {
                                        state.selection_size = 1;
                                        state.scroll_index += 1;
                                    }
                                },

                                // scroll up
                                crossterm::event::MouseEventKind::ScrollUp => {

                                    // if shift is held, increase selection size
                                    if mouse.modifiers.contains(crossterm::event::KeyModifiers::SHIFT) {
                                        state.selection_size -= 1;
                                    }
                                    else {
                                        state.selection_size = 1;
                                        if state.scroll_index > 0 {
                                            state.scroll_index -= 1;
                                        }
                                    }
                                },
                                _ => {}
                            }
                            drop(state);
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
        let args = state.args.clone();
        drop(state);
        
        task_pool(transactions, args.threads, move |tx| {

            // get the storage diff for this transaction
            let state_diff = get_storage_diff(&tx, &args);

            // unlock state
            let mut state = DUMP_STATE.lock().unwrap();
        
            // find the transaction in the state
            let txs = state.transactions.iter_mut().find(|t| t.hash == tx.hash).unwrap();
            let block_number = tx.block_number.clone();
            txs.indexed = true;

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

                                       // update value if newest modifier
                                       if slot.modifiers.iter().all(|m| m.0 < block_number) {
                                            slot.value = *value;
                                        }
                                        
                                        slot.modifiers.push((block_number, tx.hash.clone().to_owned()));
                                    },
                                    None => {

                                        // insert into state
                                        state.storage.insert(
                                            *slot, 
                                            StorageSlot {
                                                value: *value,
                                                modifiers: vec![(block_number, tx.hash.clone().to_owned())],
                                                alias: None,
                                                decode_as_type_index: 0
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
    match tui_thread.join() {
        Ok(_) => {},
        Err(e) => {
            logger.error("failed to join TUI thread.");
            logger.error(&format!("{:?}", e));
            std::process::exit(1);
        }
    }

    // write storage slots to csv
    let state = DUMP_STATE.lock().unwrap();
    write_storage_to_csv(&_output_dir, &"storage_dump.csv".to_string(), &state);

    logger.info(&format!("Dumped {} storage values from '{}' .", state.storage.len(), &args.target));
}