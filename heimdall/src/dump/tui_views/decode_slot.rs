use heimdall_common::utils::{strings::encode_hex};
use tui::{backend::Backend, Frame, layout::{Layout, Constraint, Direction}, widgets::{Block, Borders, Cell, Row, Table}, style::{Style, Color, Modifier}};

use crate::dump::{DumpState};

pub fn render_tui_decode_slot<B: Backend>(
    f: &mut Frame<B>,
    state: &mut DumpState
) {

    // build main layout
    let main_layout = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints(
            [
                Constraint::Length(std::cmp::min(4+state.selection_size, 15) as u16),
                Constraint::Percentage(100),
            ].as_ref()
        ).split(f.size());
    
    // build header cells
    let header_cells = ["Slot", "Block Number", "Value", "Modifiers"]
        .iter()
        .map(|h| Cell::from(*h).style(Style::default().fg(Color::White).add_modifier(Modifier::BOLD)));
    
    // build header row
    let header = Row::new(header_cells)
        .style(Style::default().fg(Color::White).add_modifier(Modifier::BOLD))
        .height(1)
        .bottom_margin(1);

    // ensure scroll index is within bounds
    if state.scroll_index >= state.storage.len() {
        state.scroll_index = state.storage.len() - 1;
    }

    // render storage slot list
    let mut all_rows = Vec::new();
    let mut storage_iter = state.storage.iter().collect::<Vec<_>>();

    // sort by slot
    storage_iter.sort_by_key(|(slot, _)| *slot);
    for (i, (slot, value)) in storage_iter.iter().enumerate() {
        if i >= state.scroll_index && i < state.scroll_index + state.selection_size {
            all_rows.push(
                Row::new(vec![
                    Cell::from(format!("0x{}", encode_hex(slot.to_fixed_bytes().into()))),
                    Cell::from(value.modifiers.iter().max_by_key(|m| m.0).unwrap().0.to_string()),
                    Cell::from(format!("0x{}", encode_hex(value.value.to_fixed_bytes().into())))
                ])
                .style(Style::default().fg(Color::White))
                .height(1)
                .bottom_margin(0)
            );
        }
    }

    // if all_rows > 10, add ...
    if all_rows.len() > 10 {

        // save the last row
        let last_row = all_rows.pop().unwrap();

        // slice to 10
        all_rows = all_rows[0..9].to_vec();

        // add ellipsis
        all_rows.push(
            Row::new(vec![
                Cell::from("..."),
                Cell::from("..."),
                Cell::from("..."),
            ])
            .style(Style::default().fg(Color::White))
            .height(1)
            .bottom_margin(0)
        );

        // add last row
        all_rows.push(last_row);
    }

    // render table
    let table = Table::new(all_rows)
        .header(header)
        .block(Block::default().borders(Borders::ALL)
        .title(" Decoding Storage Values "))
        .widths(&[
            Constraint::Length(68),
            Constraint::Length(14),
            Constraint::Percentage(68),
            Constraint::Percentage(68),
        ]);

    f.render_widget(table, main_layout[0]);
}