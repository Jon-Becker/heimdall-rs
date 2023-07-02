use ethers::{
    abi::{decode, ParamType},
    types::U256,
};
use heimdall_common::utils::strings::{encode_hex, hex_to_ascii};
use tui::{
    style::{Color, Style},
    widgets::{Cell, Row},
};

use crate::dump::{constants::DECODE_AS_TYPES, structures::dump_state::DumpState};

pub fn build_rows(mut state: &mut DumpState, max_row_height: usize) -> Vec<Row<'static>> {
    // ensure scroll index is within bounds
    if state.scroll_index >= state.storage.len() && state.scroll_index != 0 {
        state.scroll_index = state.storage.len() - 1;
    }

    // render storage slot list
    let mut rows = Vec::new();

    // filter storage_iter by state.filter
    let mut storage_iter = match !state.filter.is_empty() {
        true => state
            .storage
            .iter()
            .filter(|(slot, value)| {
                let slot = format!("0x{}", encode_hex(slot.to_fixed_bytes().into()));
                let value = format!("0x{}", encode_hex(value.value.to_fixed_bytes().into()));
                slot.contains(&state.filter) || value.contains(&state.filter)
            })
            .collect::<Vec<_>>(),
        false => state.storage.iter().collect::<Vec<_>>(),
    };

    // sort storage slots by slot
    storage_iter.sort_by_key(|(slot, _)| *slot);
    let num_items = std::cmp::min(max_row_height, storage_iter.len());

    let indices = match state.scroll_index + num_items <= storage_iter.len() {
        true => state.scroll_index..state.scroll_index + num_items,
        false => storage_iter.len() - num_items..storage_iter.len(),
    };

    // slice storage_iter
    for (i, (slot, value)) in storage_iter[indices].iter().enumerate() {
        let decoded_value = match value.decode_as_type_index {
            0 => format!("0x{}", encode_hex(value.value.to_fixed_bytes().into())),
            1 => format!("{}", !value.value.is_zero()),
            2 => format!(
                "0x{}",
                encode_hex(value.value.to_fixed_bytes().into()).get(24..).unwrap_or("")
            ),
            3 => match decode(&[ParamType::String], value.value.as_bytes()) {
                Ok(decoded) => decoded[0].to_string(),
                Err(_) => hex_to_ascii(&encode_hex(value.value.to_fixed_bytes().into())),
            },
            4 => {
                let decoded = U256::from_big_endian(&value.value.to_fixed_bytes());
                format!("{decoded}")
            }
            _ => "decoding error".to_string(),
        };

        rows.push(
            Row::new(vec![
                Cell::from(value.modifiers.iter().max_by_key(|m| m.0).unwrap().0.to_string()),
                Cell::from(format!("0x{}", encode_hex(slot.to_fixed_bytes().into()))),
                Cell::from(DECODE_AS_TYPES[value.decode_as_type_index].clone()),
                Cell::from(decoded_value),
            ])
            .style(if storage_iter.len() - state.scroll_index < num_items {
                if (num_items - i <= storage_iter.len() - state.scroll_index) &&
                    (num_items - i >
                        storage_iter.len() - state.scroll_index - state.selection_size)
                {
                    Style::default().fg(Color::White).bg(Color::DarkGray)
                } else {
                    Style::default().fg(Color::White)
                }
            } else if i == 0 || i < state.selection_size {
                Style::default().fg(Color::White).bg(Color::DarkGray)
            } else {
                Style::default().fg(Color::White)
            })
            .height(1)
            .bottom_margin(0),
        );
    }

    if rows.is_empty() {
        rows.push(
            Row::new(vec![
                Cell::from("No Results Found"),
                Cell::from(""),
                Cell::from(""),
                Cell::from(""),
            ])
            .style(Style::default().fg(Color::DarkGray))
            .height(1)
            .bottom_margin(0),
        );
    }

    rows
}
