use tui::{
    backend::Backend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Span, Spans},
    widgets::{Block, Borders, Cell, Gauge, Paragraph, Row, Table, Wrap},
    Frame,
};

use crate::snapshot::{structures::state::State, util::table::build_rows};

pub fn render_tui_view_main<B: Backend>(f: &mut Frame<B>, state: &mut State) {
    // creates a new block with the given title
    // https://github.com/fdehau/tui-rs/blob/master/examples/paragraph.rs
    let create_block = |title, borders| {
        Block::default()
            .borders(borders)
            .style(Style::default().fg(Color::White))
            .title(Span::styled(title, Style::default().add_modifier(Modifier::BOLD)))
    };

    // build main layout
    let main_layout = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([Constraint::Length(3), Constraint::Percentage(100)].as_ref())
        .split(f.size());

    let sub_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(14), Constraint::Percentage(100)].as_ref())
        .split(main_layout[1]);

    let detail_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(100)].as_ref())
        .split(sub_layout[1]);

    // about text
    let header = Paragraph::new(format!("heimdall-rs v{}", env!("CARGO_PKG_VERSION")))
        .style(Style::default().fg(Color::White))
        .block(create_block("Contract Snapshot", Borders::BOTTOM))
        .alignment(Alignment::Left)
        .wrap(Wrap { trim: true });

    // build rows
    let rows = build_rows(state, main_layout[1].height as usize - 4);

    // build table
    let table = Table::new(rows)
        .block(
            Block::default()
                .title(" Selectors ")
                .style(Style::default().fg(Color::White).add_modifier(Modifier::BOLD))
                .borders(Borders::ALL),
        )
        .widths(&[Constraint::Length(12), Constraint::Length(14), Constraint::Percentage(100)]);

    // build function info
    let snapshot = state.snapshots.get(state.scroll_index).unwrap();

    // build modifiers
    let modifiers = vec![
        if snapshot.payable { "payable" } else { "" },
        if snapshot.pure { "pure" } else { "" },
        if snapshot.view && !snapshot.pure { "view" } else { "" },
    ]
    .iter()
    .filter(|x| x.len() > 0)
    .map(|x| x.to_string())
    .collect::<Vec<_>>();

    // build argument list
    let mut arg_strings: Vec<String> = Vec::new();
    let mut sorted_arguments: Vec<_> = snapshot.arguments.clone().into_iter().collect();
    sorted_arguments.sort_by(|x, y| x.0.cmp(&y.0));
    for (index, (_, solidity_type)) in sorted_arguments {
        arg_strings.push(format!("arg{} {}", index, solidity_type.first().unwrap()));
    }

    let text = vec![
        // add modifiers and arguments
        Spans::from(""), // buffer
        Spans::from(Span::styled(
            " Modifiers       Returns        Arguments",
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        )),
        Spans::from(format!(
            " {:<16}{:<15}{}",
            modifiers.join(" "),
            snapshot.returns.clone().unwrap_or("None".to_owned()),
            if arg_strings.len() > 0 { arg_strings.join(", ") } else { "None".to_owned() }
        )),
        // add events
        Spans::from(""), // buffer
        Spans::from(Span::styled(
            " Events ",
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        )),
        Spans::from(if snapshot.events.len() > 0 {
            snapshot.events.iter().map(|x| x.0.to_string()).collect::<Vec<_>>().join(", ")
        } else {
            " None".to_owned()
        }),
        // add errors
        Spans::from(""), // buffer
        Spans::from(Span::styled(
            " Errors ",
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        )),
        Spans::from(if snapshot.errors.len() > 0 {
            snapshot.errors.iter().map(|x| x.0.to_string()).collect::<Vec<_>>().join(", ")
        } else {
            " None".to_owned()
        }),
    ];

    // about text
    let snapshot_header = format!("Snapshot of 0x{}", snapshot.selector);
    let function_snapshot = Paragraph::new(text)
        .style(Style::default().fg(Color::White))
        .block(create_block(&snapshot_header, Borders::ALL))
        .alignment(Alignment::Left);

    f.render_widget(header, main_layout[0]);
    f.render_widget(table, sub_layout[0]);
    f.render_widget(function_snapshot, detail_layout[0]);
}
