use crate::{
    core::types::SolidityType,
    interfaces::{AnalyzedFunction, CalldataFrame},
};
use alloy_dyn_abi::DynSolType;
use heimdall_common::ether::signatures::ResolvedFunction;
use tracing::trace;

/// Dynamic array bounds establish a word-sized element head, not its semantic type.
fn has_single_word_head(ty: &DynSolType) -> bool {
    if ty.is_dynamic() {
        return true;
    }
    match ty {
        DynSolType::FixedArray(element, 1) => has_single_word_head(element),
        DynSolType::Tuple(elements) if elements.len() == 1 => has_single_word_head(&elements[0]),
        DynSolType::FixedArray(..) | DynSolType::Tuple(..) => false,
        _ => true,
    }
}

fn parameter_matches(frame: &CalldataFrame, input: &str) -> bool {
    if matches!(frame.dynamic_type, Some(SolidityType::Array { length: None, .. })) {
        return matches!(
            DynSolType::parse(input),
            Ok(DynSolType::Array(element)) if has_single_word_head(&element)
        );
    }
    let input_type = SolidityType::parse(input);
    let mut candidate = &input_type;
    if frame.dynamic_type.is_none() {
        // A singleton static array occupies the same head word as its scalar element.
        while let SolidityType::Array { element, length: Some(1) } = candidate {
            candidate = element;
        }
    }
    frame.potential_types().contains(candidate)
}

/// Given a list of potential [`ResolvedFunction`]s and a [`Snapshot`], return a list of
/// [`ResolvedFunction`]s (that is, resolved signatures that were found on a 4byte directory) that
/// match the parameters found during symbolic execution for said [`Snapshot`].
pub(crate) fn match_parameters(
    resolved_functions: Vec<ResolvedFunction>,
    function: &AnalyzedFunction,
) -> Vec<ResolvedFunction> {
    let mut matched_functions: Vec<ResolvedFunction> = Vec::new();
    for mut resolved_function in resolved_functions {
        trace!(
            "checking function {} against Unresolved_0x{}({})",
            &resolved_function.signature,
            &function.selector,
            &function
                .arguments
                .values()
                .map(|f| f
                    .potential_types()
                    .first()
                    .cloned()
                    .unwrap_or(SolidityType::FixedBytes(32))
                    .to_string())
                .collect::<Vec<String>>()
                .join(",")
        );
        // skip checking if length of parameters list is less than the resolved functions inputs
        resolved_function.inputs.retain(|x| !x.is_empty());
        if resolved_function.inputs.len() < function.arguments.len() {
            continue;
        }
        let mut matched = true;

        // check each parameter type against a list of potential types
        for (index, input) in resolved_function.inputs.iter().enumerate() {
            trace!("    checking for parameter {} with type {}", &index.to_string(), &input);
            match function.arguments.get(&index) {
                Some(f) => {
                    if !parameter_matches(f, input) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use hashbrown::HashSet;

    fn matching_types(dynamic_type: Option<SolidityType>, candidates: &[&str]) -> Vec<String> {
        let mut function = AnalyzedFunction::new("12345678", false);
        function.arguments.insert(
            0,
            CalldataFrame {
                arg_op: "4".to_string(),
                mask_size: 32,
                heuristics: HashSet::new(),
                dynamic_type,
            },
        );
        let candidates = candidates
            .iter()
            .map(|input| ResolvedFunction {
                name: "candidate".to_string(),
                signature: format!("candidate({input})"),
                inputs: vec![input.to_string()],
                decoded_inputs: None,
            })
            .collect();
        match_parameters(candidates, &function)
            .into_iter()
            .map(|function| function.inputs[0].clone())
            .collect()
    }

    #[test]
    fn scalar_arguments_do_not_match_array_signatures() {
        assert_eq!(
            matching_types(None, &["uint256[]", "uint256[2]", "bytes", "uint256"]),
            vec!["uint256"]
        );
    }

    #[test]
    fn singleton_static_arrays_match_their_scalar_element() {
        for (mask_size, element) in [(32, "uint256"), (20, "address")] {
            let frame = CalldataFrame {
                arg_op: "4".to_string(),
                mask_size,
                heuristics: HashSet::new(),
                dynamic_type: None,
            };
            assert!(parameter_matches(&frame, &format!("{element}[1]")));
            assert!(parameter_matches(&frame, &format!("{element}[1][1]")));
            assert!(!parameter_matches(&frame, &format!("{element}[2]")));
            assert!(!parameter_matches(&frame, "bytes[1]"));
            assert!(!parameter_matches(&frame, "uint256[][1]"));
        }
        assert!(!parameter_matches(
            &CalldataFrame {
                arg_op: "4".to_string(),
                mask_size: 32,
                heuristics: HashSet::new(),
                dynamic_type: None,
            },
            "address[1]",
        ));
    }

    #[test]
    fn byte_lengths_match_bytes_or_string_but_not_word_arrays() {
        assert_eq!(
            matching_types(
                Some(SolidityType::Bytes),
                &["bytes", "string", "uint256[]", "bytes32", "uint256"],
            ),
            vec!["bytes", "string"]
        );
    }

    #[test]
    fn word_array_lengths_preserve_unknown_element_types() {
        assert_eq!(
            matching_types(
                Some(SolidityType::Array {
                    element: Box::new(SolidityType::Uint(256)),
                    length: None,
                }),
                &[
                    "uint256[]",
                    "address[]",
                    "uint256[][]",
                    "string[]",
                    "uint256[1][]",
                    "uint256[2][]",
                    "uint256[2]",
                    "string[2]",
                    "bytes",
                    "string",
                    "uint256",
                ],
            ),
            vec!["uint256[]", "address[]", "uint256[][]", "string[]", "uint256[1][]"]
        );
    }
}
