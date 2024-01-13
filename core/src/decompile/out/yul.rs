use std::{collections::HashMap, time::Duration};

use crate::{
    decompile::{constants::DECOMPILED_SOURCE_HEADER_YUL, util::Function, DecompilerArgs},
    error::Error,
};
use heimdall_common::{
    ether::signatures::ResolvedLog,
    utils::io::{
        file::short_path,
        logging::{Logger, TraceFactory},
    },
};
use indicatif::ProgressBar;

use super::postprocessers::yul::postprocess;

/// Build the decompiled Yul source code from the given functions. Will piece together decompiled
/// [`Function`]s and [`ResolvedLog`]s into a Yul contract.
pub fn build_yul_output(
    args: &DecompilerArgs,
    functions: Vec<Function>,
    all_resolved_events: HashMap<String, ResolvedLog>,
    trace: &mut TraceFactory,
    trace_parent: u32,
) -> Result<String, Error> {
    // get a new logger
    let logger = Logger::default();

    // get a new progress bar
    let progress_bar = ProgressBar::new_spinner();
    progress_bar.enable_steady_tick(Duration::from_millis(100));
    progress_bar.set_style(logger.info_spinner());

    // build the decompiled source
    let mut decompiled_output: Vec<String> = Vec::new();

    // truncate target for prettier display
    let mut shortened_target = args.target.clone();
    if shortened_target.len() > 66 {
        shortened_target = shortened_target.chars().take(66).collect::<String>() +
            "..." +
            &shortened_target.chars().skip(shortened_target.len() - 16).collect::<String>();
    }

    // add the call to the trace
    trace.add_call(
        trace_parent,
        line!(),
        "heimdall".to_string(),
        "build_yul_output".to_string(),
        vec![args.target.to_string()],
        short_path(&shortened_target),
    );

    // add the header to the output
    decompiled_output.extend(
        DECOMPILED_SOURCE_HEADER_YUL
            .replace("{}", env!("CARGO_PKG_VERSION"))
            .split('\n')
            .map(|x| x.to_string()),
    );

    // build contract logic
    for function in functions {
        progress_bar.set_message(format!("building logic for '0x{}'", function.selector));

        // build the function's header and parameters
        let function_header = match function.resolved_function {
            Some(resolved_function) => {
                format!(
                    "{}({})",
                    resolved_function.name,
                    resolved_function
                        .inputs
                        .iter()
                        .map(|solidity_type| {
                            format!(
                                "{}{}",
                                solidity_type,
                                if solidity_type.contains("[]") ||
                                    solidity_type.contains('(') ||
                                    ["string", "bytes"].contains(&solidity_type.as_str())
                                {
                                    " memory"
                                } else {
                                    ""
                                }
                            )
                        })
                        .collect::<Vec<String>>()
                        .join(", "),
                )
            }
            None => {
                // sort arguments by their calldata index
                let mut sorted_arguments: Vec<_> = function.arguments.clone().into_iter().collect();
                sorted_arguments.sort_by(|x, y| x.0.cmp(&y.0));

                format!(
                    "Unresolved_{}({})",
                    function.selector,
                    sorted_arguments
                        .iter()
                        .map(|(_, (_, potential_types))| {
                            format!(
                                "{}{}",
                                potential_types[0],
                                if potential_types[0].contains("[]") ||
                                    potential_types[0].contains('(') ||
                                    ["string", "bytes"].contains(&potential_types[0].as_str())
                                {
                                    " memory"
                                } else {
                                    ""
                                },
                            )
                        })
                        .collect::<Vec<String>>()
                        .join(", ")
                )
            }
        };

        // sort arguments by their calldata index
        let mut sorted_arguments: Vec<_> = function.arguments.into_iter().collect();
        sorted_arguments.sort_by(|x, y| x.0.cmp(&y.0));

        decompiled_output
            .push(format!("case 0x{} /* \"{}\" */ {{", function.selector, function_header));
        decompiled_output.extend(function.logic);
        decompiled_output.push(String::from("}"));
    }

    // closing brackets
    decompiled_output.append(&mut vec![
        "default { revert(0, 0) }".to_string(),
        "}".to_string(),
        "}".to_string(),
        "}".to_string(),
    ]);

    progress_bar.finish_and_clear();
    Ok(postprocess(decompiled_output, all_resolved_events, &progress_bar).join("\n"))
}
