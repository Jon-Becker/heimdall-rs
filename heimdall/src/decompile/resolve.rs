use std::{collections::HashMap, time::Duration};

use heimdall_common::{
    ether::signatures::{resolve_signature, ResolvedFunction},
    io::logging::Logger,
};
use indicatif::ProgressBar;

use super::util::Function;

// resolve a list of function selectors to their possible signatures
pub fn resolve_function_selectors(
    selectors: Vec<String>,
    logger: &Logger,
) -> HashMap<String, Vec<ResolvedFunction>> {
    let mut resolved_functions: HashMap<String, Vec<ResolvedFunction>> = HashMap::new();

    let resolve_progress = ProgressBar::new_spinner();
    resolve_progress.enable_steady_tick(Duration::from_millis(100));
    resolve_progress.set_style(logger.info_spinner());

    for selector in selectors {
        resolve_progress.set_message(format!("resolving '0x{}'", selector));
        match resolve_signature(&selector) {
            Some(function) => {
                resolved_functions.insert(selector, function);
            }
            None => continue,
        }
    }
    resolve_progress.finish_and_clear();

    resolved_functions
}

// match the ResolvedFunction to a list of Function parameters
pub fn match_parameters(
    resolved_functions: Vec<ResolvedFunction>,
    function: &Function,
) -> Vec<ResolvedFunction> {

    let mut matched_functions: Vec<ResolvedFunction> = Vec::new();

    for mut resolved_function in resolved_functions {

        // skip checking if length of parameters is different
        resolved_function.inputs.retain(|x| x != "");
        if resolved_function.inputs.len() == function.arguments.len() {
            let mut matched = true;

            // check each parameter type against a list of potential types
            for (index, input) in resolved_function.inputs.iter().enumerate() {
                match function.arguments.get(&index) {
                    Some((_, potential_types)) => {

                        // arrays are typically recorded as bytes by the decompiler's potential types
                        if input.contains("[]") {
                            if !potential_types.contains(&"bytes".to_string()) {
                                continue;
                            }
                        } 
                        else if !potential_types.contains(&input) {
                            matched = false;
                            break;
                        }
                    }
                    None => {

                        // parameter not found
                        matched = false;
                        break;
                    }
                }
            }
            if matched {
                matched_functions.push(resolved_function);
            }
        }
    }

    matched_functions
}
