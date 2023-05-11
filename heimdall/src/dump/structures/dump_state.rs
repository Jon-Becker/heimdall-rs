use std::{collections::HashMap, time::Instant};

use ethers::types::H256;

use crate::dump::{tui_views::TUIView, DumpArgs};

use super::{storage_slot::StorageSlot, transaction::Transaction};

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
                no_tui: false,
                chain: String::from("ethereum"),
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
