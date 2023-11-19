use tui::{
    style::{Color, Modifier, Style},
    widgets::{Cell, Row},
};

use crate::snapshot::structures::state::State;

/// A helper function used in many TUI views for rendering list rows, as well as handling scrolling
/// and selection.
pub fn build_rows(state: &mut State, max_row_height: usize) -> Vec<Row<'static>> {
    // ensure scroll index is within bounds
    if state.function_index >= state.snapshots.len() && state.function_index != 0 {
        state.function_index = state.snapshots.len() - 1;
    }

    // render storage slot list
    let mut rows = Vec::new();
    let snapshots = &state.snapshots;
    let num_items = std::cmp::min(max_row_height, snapshots.len());
    let indices = match state.function_index + num_items <= snapshots.len() {
        true => state.function_index..state.function_index + num_items,
        false => snapshots.len() - num_items..snapshots.len(),
    };

    let mut sorted_snapshots = snapshots[indices].to_vec();
    sorted_snapshots.sort_by(|a, b| a.selector.cmp(&b.selector));

    // slice storage_iter
    for (i, snapshot) in sorted_snapshots.iter().enumerate() {
        rows.push(
            Row::new(vec![Cell::from(format!(" 0x{} ", snapshot.selector))])
                .style(if snapshots.len() - state.function_index < num_items {
                    if (num_items - i <= snapshots.len() - state.function_index) &&
                        (num_items - i > snapshots.len() - state.function_index - 1)
                    {
                        if state.scroll {
                            Style::default().fg(Color::White)
                        } else {
                            Style::default().fg(Color::White).bg(Color::DarkGray)
                        }
                    } else {
                        Style::default().fg(Color::White).remove_modifier(Modifier::BOLD)
                    }
                } else if i == 0 {
                    if state.scroll {
                        Style::default().fg(Color::White)
                    } else {
                        Style::default().fg(Color::White).bg(Color::DarkGray)
                    }
                } else {
                    Style::default().fg(Color::White).remove_modifier(Modifier::BOLD)
                })
                .height(1)
                .bottom_margin(0),
        );
    }

    if rows.is_empty() {
        rows.push(
            Row::new(vec![Cell::from(" None Found ")])
                .style(Style::default().fg(Color::DarkGray))
                .height(1)
                .bottom_margin(0),
        );
    }

    rows
}

