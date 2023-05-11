use tui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table},
    Frame,
};

use crate::dump::{structures::dump_state::DumpState, util::table::build_rows};

pub fn render_tui_command_palette<B: Backend>(f: &mut Frame<B>, state: &mut DumpState) {
    // build main layout
    let main_layout = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([Constraint::Length(3), Constraint::Percentage(100)].as_ref())
        .split(f.size());

    // add command paragraph input
    let input_buffer = state.input_buffer.clone();
    let command_input = Paragraph::new(input_buffer)
        .style(Style::default().fg(Color::White))
        .block(Block::default().title(" Command ").borders(Borders::ALL));

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

    f.render_widget(command_input, main_layout[0]);
    f.render_widget(table, main_layout[1]);
}
