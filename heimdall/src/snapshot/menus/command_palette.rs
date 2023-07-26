use tui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table},
    Frame,
};

use crate::snapshot::structures::state::State;

pub fn render_tui_command_palette<B: Backend>(f: &mut Frame<B>, state: &mut State) {
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

    f.render_widget(command_input, main_layout[0]);
}
