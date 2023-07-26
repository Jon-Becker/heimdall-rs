use tui::{
    style::{Color, Modifier, Style},
    widgets::{Cell, Row},
};

use crate::snapshot::structures::state::State;

pub fn build_rows(state: &mut State, max_row_height: usize) -> Vec<Row<'static>> {
    // ensure scroll index is within bounds
    if state.scroll_index >= state.snapshots.len() && state.scroll_index != 0 {
        state.scroll_index = state.snapshots.len() - 1;
    }

    // render storage slot list
    let mut rows = Vec::new();
    let snapshots = &state.snapshots;
    let num_items = std::cmp::min(max_row_height, snapshots.len());
    let indices = match state.scroll_index + num_items <= snapshots.len() {
        true => state.scroll_index..state.scroll_index + num_items,
        false => snapshots.len() - num_items..snapshots.len(),
    };

    // slice storage_iter
    for (i, snapshot) in snapshots[indices].iter().enumerate() {
        rows.push(
            Row::new(vec![Cell::from(format!(" 0x{} ", snapshot.selector))])
                .style(if snapshots.len() - state.scroll_index < num_items {
                    if (num_items - i <= snapshots.len() - state.scroll_index) &&
                        (num_items - i > snapshots.len() - state.scroll_index - 1)
                    {
                        Style::default().fg(Color::White).bg(Color::DarkGray)
                    } else {
                        Style::default().fg(Color::White).remove_modifier(Modifier::BOLD)
                    }
                } else if i == 0 {
                    Style::default().fg(Color::White).bg(Color::DarkGray)
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
