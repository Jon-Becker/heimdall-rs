use std::{process::Command, time::Duration};

use heimdall_common::io::{file::write_file, logging::Logger};
use indicatif::ProgressBar;
use petgraph::{dot::Dot, graph::Graph};

use super::CFGArgs;

pub fn build_output(
    contract_cfg: &Graph<String, String>,
    args: &CFGArgs,
    output_dir: String,
    logger: &Logger,
) {
    let progress_bar = ProgressBar::new_spinner();
    progress_bar.enable_steady_tick(Duration::from_millis(100));
    progress_bar.set_style(logger.info_spinner());
    progress_bar.set_message("writing CFG .dot file".to_string());

    let dot_output_path = format!("{output_dir}/cfg.dot");
    let output = format!("{}", Dot::with_config(&contract_cfg, &[]));

    // find regex matches and replace
    let mut output = output.replace(
        "digraph {",
        "digraph G {\n    node [shape=box, style=\"rounded\", fontname=\"Helvetica\"];\n    edge [fontname=\"Helvetica\"];"
    );

    if args.color_edges {
        // replace edge labels with colors
        output = output.replace("[ label = \"true\" ]", "[ color = \"green\" ]");
        output = output.replace("[ label = \"false\" ]", "[ color = \"red\" ]");
    } else {
        // remove edge labels
        output = output.replace("[ label = \"true\" ]", "[]");
        output = output.replace("[ label = \"false\" ]", "[]");
    }

    output = output.replace("[ label = \"\" ]", "[]");

    write_file(&dot_output_path, &output);

    progress_bar.suspend(|| {
        logger.success(&format!("wrote generated dot to '{}' .", &dot_output_path));
    });

    if !args.format.is_empty() {
        // check for graphviz
        match Command::new("dot").spawn() {
            Ok(_) => {
                progress_bar.set_message(format!("generating CFG .{} file", &args.format));

                let image_output_path = format!("{}/cfg.{}", output_dir, &args.format);
                match Command::new("dot").arg("-T").arg(&args.format).arg(&dot_output_path).output()
                {
                    Ok(output) => {
                        match String::from_utf8(output.stdout) {
                            Ok(output) => {
                                // write the output
                                write_file(&image_output_path, &output);
                                progress_bar.suspend(|| {
                                    logger.success(&format!(
                                        "wrote generated {} to '{}' .",
                                        &args.format, &image_output_path
                                    ));
                                });
                            }
                            Err(_) => {
                                progress_bar.suspend(|| {
                                    logger.error(&format!(
                                        "graphviz failed to generate {} file.",
                                        &args.format
                                    ));
                                });
                            }
                        }
                    }
                    Err(_) => {
                        progress_bar.suspend(|| {
                            logger.error(&format!(
                                "graphviz failed to generate {} file.",
                                &args.format
                            ));
                        });
                    }
                }
            }
            Err(_) => {
                progress_bar.suspend(|| {
                    logger.error("graphviz doesn't appear to be installed. please install graphviz to generate images.");
                });
            }
        }
    }

    progress_bar.finish_and_clear();
}
