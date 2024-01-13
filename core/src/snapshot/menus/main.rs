use heimdall_common::utils::strings::encode_hex_reduced;
use tui::{
    backend::Backend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Span, Spans},
    widgets::{Block, Borders, Paragraph, Table, Wrap},
    Frame,
};

use crate::{
    error::Error,
    snapshot::{structures::state::State, util::table::build_rows},
};

/// Render the TUI main view
pub fn render_tui_view_main<B: Backend>(f: &mut Frame<B>, state: &mut State) -> Result<(), Error> {
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
    let header = Paragraph::new(format!(
        "heimdall-rs v{}{:>space$}",
        env!("CARGO_PKG_VERSION"),
        "type :q to exit",
        space = f.size().width as usize - 20
    ))
    .style(Style::default().fg(Color::White))
    .block(create_block(
        format!(
            "Snapshot of Contract {:<space$} {} {}",
            state.target,
            state.compiler.0,
            state.compiler.1,
            space = f.size().width as usize - state.target.len() + 17 -
                state.compiler.0.len() -
                state.compiler.1.len()
        ),
        Borders::BOTTOM,
    ))
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
    let snapshot = state.snapshots.get(state.function_index).ok_or_else(|| {
        Error::Generic("impossible case: function index out of bounds".to_owned())
    })?;

    // build modifiers
    let modifiers = [
        if snapshot.payable { "payable" } else { "" },
        if snapshot.pure { "pure" } else { "" },
        if snapshot.view && !snapshot.pure { "view" } else { "" },
    ]
    .iter()
    .filter(|x| !x.is_empty())
    .map(|x| x.to_string())
    .collect::<Vec<_>>();

    // build argument list
    let mut arg_strings: Vec<String> = Vec::new();
    match &snapshot.resolved_function {
        Some(function) => {
            for (index, input) in function.inputs.iter().enumerate() {
                arg_strings.push(format!("arg{} {}", index, input));
            }
        }
        None => {
            let mut sorted_arguments: Vec<_> = snapshot.arguments.clone().into_iter().collect();
            sorted_arguments.sort_by(|x, y| x.0.cmp(&y.0));
            for (index, (_, solidity_type)) in sorted_arguments {
                arg_strings.push(format!(
                    "arg{} {}",
                    index,
                    solidity_type
                        .first()
                        .expect("impossible case: list of potential types is empty")
                ));
            }
        }
    };

    // add function resolved name
    let mut text = vec![
        Spans::from(""), // buffer
        Spans::from(Span::styled(
            " Function ",
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        )),
        Spans::from(match &snapshot.resolved_function {
            Some(function) => format!(" {}({})", function.name, arg_strings.join(", ")),
            None => format!(" Unresolved_{}()", snapshot.selector),
        }),
    ];

    // build function snapshot
    text.append(&mut vec![
        // add modifiers and arguments
        Spans::from(""), // buffer
        Spans::from(Span::styled(
            " Modifiers       Returns        Entry Point      Branch Count",
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        )),
        Spans::from(format!(
            " {:<16}{:<15}{:<17}{}",
            modifiers.join(" "),
            snapshot.returns.clone().unwrap_or("None".to_owned()),
            snapshot.entry_point,
            snapshot.branch_count
        )),
    ]);

    // add gas consumptions
    text.append(&mut vec![
        // add modifiers and arguments
        Spans::from(""), // buffer
        Spans::from(Span::styled(
            " Minimum Gas Consumed    Maximum Gas Consumed    Average Gas Consumed",
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        )),
        Spans::from(format!(
            " {:<24}{:<25}{}",
            snapshot.gas_used.min, snapshot.gas_used.max, snapshot.gas_used.avg
        )),
    ]);

    // add events
    if !snapshot.events.is_empty() {
        text.append(&mut vec![
            Spans::from(""), // buffer
            Spans::from(Span::styled(
                " Events ",
                Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
            )),
        ]);
        text.append(
            &mut snapshot
                .events
                .iter()
                .map(|x| {
                    let key = encode_hex_reduced(*x.0).replacen("0x", "", 1);
                    match state.resolved_events.get(&key) {
                        Some(event) => {
                            Spans::from(format!(" {}({})", event.name, event.inputs.join(",")))
                        }
                        None => Spans::from(format!(" Event_{}()", key[0..8].to_owned())),
                    }
                })
                .collect::<Vec<_>>(),
        );
    }

    // add errors
    if !snapshot.errors.is_empty() {
        text.append(&mut vec![
            Spans::from(""), // buffer
            Spans::from(Span::styled(
                " Errors ",
                Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
            )),
        ]);
        text.append(
            &mut snapshot
                .errors
                .iter()
                .map(|x| {
                    let key = encode_hex_reduced(*x.0).replacen("0x", "", 1);
                    match state.resolved_errors.get(&key) {
                        Some(error) => {
                            Spans::from(format!(" {}({})", error.name, error.inputs.join(",")))
                        }
                        None => Spans::from(format!(" Error_{}()", key[0..8].to_owned())),
                    }
                })
                .collect::<Vec<_>>(),
        );
    }

    // add external calls
    if !snapshot.external_calls.is_empty() {
        text.append(&mut vec![
            Spans::from(""), // buffer
            Spans::from(Span::styled(
                " External Calls ",
                Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
            )),
        ]);
        text.append(
            &mut snapshot
                .external_calls
                .iter()
                .map(|x| Spans::from(format!(" {}", x)))
                .collect::<Vec<_>>(),
        );
    }

    // add strings
    if !snapshot.strings.is_empty() {
        text.append(&mut vec![
            Spans::from(""), // buffer
            Spans::from(Span::styled(
                " Strings ",
                Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
            )),
        ]);
        text.append(
            &mut snapshot
                .strings
                .iter()
                .map(|x| Spans::from(format!(" {}", x)))
                .collect::<Vec<_>>(),
        );
    }

    // add addresses
    if !snapshot.addresses.is_empty() {
        text.append(&mut vec![
            Spans::from(""), // buffer
            Spans::from(Span::styled(
                " Hardcoded Addresses ",
                Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
            )),
        ]);
        text.append(
            &mut snapshot
                .addresses
                .iter()
                .map(|x| Spans::from(format!(" {}", x)))
                .collect::<Vec<_>>(),
        );
    }

    // add storage
    if !snapshot.storage.is_empty() {
        text.append(&mut vec![
            Spans::from(""), // buffer
            Spans::from(Span::styled(
                " Storage ",
                Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
            )),
        ]);
        text.append(
            &mut snapshot
                .storage
                .iter()
                .map(|x| Spans::from(format!(" {}", x)))
                .collect::<Vec<_>>(),
        );
    }

    // add control statements
    if !snapshot.control_statements.is_empty() {
        text.append(&mut vec![
            Spans::from(""), // buffer
            Spans::from(Span::styled(
                " Control Statements ",
                Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
            )),
        ]);
        text.append(
            &mut snapshot
                .control_statements
                .iter()
                .map(|x| Spans::from(format!(" {}", x)))
                .collect::<Vec<_>>(),
        );
    }

    // about text
    let snapshot_header = format!(
        " {}Snapshot of 0x{}{} ",
        if state.scroll { "> " } else { "" },
        snapshot.selector,
        if state.scroll { " <" } else { "" },
    );
    let function_snapshot = Paragraph::new(text)
        .style(Style::default().fg(Color::White))
        .block(create_block(snapshot_header, Borders::ALL))
        .alignment(Alignment::Left)
        .scroll((state.scroll_index as u16, 0));

    f.render_widget(header, main_layout[0]);
    f.render_widget(table, sub_layout[0]);
    f.render_widget(function_snapshot, detail_layout[0]);

    Ok(())
}
