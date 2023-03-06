use heimdall_common::utils::strings::encode_hex;
use tui::{widgets::{Row, Cell}, style::{Style, Color}};

use crate::dump::DumpState;

pub fn build_rows(mut state: &mut DumpState, max_row_height: usize) -> Vec<Row<'static>> {

    // ensure scroll index is within bounds
    if state.scroll_index >= state.storage.len() && state.scroll_index != 0 {
        state.scroll_index = state.storage.len() - 1;
    }

    // render storage slot list
    let mut rows = Vec::new();
    let mut storage_iter =  state.storage.iter().collect::<Vec<_>>();

    // sort storage slots by slot
    storage_iter.sort_by_key(|(slot, _)| *slot);
    let num_items = std::cmp::min(max_row_height, storage_iter.len());

    let indices = match state.scroll_index + num_items <= storage_iter.len() {
        true => state.scroll_index..state.scroll_index + num_items,
        false => storage_iter.len() - num_items..storage_iter.len(),
    };

    // slice storage_iter
    for (i, (slot, value)) in storage_iter[indices.clone()].iter().enumerate() {
        rows.push(
            Row::new(vec![
                Cell::from(format!("0x{}", encode_hex(slot.to_fixed_bytes().into()))),
                Cell::from(value.modifiers.iter().max_by_key(|m| m.0).unwrap().0.to_string()),
                Cell::from(format!("0x{}", encode_hex(value.value.to_fixed_bytes().into()))),
                Cell::from(value.modifiers.len().to_string())
            ])
            .style(
                if storage_iter.len() - state.scroll_index < num_items {
                    if (num_items - i <= storage_iter.len() - state.scroll_index) && (num_items - i > storage_iter.len() - state.scroll_index - state.selection_size){
                        Style::default().fg(Color::White).bg(Color::DarkGray)
                    }
                    else {
                        Style::default().fg(Color::White)
                    }
                }
                else if i == 0 || i < state.selection_size {
                    Style::default().fg(Color::White).bg(Color::DarkGray)
                }
                else {
                    Style::default().fg(Color::White)
                }
            )
            .height(1)
            .bottom_margin(0)
        );
    }

    rows
}