use super::util::Function;
use heimdall_common::ether::signatures::ResolvedFunction;
use tracing::trace;

/// Given a list of potential [`ResolvedFunction`]s and a [`Function`], return a list of
/// [`ResolvedFunction`]s (that is, resolved signatures that were found on a 4byte directory) that
/// match the parameters found during symbolic execution for said [`Function`].
// TODO: revisit this logic, it's not very efficient
pub fn match_parameters(
    resolved_functions: Vec<ResolvedFunction>,
    function: &Function,
) -> Vec<ResolvedFunction> {
    let mut matched_functions: Vec<ResolvedFunction> = Vec::new();
    for mut resolved_function in resolved_functions {
        trace!(
            "checking function {}({}) against Unresolved_0x{}({})",
            &resolved_function.name,
            &resolved_function.inputs.join(","),
            &function.selector,
            &function
                .arguments
                .values()
                .map(|(_, potential_types)| potential_types
                    .first()
                    .expect("impossible case: argument has no potential types")
                    .clone())
                .collect::<Vec<String>>()
                .join(",")
        );
        // skip checking if length of parameters list is less than the resolved functions inputs
        resolved_function.inputs.retain(|x| !x.is_empty());
        let mut matched = true;

        // check each parameter type against a list of potential types
        for (index, input) in resolved_function.inputs.iter().enumerate() {
            trace!("    checking for parameter {} with type {}", &index.to_string(), &input);
            match function.arguments.get(&index) {
                Some((_, potential_types)) => {
                    // arrays are typically recorded as bytes by the decompiler's potential
                    // types
                    if input.contains("[]") {
                        if !potential_types.contains(&"bytes".to_string()) {
                            trace!(
                                "        parameter {} does not match type {} for function {}({})",
                                &index.to_string(),
                                &input,
                                &resolved_function.name,
                                &resolved_function.inputs.join(",")
                            );
                            continue;
                        }
                    } else if !potential_types.contains(input) {
                        matched = false;
                        trace!(
                            "        parameter {} does not match type {} for function {}({})",
                            &index.to_string(),
                            &input,
                            &resolved_function.name,
                            &resolved_function.inputs.join(",")
                        );
                        break;
                    }
                }
                None => {
                    // parameter not found
                    matched = false;
                    trace!(
                        "        parameter {} not found for function {}({})",
                        &index.to_string(),
                        &resolved_function.name,
                        &resolved_function.inputs.join(",")
                    );
                    break;
                }
            }
        }

        trace!("    matched: {}", &matched.to_string());
        if matched {
            matched_functions.push(resolved_function);
        }
    }

    matched_functions
}
