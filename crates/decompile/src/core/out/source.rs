use hashbrown::{HashMap, HashSet};
use std::{str::FromStr, time::Instant};

use alloy::primitives::U256;
use alloy_json_abi::StateMutability;

use eyre::{OptionExt, Result};
use heimdall_common::{
    ether::signatures::{ResolvedError, ResolvedLog},
    resources::openrouter::{complete_chat_structured, AnnotatedContractResponse},
    utils::{hex::ToLowerHex, strings::encode_hex_reduced},
};

use tracing::debug;

use crate::{
    core::analyze::AnalyzerType,
    interfaces::AnalyzedFunction,
    utils::constants::{
        DECOMPILED_SOURCE_HEADER_SOL, DECOMPILED_SOURCE_HEADER_YUL, LLM_POSTPROCESSING_PROMPT,
    },
};

/// Annotates the entire contract source code using an LLM with structured output.
async fn annotate_contract(source: &str, openrouter_api_key: &str, model: &str) -> Result<String> {
    let response: AnnotatedContractResponse = complete_chat_structured(
        &LLM_POSTPROCESSING_PROMPT.replace("{source}", source),
        openrouter_api_key,
        model,
        "annotated_contract",
    )
    .await
    .ok_or_eyre("failed to llm postprocess contract")?;

    Ok(response.source)
}

#[derive(Clone, Debug)]
pub(crate) struct StorageVariable {
    pub typ: String,
    pub slot: Option<String>,
}

