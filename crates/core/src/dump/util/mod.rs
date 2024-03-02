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

use crate::error::Error;

/// cleanup the terminal, disable raw mode, and leave the alternate screen
pub fn cleanup_terminal() -> Result<(), Error> {
    let stdout = io::stdout();
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)
        .map_err(|_| Error::Generic("failed to create terminal".to_string()))?;
    disable_raw_mode().map_err(|_| Error::Generic("failed to disable raw mode".to_string()))?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)
        .map_err(|_| Error::Generic("failed to cleanup terminal".to_string()))?;
    terminal.show_cursor().map_err(|_| Error::Generic("failed to show cursor".to_string()))?;

    Ok(())
}
