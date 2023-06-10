use super::util::Function;
use heimdall_common::ether::signatures::ResolvedFunction;

// match the ResolvedFunction to a list of Function parameters
pub fn match_parameters(
    resolved_functions: Vec<ResolvedFunction>,
    function: &Function,
) -> Vec<ResolvedFunction> {
    let mut matched_functions: Vec<ResolvedFunction> = Vec::new();

    for mut resolved_function in resolved_functions {
        // skip checking if length of parameters is different
        resolved_function.inputs.retain(|x| !x.is_empty());
        if resolved_function.inputs.len() == function.arguments.len() {
            let mut matched = true;

            // check each parameter type against a list of potential types
            for (index, input) in resolved_function.inputs.iter().enumerate() {
                match function.arguments.get(&index) {
                    Some((_, potential_types)) => {
                        // arrays are typically recorded as bytes by the decompiler's potential
                        // types
                        if input.contains("[]") {
                            if !potential_types.contains(&"bytes".to_string()) {
                                continue
                            }
                        } else if !potential_types.contains(input) {
                            matched = false;
                            break
                        }
                    }
                    None => {
                        // parameter not found
                        matched = false;
                        break
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
