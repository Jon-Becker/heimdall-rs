pub mod csv;
pub mod table;
pub mod threads;

use std::io;

use crossterm::{
    event::DisableMouseCapture,
    execute,
    terminal::{disable_raw_mode, LeaveAlternateScreen},
};
use tui::{backend::CrosstermBackend, Terminal};

/// cleanup the terminal, disable raw mode, and leave the alternate screen
pub fn cleanup_terminal() {
    let stdout = io::stdout();
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).unwrap();
    disable_raw_mode().unwrap();
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture).unwrap();
    terminal.show_cursor().unwrap();
}
