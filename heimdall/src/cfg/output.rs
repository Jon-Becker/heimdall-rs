use std::{time::Duration};

use heimdall_common::{io::{logging::{TraceFactory, Logger}, file::{short_path, write_file}}};
use indicatif::ProgressBar;
use petgraph::{graph::Graph, dot::{Dot, Config}};

use super::{CFGArgs};

pub fn build_output(
    contract_cfg: &Graph<String, String>,
    args: &CFGArgs,
    output_dir: String,
    logger: &Logger,
    trace: &mut TraceFactory,
    trace_parent: u32
) {
    let progress_bar = ProgressBar::new_spinner();
    progress_bar.enable_steady_tick(Duration::from_millis(100));
    progress_bar.set_style(logger.info_spinner());
    progress_bar.set_message(format!("writing CFG .dot file"));

    let dot_output_path = format!("{}/cfg.dot", output_dir);
    let output = format!(
        "{}",
        Dot::with_config(&contract_cfg, &[])
    );

    // find regex matches and replace
    let output = output.replace(
        "digraph {",
        "digraph G {\n    node [shape=box, style=\"rounded\", fontname=\"Helvetica\"];\n"
    );

    write_file(&dot_output_path, &output);

    progress_bar.suspend(|| {
        logger.success(&format!("wrote generated dot to '{}' .", &dot_output_path).to_string());
    });
}