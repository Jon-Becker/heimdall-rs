use tui::{backend::Backend, Frame};

use super::structures::dump_state::DumpState;

pub mod command_palette;
pub mod help;
pub mod main;

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub enum TUIView {
    Killed,
    Main,
    CommandPalette,
    Help,
}

#[allow(unreachable_patterns)]
pub fn render_ui<B: Backend>(f: &mut Frame<B>, state: &mut DumpState) {
    match state.view {
        TUIView::Main => main::render_tui_view_main(f, state),
        TUIView::CommandPalette => command_palette::render_tui_command_palette(f, state),
        TUIView::Help => help::render_tui_help(f, state),
        _ => {}
    }
}
