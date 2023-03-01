use tui::{backend::Backend, Frame, layout::{Layout, Constraint, Direction}, widgets::{Gauge, ListItem, List, Block, Borders}, style::{Style, Color, Modifier}};

use crate::dump::{DumpState};

pub fn render_tui_view_main<B: Backend>(
    f: &mut Frame<B>,
    state: &mut DumpState
) {

    // build main layout
    let main_layout = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints(
            [
                Constraint::Length(3),
                Constraint::Percentage(100),
            ].as_ref()
        ).split(f.size());

    let min_block_number = state.transactions.iter().min_by_key(|t| t.block).unwrap().block;
    let max_block_number = state.transactions.iter().max_by_key(|t| t.block).unwrap().block;
    let max_indexed_block_number = match state.transactions.iter().filter(|t| t.indexed).max_by_key(|t| t.block) {
        Some(t) => t.block,
        None => min_block_number
    };

    let blocks_indexed = max_indexed_block_number - min_block_number;
    let percent_indexed = (max_indexed_block_number - min_block_number) as f64 / (max_block_number - min_block_number) as f64 * 100.0;
    let elapsed_seconds = state.start_time.elapsed().as_secs();
    let blocks_per_second = blocks_indexed as f64 / elapsed_seconds as f64;

    

    // render progress bar
    let progress = Gauge::default()
        .block(Block::default().title(" Dump Progress ").borders(Borders::ALL))
        .gauge_style(Style::default().fg(Color::White).bg(Color::DarkGray))
        .percent(percent_indexed as u16)
        .label(format!(
            "Block {}/{} ({:.1}%). {} Blocks Per Second. ETA: {}",
            max_indexed_block_number,
            max_block_number,
            percent_indexed,
            blocks_per_second,
            0
        ));

    let items = [ListItem::new("Item 1"), ListItem::new("Item 2"), ListItem::new("Item 3")];
    let list = List::new(items)
        .block(Block::default().title("List").borders(Borders::ALL))
        .style(Style::default().fg(Color::White))
        .highlight_style(Style::default().add_modifier(Modifier::ITALIC))
        .highlight_symbol(">>");

    f.render_widget(progress, main_layout[0]);
    f.render_widget(list, main_layout[1]);
}