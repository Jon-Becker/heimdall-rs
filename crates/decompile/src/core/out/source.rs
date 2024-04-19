use std::{collections::HashMap, time::Instant};

use alloy_json_abi::StateMutability;

use eyre::Result;
use heimdall_common::ether::signatures::{ResolvedError, ResolvedLog};

use tracing::debug;

use crate::{
    core::analyze::AnalyzerType,
    interfaces::AnalyzedFunction,
    utils::constants::{DECOMPILED_SOURCE_HEADER_SOL, DECOMPILED_SOURCE_HEADER_YUL},
};

pub fn build_source(
    functions: &[AnalyzedFunction],
    _all_resolved_errors: &HashMap<String, ResolvedError>,
    _all_resolved_logs: &HashMap<String, ResolvedLog>,
) -> Result<Option<String>> {
    // we can get the AnalyzerType from the first function, since they are all the same
    let analyzer_type =
        functions.first().map(|f| f.analyzer_type.clone()).unwrap_or(AnalyzerType::Yul);
    if analyzer_type == AnalyzerType::Abi {
        debug!("skipping source construction for due to {} analyzer type", analyzer_type);
        return Ok(None);
    }

    debug!("constructing {} source representation", analyzer_type);
    let mut source = Vec::new();
    let start_time = Instant::now();

    // write the header to the output file
    source.extend(get_source_header(&analyzer_type));

    // add functions
    functions.iter().for_each(|f| {
        // get the function header
        source.extend(get_function_header(f));

        // add the function body
        source.extend(f.logic.clone());

        // close the function
        source.push("}".to_string());
    });

    debug!("constructing {} source took {:?}", analyzer_type, start_time.elapsed());

    // add missing closing brackets
    let imbalance = get_indentation_imbalance(&source);
    source.extend(vec!["}".to_string(); imbalance as usize]);

    // indent the source
    indent_source(&mut source);

    Ok(Some(source.join("\n")))
}

/// Helper function which returns the header for the decompiled source code.
fn get_source_header(analyzer_type: &AnalyzerType) -> Vec<String> {
    match analyzer_type {
        AnalyzerType::Solidity => DECOMPILED_SOURCE_HEADER_SOL
            .replace("{}", env!("CARGO_PKG_VERSION"))
            .split('\n')
            .map(|x| x.to_string())
            .collect(),
        AnalyzerType::Yul => DECOMPILED_SOURCE_HEADER_YUL
            .replace("{}", env!("CARGO_PKG_VERSION"))
            .split('\n')
            .map(|x| x.to_string())
            .collect(),
        _ => vec![],
    }
}

/// Helper function which will get the function header/signature for a given [`AnalyzedFunction`].
fn get_function_header(f: &AnalyzedFunction) -> Vec<String> {
    // get the state mutability of the function
    let state_mutability = match f.payable {
        true => StateMutability::Payable,
        false => match f.pure {
            true => StateMutability::Pure,
            false => match f.view {
                true => StateMutability::View,
                false => StateMutability::NonPayable,
            },
        },
    };

    // build function modifiers
    let mut function_modifiers = vec!["public".to_string()];
    if let Some(state_mutability) = state_mutability.as_str() {
        function_modifiers.push(state_mutability.to_owned());
    }
    if let Some(returns) = f.returns.clone() {
        function_modifiers.push(format!("returns ({})", returns));
    }

    // determine the name of the function
    let function_name = match f.resolved_function {
        Some(ref sig) => sig.name.clone(),
        None => format!("Unresolved_{}", f.selector),
    };

    let function_signature = format!(
        "{}({}) {}",
        function_name,
        f.arguments
            .iter()
            .enumerate()
            .map(|(i, (_, arg))| {
                format!(
                    "{} arg{i}",
                    match f.resolved_function {
                        Some(ref sig) => sig.inputs[i].clone(),
                        None => arg
                            .potential_types()
                            .first()
                            .unwrap_or(&"bytes32".to_string())
                            .to_string(),
                    }
                )
            })
            .collect::<Vec<String>>()
            .join(", "),
        function_modifiers.join(" ")
    );

    match f.analyzer_type {
        AnalyzerType::Solidity => {
            let mut output = vec![
                String::new(),
                format!("/// @custom:selector    0x{}", f.selector),
                format!("/// @custom:signature   {function_signature}"),
            ];
            output
                .extend(f.notices.iter().map(|notice| format!("/// @notice             {notice}")));
            output.extend(f.arguments.iter().map(|(i, arg)| {
                format!("/// @param              arg{i} {:?}", arg.potential_types(),)
            }));
            output.push(format!("function {function_signature} {{"));

            output
        }
        AnalyzerType::Yul => {
            let mut output = vec![
                String::new(),
                format!("/*"),
                format!(" * @custom:signature    {function_signature}"),
            ];
            output
                .extend(f.notices.iter().map(|notice| format!(" * @notice             {notice}")));
            output.extend(f.arguments.iter().map(|(i, arg)| {
                format!(" * @param                arg{i} {:?}", arg.potential_types(),)
            }));
            output.extend(vec![" */".to_string(), format!("case 0x{} {{", f.selector)]);

            output
        }
        _ => vec![],
    }
}

/// Helper function which will indent the source code.
fn indent_source(source: &mut Vec<String>) {
    let mut indentation_level = 0;
    for line in source.iter_mut() {
        if line.trim().starts_with('}') {
            indentation_level -= 1;
        }

        let mut new_line = String::new();
        for _ in 0..indentation_level {
            new_line.push_str("    ");
        }
        new_line.push_str(line);
        *line = new_line;

        if line.trim().ends_with('{') {
            indentation_level += 1;
        }
    }
}

/// Helper function which returns the imbalance of the source code's indentation. For example, if we
/// are missing 3 closing brackets, this function will return 3.
fn get_indentation_imbalance(source: &Vec<String>) -> i32 {
    let mut indentation_level = 0;
    for line in source.iter() {
        if line.trim().starts_with('}') {
            indentation_level -= 1;
        } else if line.trim().ends_with('{') {
            indentation_level += 1;
        }
    }

    indentation_level
}