pub(crate) async fn build_source(
    functions: &[AnalyzedFunction],
    all_resolved_errors: &HashMap<String, ResolvedError>,
    all_resolved_logs: &HashMap<String, ResolvedLog>,
    storage_variables: &HashMap<String, StorageVariable>,
    llm_postprocess: bool,
    openrouter_api_key: String,
    model: String,
) -> Result<Option<String>> {
    // we can get the AnalyzerType from the first function, since they are all the same
    let analyzer_type = functions.first().map(|f| f.analyzer_type).unwrap_or(AnalyzerType::Yul);
    if analyzer_type == AnalyzerType::Abi {
        debug!("skipping source construction for due to {} analyzer type", analyzer_type);
        return Ok(None);
    }

    debug!("constructing {} source representation", analyzer_type);
    let mut source = Vec::new();
    let start_time = Instant::now();

    // write the header to the output file
    source.extend(get_source_header(&analyzer_type));

    // add storage variables
    if analyzer_type == AnalyzerType::Solidity {
        source.extend(get_constants(functions));
    }

    // add storage variables
    if analyzer_type == AnalyzerType::Solidity {
        source.extend(get_storage_variables(storage_variables, functions));
    }

    // add event and error declarations
    let resolved_event_error_map =
        get_event_and_error_declarations(functions, all_resolved_errors, all_resolved_logs);
    if analyzer_type == AnalyzerType::Solidity {
        resolved_event_error_map.iter().for_each(|(_, (resolved_name, typ))| {
            source.push(format!("{typ} {resolved_name}"));
        });

        // add the fallback function, if it exists
        if let Some(fallback) = functions.iter().find(|f| f.fallback) {
            source.push(String::from("fallback() external payable {"));
            source.extend(fallback.logic.clone());
            source.extend(vec![String::from("}"), String::from("")]);
        }
    }

    // add functions
    for f in functions.iter().filter(|f| {
        !f.fallback &&
            (analyzer_type == AnalyzerType::Yul ||
                (f.maybe_getter_for.is_none() && !f.is_constant()))
    }) {
        let mut function_source = Vec::new();

        // get the function header
        function_source.extend(get_function_header(f));
        function_source.extend(f.logic.clone());
        function_source.push("}".to_string());

        let imbalance = get_indentation_imbalance(&function_source);
        function_source.extend(vec!["}".to_string(); imbalance as usize]);

        source.extend(function_source);
    }

    if analyzer_type == AnalyzerType::Yul {
        // add the fallback function, if it exists
        if let Some(fallback) = functions.iter().find(|f| f.fallback) {
            source.push("default {".to_string());
            source.extend(fallback.logic.clone());
            source.push("}".to_string());
        } else {
            source.push("default { revert(0, 0) }".to_string());
        }
    }

    // add missing closing brackets
    let imbalance = get_indentation_imbalance(&source);
    source.extend(vec!["}".to_string(); imbalance as usize]);

    // indent and combine source
    indent_source(&mut source);
    let mut source = source.join("\n");

    // replace all custom event and error declarations with their resolved names
    resolved_event_error_map.iter().for_each(|(unresolved_name, (resolved_name, _))| {
        // get only the name of both (remove `(..)`)
        let unresolved_name = unresolved_name.split('(').next().expect("unresolved name is empty");
        let resolved_name = resolved_name.split('(').next().expect("resolved name is empty");
        source = source.replace(unresolved_name, resolved_name);
    });

    // replace all storage variables w/ getters w/ their resolved names
    functions.iter().filter(|f| f.maybe_getter_for.is_some()).for_each(|f| {
        let getter_for_storage_variable = f.maybe_getter_for.as_ref().expect("impossible");
        let resolved_name = f
            .resolved_function
            .as_ref()
            .map(|x| x.name.clone())
            .unwrap_or_else(|| format!("unresolved_{}", f.selector));
        source = source.replace(getter_for_storage_variable, &resolved_name);
    });

    // apply LLM postprocessing to the entire contract source
    if llm_postprocess {
        let postprocess_start = Instant::now();
        debug!("llm postprocessing entire contract source");

        match annotate_contract(&source, &openrouter_api_key, &model).await {
            Ok(annotated_source) => {
                debug!("llm postprocessing contract took {:?}", postprocess_start.elapsed());
                source = annotated_source;
            }
            Err(e) => {
                debug!("llm postprocessing contract failed: {:?}", e);
            }
        }
    }

    debug!("constructing {} source took {:?}", analyzer_type, start_time.elapsed());

    Ok(Some(source))
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
    // determine the state mutability of the function
    let state_mutability = match f.pure {
        true => StateMutability::Pure,
        false => match f.view {
            true => StateMutability::View,
            false => match f.payable {
                true => StateMutability::Payable,
                false => StateMutability::NonPayable,
            },
        },
    };

    // build function modifiers
    let mut function_modifiers = vec!["public".to_string()];
    if let Some(state_mutability) = state_mutability.as_str() {
        function_modifiers.push(state_mutability.to_owned());
    }
    if let Some(returns) = f.returns.as_ref() {
        function_modifiers.push(format!("returns ({returns})"));
    }

    // determine the name of the function
    let function_name = match f.resolved_function {
        Some(ref sig) => sig.name.clone(),
        None => format!("Unresolved_{}", f.selector),
    };

    let function_signature = match f.resolved_function {
        Some(ref sig) => format!(
            "{}({}) {}",
            function_name,
            sig.inputs()
                .iter()
                .enumerate()
                .map(|(i, arg)| { format!("{} arg{i}", arg.to_string()) })
                .collect::<Vec<String>>()
                .join(", "),
            function_modifiers.join(" ")
        ),
        None => format!(
            "{}({}) {}",
            function_name,
            f.sorted_arguments()
                .iter()
                .enumerate()
                .map(|(i, (_, arg))| {
                    format!(
                        "{} arg{i}",
                        arg.potential_types()
                            .first()
                            .cloned()
                            .unwrap_or_else(|| "bytes32".to_string())
                    )
                })
                .collect::<Vec<String>>()
                .join(", "),
            function_modifiers.join(" ")
        ),
    };

    match f.analyzer_type {
        AnalyzerType::Solidity => {
            let mut output = vec![
                String::new(),
                format!("/// @custom:selector    0x{}", f.selector),
                format!("/// @custom:signature   {function_signature}"),
            ];
            output
                .extend(f.notices.iter().map(|notice| format!("/// @notice             {notice}")));
            output.extend(f.sorted_arguments().iter().map(|(i, arg)| {
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
            output.extend(f.sorted_arguments().iter().map(|(i, arg)| {
                format!(" * @param                arg{i} {:?}", arg.potential_types(),)
            }));
            output.extend(vec![" */".to_string(), format!("case 0x{} {{", f.selector)]);

            output
        }
        _ => vec![],
    }
}

/// Helper function which will write constant variables to the source code.
fn get_constants(functions: &[AnalyzedFunction]) -> Vec<String> {
    let mut output: Vec<String> = functions
        .iter()
        .filter_map(|f| {
            if f.is_constant() {
                Some(format!(
                    "{} public constant {} = {};",
                    f.returns.as_deref().unwrap_or("bytes").replacen("memory", "", 1).trim(),
                    f.resolved_function
                        .as_ref()
                        .map(|x| x.name.clone())
                        .unwrap_or_else(|| format!("unresolved_{}", f.selector)),
                    f.constant_value.as_deref().unwrap_or("0x")
                ))
            } else {
                None
            }
        })
        .collect();
    if !output.is_empty() {
        output.push("".to_string());
    }
    output
}

/// Helper function which will write the storage variable declarations for the decompiled source
/// code.
fn get_storage_variables(
    storage_variables: &HashMap<String, StorageVariable>,
    functions: &[AnalyzedFunction],
) -> Vec<String> {
    let mut declarations = storage_variables
        .iter()
        .map(|(name, variable)| {
            let typ = &variable.typ;
            let slot = variable.slot.as_deref().unwrap_or("unknown");
            if let Some(f) = functions.iter().find(|f| f.maybe_getter_for.as_ref() == Some(name)) {
                let name = f
                    .resolved_function
                    .as_ref()
                    .map(|x| x.name.clone())
                    .unwrap_or_else(|| format!("unresolved_{}", f.selector));

                // TODO: for public getters, we can use `eth_getStorageAt` to get the value
                return (
                    typ.starts_with("mapping("),
                    U256::from_str(slot).ok(),
                    format!("{typ} public {name}; // storage slot: {slot}"),
                );
            }

            (
                typ.starts_with("mapping("),
                U256::from_str(slot).ok(),
                format!("{typ} {name}; // storage slot: {slot}"),
            )
        })
        .collect::<Vec<_>>();
    declarations.sort_by(|a, b| {
        a.0.cmp(&b.0)
            .then_with(|| a.1.unwrap_or(U256::MAX).cmp(&b.1.unwrap_or(U256::MAX)))
            .then_with(|| a.2.cmp(&b.2))
    });

    let first_mapping = declarations.iter().position(|(mapping, ..)| *mapping);
    let mut output = Vec::new();
    for (index, (_, _, declaration)) in declarations.into_iter().enumerate() {
        if first_mapping == Some(index) && index > 0 {
            output.push(String::new());
        }
        output.push(declaration);
    }
    if !output.is_empty() {
        output.push(String::new());
    }
    output
}

/// Helper function which will get the event and error declarations for the decompiled source code.
fn get_event_and_error_declarations(
    functions: &[AnalyzedFunction],
    all_resolved_errors: &HashMap<String, ResolvedError>,
    all_resolved_logs: &HashMap<String, ResolvedLog>,
) -> HashMap<String, (String, String)> {
    let mut output = HashMap::new();

    // get all events and errors
    let all_events = functions.iter().flat_map(|f| f.events.clone()).collect::<HashSet<_>>();
    let all_errors = functions.iter().flat_map(|f| f.errors.clone()).collect::<HashSet<_>>();

    // add event declarations
    all_events.iter().for_each(|event_selector| {
        // determine the name of the event
        let (name, inputs) = match all_resolved_logs
            .get(&encode_hex_reduced(*event_selector).replacen("0x", "", 1))
        {
            Some(event) => {
                (event.name.clone(), event.inputs().iter().map(|i| i.to_string()).collect())
            }
            None => (
                format!(
                    "Event_{}",
                    event_selector
                        .to_lower_hex()
                        .replacen("0x", "", 1)
                        .get(0..8)
                        .unwrap_or("00000000")
                ),
                vec![],
            ),
        };

        let unresolved_name = format!(
            "Event_{}",
            event_selector.to_lower_hex().replacen("0x", "", 1).get(0..8).unwrap_or("00000000")
        );
        output.insert(
            unresolved_name,
            (format!("{name}({});", inputs.join(", ")), "event".to_string()),
        );
    });

    // add error declarations
    all_errors.iter().for_each(|error_selector| {
        // determine the name of the error
        let (name, inputs) = match all_resolved_errors
            .get(&encode_hex_reduced(*error_selector).replacen("0x", "", 1))
        {
            Some(error) => {
                (error.name.clone(), error.inputs().iter().map(|i| i.to_string()).collect())
            }
            None => (
                format!(
                    "CustomError_{}",
                    error_selector
                        .to_lower_hex()
                        .replacen("0x", "", 1)
                        .get(0..8)
                        .unwrap_or("00000000")
                ),
                vec![],
            ),
        };

        let unresolved_name = format!(
            "CustomError_{}",
            error_selector.to_lower_hex().replacen("0x", "", 1).get(0..8).unwrap_or("00000000")
        );
        output.insert(
            unresolved_name,
            (format!("{name}({});", inputs.join(", ")), "error".to_string()),
        );
    });

    output
}

/// Helper function which will indent the source code.
fn indent_source(source: &mut [String]) {
    let mut indentation_level = 0;
    for line in source.iter_mut() {
        if line.trim().starts_with('}') {
            indentation_level -= 1;
        }

        let mut new_line = String::new();
        for _ in 0..indentation_level {
            new_line.push_str("    ");
        }
        new_line.push_str(line.trim_start());
        *line = new_line;

        if line.trim().ends_with('{') {
            indentation_level += 1;
        }
    }
}

/// Helper function which returns the imbalance of the source code's indentation. For example, if we
/// are missing 3 closing brackets, this function will return 3.
fn get_indentation_imbalance(source: &[String]) -> i32 {
    let mut indentation_level = 0;
    for line in source.iter() {
        if line.trim().starts_with('}') {
            indentation_level -= 1;
        }
        if line.trim().ends_with('{') {
            indentation_level += 1;
        }
    }

    indentation_level
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn storage_declarations_include_base_slots() {
        let variables = HashMap::from([
            (
                "storage_map_a".to_string(),
                StorageVariable {
                    typ: "mapping(address => uint256)".to_string(),
                    slot: Some("0x03".to_string()),
                },
            ),
            (
                "store_b".to_string(),
                StorageVariable { typ: "string".to_string(), slot: Some("0x00".to_string()) },
            ),
        ]);
        let output = get_storage_variables(&variables, &[]);
        assert_eq!(
            output,
            vec![
                "string store_b; // storage slot: 0x00",
                "",
                "mapping(address => uint256) storage_map_a; // storage slot: 0x03",
                "",
            ]
        );
    }
}
