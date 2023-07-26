use crate::snapshot::{menus::TUIView, util::Snapshot};

#[derive(Debug, Clone)]
pub struct State {
    pub scroll_index: usize,
    pub view: TUIView,
    pub input_buffer: String,
    pub snapshots: Vec<Snapshot>,
}

impl State {
    pub fn new() -> Self {
        Self {
            snapshots: Vec::new(),
            scroll_index: 0,
            view: TUIView::Main,
            input_buffer: String::new(),
        }
    }
}
