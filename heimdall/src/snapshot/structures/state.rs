use std::collections::HashMap;

use heimdall_common::ether::signatures::{ResolvedError, ResolvedLog};

use crate::snapshot::{menus::TUIView, util::Snapshot};

#[derive(Debug, Clone)]
pub struct State {
    pub scroll_index: usize,
    pub view: TUIView,
    pub input_buffer: String,
    pub snapshots: Vec<Snapshot>,
    pub resolved_events: HashMap<String, ResolvedLog>,
    pub resolved_errors: HashMap<String, ResolvedError>,
    pub target: String,
    pub compiler: (String, String),
}

impl State {
    pub fn new() -> Self {
        Self {
            snapshots: Vec::new(),
            scroll_index: 0,
            view: TUIView::Main,
            input_buffer: String::new(),
            resolved_events: HashMap::new(),
            resolved_errors: HashMap::new(),
            target: String::new(),
            compiler: (String::new(), String::new()),
        }
    }
}
