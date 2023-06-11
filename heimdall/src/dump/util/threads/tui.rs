use std::{io, time::Duration};

use crossterm::{
    event::EnableMouseCapture,
    execute,
    terminal::{enable_raw_mode, EnterAlternateScreen},
};
use tui::{backend::CrosstermBackend, Terminal};

use crate::dump::{
    constants::{DECODE_AS_TYPES, DUMP_STATE},
    tui_views::{render_ui, TUIView},
    util::{cleanup_terminal, csv::write_storage_to_csv},
    DumpArgs,
};

pub fn handle(args: DumpArgs, output_dir: String) {
    // if no TUI is requested, just run the dump
    if args.no_tui {
        return
    }

    // create new TUI terminal
    enable_raw_mode().unwrap();
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture).unwrap();
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).unwrap();

    loop {
        let mut state = DUMP_STATE.lock().unwrap();
        terminal
            .draw(|f| {
                render_ui(f, &mut state);
            })
            .unwrap();
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
                                }

                                // handle backspace
                                crossterm::event::KeyCode::Backspace => {
                                    state.input_buffer.pop();
                                }

                                // enter command
                                crossterm::event::KeyCode::Enter => {
                                    state.filter = String::new();
                                    let mut split = state.input_buffer.split(' ');
                                    let command = split.next().unwrap();
                                    let args = split.collect::<Vec<&str>>();

                                    match command {
                                        ":q" | ":quit" => {
                                            state.view = TUIView::Killed;
                                            break
                                        }
                                        ":h" | ":help" => {
                                            state.view = TUIView::Help;
                                        }
                                        ":f" | ":find" => {
                                            if !args.is_empty() {
                                                state.filter = args[0].to_string();
                                            }
                                            state.view = TUIView::Main;
                                        }
                                        ":e" | ":export" => {
                                            if !args.is_empty() {
                                                write_storage_to_csv(
                                                    &output_dir.clone(),
                                                    &args[0].to_string(),
                                                    &state,
                                                );
                                            }
                                            state.view = TUIView::Main;
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
                                                        if state.scroll_index + amount <
                                                            state.storage.len()
                                                        {
                                                            state.scroll_index += amount;
                                                        } else {
                                                            state.scroll_index =
                                                                state.storage.len() - 1;
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
                                }

                                // handle escape
                                crossterm::event::KeyCode::Esc => {
                                    state.filter = String::new();
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
                                state.filter = String::new();
                                state.view = TUIView::Main;
                            }

                            // select transaction
                            crossterm::event::KeyCode::Right => {
                                // increment decode_as_type_index on all selected transactions
                                let scroll_index = state.scroll_index;
                                let selection_size = state.selection_size;
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
                                    } else if i >= scroll_index + selection_size {
                                        break
                                    }
                                }
                            }

                            // deselect transaction
                            crossterm::event::KeyCode::Left => {
                                // decrement decode_as_type_index on all selected transactions
                                let scroll_index = state.scroll_index;
                                let selection_size = state.selection_size;
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
                                    } else if i >= scroll_index + selection_size {
                                        break
                                    }
                                }
                            }

                            // scroll down
                            crossterm::event::KeyCode::Down => {
                                state.selection_size = 1;
                                state.scroll_index += 1;
                            }

                            // scroll up
                            crossterm::event::KeyCode::Up => {
                                state.selection_size = 1;
                                if state.scroll_index > 0 {
                                    state.scroll_index -= 1;
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
                        let mut state = DUMP_STATE.lock().unwrap();
                        match mouse.kind {
                            // scroll down
                            crossterm::event::MouseEventKind::ScrollDown => {
                                // if shift is held, increase selection size
                                if mouse.modifiers.contains(crossterm::event::KeyModifiers::SHIFT) {
                                    state.selection_size += 1;
                                } else {
                                    state.selection_size = 1;
                                    state.scroll_index += 1;
                                }
                            }

                            // scroll up
                            crossterm::event::MouseEventKind::ScrollUp => {
                                // if shift is held, increase selection size
                                if mouse.modifiers.contains(crossterm::event::KeyModifiers::SHIFT) {
                                    state.selection_size -= 1;
                                } else {
                                    state.selection_size = 1;
                                    if state.scroll_index > 0 {
                                        state.scroll_index -= 1;
                                    }
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

    cleanup_terminal();
}
