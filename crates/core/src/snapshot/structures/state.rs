use std::collections::HashMap;

use heimdall_common::ether::signatures::{ResolvedError, ResolvedLog};

use crate::snapshot::{menus::TUIView, structures::snapshot::Snapshot};

/// The state of the snapshot process, which will be updated as the process continues.
/// This struct is also used to store the state of the TUI, and is often passed to the TUI renderer
/// as a mutable reference.
#[derive(Debug, Clone)]
pub struct State {
    pub function_index: usize,
    pub scroll_index: usize,
    pub view: TUIView,
    pub input_buffer: String,
    pub snapshots: Vec<Snapshot>,
    pub resolved_events: HashMap<String, ResolvedLog>,
    pub resolved_errors: HashMap<String, ResolvedError>,
    pub target: String,
    pub compiler: (String, String),
    pub scroll: bool,
}

impl Default for State {
    fn default() -> Self {
        Self::new()
    }
}

impl State {
    pub fn new() -> Self {
        Self {
            snapshots: Vec::new(),
            function_index: 0,
            scroll_index: 0,
            view: TUIView::Main,
            input_buffer: String::new(),
            resolved_events: HashMap::new(),
            resolved_errors: HashMap::new(),
            target: String::new(),
            compiler: (String::new(), String::new()),
            scroll: false,
        }
    }
}
