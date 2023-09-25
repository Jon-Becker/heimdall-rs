use tui::{
    backend::Backend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::dump::{
    constants::{ABOUT_TEXT, HELP_MENU_COMMANDS, HELP_MENU_CONTROLS},
    structures::dump_state::DumpState,
};

pub fn render_tui_help<B: Backend>(f: &mut Frame<B>, _: &mut DumpState) {
    // build main layout
    let main_layout = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints(
            [
                Constraint::Length(6),
                Constraint::Length((HELP_MENU_COMMANDS.len() + 2).try_into().unwrap()),
                Constraint::Percentage(100),
            ]
            .as_ref(),
        )
        .split(f.size());

    // creates a new block with the given title
    // https://github.com/fdehau/tui-rs/blob/master/examples/paragraph.rs
    let create_block = |title| {
        Block::default()
            .borders(Borders::NONE)
            .style(Style::default().fg(Color::White))
            .title(Span::styled(title, Style::default().add_modifier(Modifier::BOLD)))
    };

    // about text
    let paragraph = Paragraph::new(ABOUT_TEXT.join("\n"))
        .style(Style::default().fg(Color::White))
        .block(create_block("About"))
        .alignment(Alignment::Left)
        .wrap(Wrap { trim: true });
    f.render_widget(paragraph, main_layout[0]);

    // commands paragraph
    let paragraph = Paragraph::new(HELP_MENU_COMMANDS.join("\n"))
        .style(Style::default().fg(Color::White))
        .block(create_block("Commands"))
        .alignment(Alignment::Left)
        .wrap(Wrap { trim: true });
    f.render_widget(paragraph, main_layout[1]);

    // controls paragraph
    let paragraph = Paragraph::new(HELP_MENU_CONTROLS.join("\n"))
        .style(Style::default().fg(Color::White))
        .block(create_block("Controls"))
        .alignment(Alignment::Left)
        .wrap(Wrap { trim: true });
    f.render_widget(paragraph, main_layout[2]);
}
