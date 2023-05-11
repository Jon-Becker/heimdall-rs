use heimdall_common::utils::time::{calculate_eta, format_eta};
use tui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Cell, Gauge, Row, Table},
    Frame,
};

use crate::dump::{structures::dump_state::DumpState, util::table::build_rows};

pub fn render_tui_view_main<B: Backend>(f: &mut Frame<B>, state: &mut DumpState) {
    // build main layout
    let main_layout = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([Constraint::Length(3), Constraint::Percentage(100)].as_ref())
        .split(f.size());

    let min_block_number =
        state.transactions.iter().min_by_key(|t| t.block_number).unwrap().block_number;
    let max_block_number =
        state.transactions.iter().max_by_key(|t| t.block_number).unwrap().block_number;
    let max_indexed_block_number =
        match state.transactions.iter().filter(|t| t.indexed).max_by_key(|t| t.block_number) {
            Some(t) => t.block_number,
            None => min_block_number,
        };

    // calculate progress and stats
    let transactions_indexed = state.transactions.iter().filter(|t| t.indexed).count();
    let transactions_total = state.transactions.len();
    let transactions_remaining = transactions_total - transactions_indexed;
    let percent_indexed = (transactions_indexed as f64 / transactions_total as f64) * 100.0;
    let elapsed_seconds = state.start_time.elapsed().as_secs();
    let transactions_per_second = transactions_indexed as f64 / elapsed_seconds as f64;

    // render progress bar
    let progress = Gauge::default()
        .block(Block::default().title(" Dump Progress ").borders(Borders::ALL))
        .gauge_style(Style::default().fg(Color::White).bg(Color::DarkGray))
        .percent(percent_indexed as u16)
        .label(if transactions_indexed != transactions_total {
            format!(
                "Block {}/{} ({:.2}%). {:.2} TPS. ETA: {}",
                max_indexed_block_number,
                max_block_number,
                percent_indexed,
                transactions_per_second,
                format_eta(calculate_eta(transactions_per_second, transactions_remaining))
            )
        } else {
            String::from("Storage Slot Dump Complete")
        });

    // build header cells
    let header_cells = ["Last Modified", "Slot", "As Type", "Value"].iter().map(|h| {
        Cell::from(*h).style(Style::default().fg(Color::White).add_modifier(Modifier::BOLD))
    });

    // build header row
    let header = Row::new(header_cells)
        .style(Style::default().fg(Color::White).add_modifier(Modifier::BOLD))
        .height(1)
        .bottom_margin(1);

    let rows = build_rows(state, main_layout[1].height as usize - 4);

    // render table
    let table = Table::new(rows)
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" Storage for Contract {} ", &state.args.target)),
        )
        .widths(&[
            Constraint::Length(14),
            Constraint::Length(68),
            Constraint::Length(9),
            Constraint::Percentage(100),
        ]);

    f.render_widget(progress, main_layout[0]);
    f.render_widget(table, main_layout[1]);
}
