use std::{collections::HashMap, io, time::Duration};

use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use heimdall_common::ether::{
    compiler::Compiler,
    signatures::{ResolvedError, ResolvedLog},
};
use tui::{backend::CrosstermBackend, Terminal};

use crate::{
    error::Error,
    snapshot::{
        constants::STATE,
        menus::{render_ui, TUIView},
        structures::snapshot::Snapshot,
    },
};

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

/// The TUI thread handler, which will be called from the main thread.
pub fn handle(
    snapshots: Vec<Snapshot>,
    resolved_errors: &HashMap<String, ResolvedError>,
    resolved_events: &HashMap<String, ResolvedLog>,
    target: &str,
    compiler: (Compiler, &str),
) -> Result<(), Error> {
    // create new TUI terminal
    enable_raw_mode().map_err(|_| Error::Generic("failed to enable raw mode".to_string()))?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)
        .map_err(|_| Error::Generic("failed to enter alternate screen".to_string()))?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)
        .map_err(|_| Error::Generic("failed to create terminal".to_string()))?;

    // initialize state
    let mut state =
        STATE.lock().map_err(|_| Error::Generic("failed to obtain lock on state".to_string()))?;
    state.snapshots = snapshots;
    state.resolved_errors = resolved_errors.clone();
    state.resolved_events = resolved_events.clone();
    state.target = target.to_string();
    state.compiler = (compiler.0.to_string(), compiler.1.to_string());
    drop(state);

    loop {
        let mut state = STATE
            .lock()
            .map_err(|_| Error::Generic("failed to obtain lock on state".to_string()))?;
        terminal
            .draw(|f| {
                let _ = render_ui(f, &mut state);
            })
            .map_err(|_| Error::Generic("failed to render ui".to_string()))?;
        drop(state);

        // check for user input
        if crossterm::event::poll(Duration::from_millis(10))
            .map_err(|_| Error::Generic("failed to poll for events".to_string()))?
        {
            if let Ok(event) = crossterm::event::read() {
                match event {
                    crossterm::event::Event::Key(key) => {
                        let mut state = STATE.lock().map_err(|_| {
                            Error::Generic("failed to obtain lock on state".to_string())
                        })?;

                        // ignore key events if command palette is open
                        if state.view == TUIView::CommandPalette {
                            match key.code {
                                // handle keys in command palette
                                crossterm::event::KeyCode::Char(c) => {
                                    state.input_buffer.push(c);
                                }

                                // handle backspace
                                crossterm::event::KeyCode::Backspace => {
                                    state.input_buffer.pop();
                                }

                                // enter command
                                crossterm::event::KeyCode::Enter => {
                                    let mut split = state.input_buffer.split(' ');
                                    let command = split.next().unwrap_or("");
                                    let _args = split.collect::<Vec<&str>>();

                                    match command {
                                        ":q" | ":quit" => {
                                            state.view = TUIView::Killed;
                                            break
                                        }
                                        ":h" | ":help" => {
                                            state.view = TUIView::Help;
                                        }
                                        _ => {
                                            state.view = TUIView::Main;
                                        }
                                    }
                                }

                                // handle escape
                                crossterm::event::KeyCode::Esc => {
                                    state.view = TUIView::Main;
                                }

                                _ => {}
                            }

                            drop(state);
                            continue
                        }

                        match key.code {
                            // main on escape
                            crossterm::event::KeyCode::Esc => {
                                state.view = TUIView::Main;
                            }

                            // select transaction
                            crossterm::event::KeyCode::Right => {
                                state.scroll = true;
                            }

                            // deselect transaction
                            crossterm::event::KeyCode::Left => {
                                state.scroll = false;
                            }

                            // scroll down
                            crossterm::event::KeyCode::Down => {
                                if state.scroll {
                                    state.scroll_index += 1;
                                } else {
                                    state.scroll_index = 0;
                                    state.function_index += 1;
                                }
                            }

                            // scroll up
                            crossterm::event::KeyCode::Up => {
                                if state.scroll {
                                    if state.scroll_index > 0 {
                                        state.scroll_index -= 1;
                                    }
                                } else if state.function_index > 0 {
                                    state.scroll_index = 0;
                                    state.function_index -= 1;
                                }
                            }

                            // toggle command palette on ":"
                            crossterm::event::KeyCode::Char(':') => match state.view {
                                TUIView::CommandPalette => {
                                    state.view = TUIView::Main;
                                }
                                _ => {
                                    state.input_buffer = String::from(":");
                                    state.view = TUIView::CommandPalette;
                                }
                            },

                            _ => {}
                        }
                        drop(state)
                    }
                    crossterm::event::Event::Mouse(mouse) => {
                        let mut state = STATE.lock().map_err(|_| {
                            Error::Generic("failed to obtain lock on state".to_string())
                        })?;
                        match mouse.kind {
                            // scroll down
                            crossterm::event::MouseEventKind::ScrollDown => {
                                if state.scroll {
                                    state.scroll_index += 1;
                                } else {
                                    state.scroll_index = 0;
                                    state.function_index += 1;
                                }
                            }

                            // scroll up
                            crossterm::event::MouseEventKind::ScrollUp => {
                                if state.scroll {
                                    if state.scroll_index > 0 {
                                        state.scroll_index -= 1;
                                    }
                                } else if state.function_index > 0 {
                                    state.scroll_index = 0;
                                    state.function_index -= 1;
                                }
                            }
                            _ => {}
                        }
                        drop(state);
                    }
                    _ => {}
                }
            }
        }
    }

    cleanup_terminal()?;

    Ok(())
}
