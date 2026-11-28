use crate::{
    core::types::SolidityType,
    interfaces::{AnalyzedFunction, TypeHeuristic},
};
use alloy_json_abi::Function;
use heimdall_common::ether::signatures::ResolvedFunction;
use tracing::trace;

/// Given a list of potential [`ResolvedFunction`]s and a [`Snapshot`], return a list of
/// [`ResolvedFunction`]s (that is, resolved signatures that were found on a 4byte directory) that
/// match the parameters found during symbolic execution for said [`Snapshot`].
pub(crate) fn match_parameters(
    resolved_functions: Vec<ResolvedFunction>,
    function: &AnalyzedFunction,
) -> Vec<ResolvedFunction> {
    let mut matched_functions: Vec<ResolvedFunction> = Vec::new();
    for mut resolved_function in resolved_functions {
        if !Function::parse(&resolved_function.signature).is_ok_and(|candidate| {
            candidate.selector().to_string().trim_start_matches("0x") == function.selector
        }) {
            continue;
        }
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
                    let input_type = SolidityType::parse(input);
                    // Byte-counted payloads exclude the word encoding used by ABI arrays.
                    if let SolidityType::Array { element, length } = input_type.without_location() {
                        let compatible = !f.heuristics.contains(&TypeHeuristic::DynamicBytes) &&
                            match length {
                                None => f.heuristics.contains(&TypeHeuristic::DynamicLength),
                                Some(_) => f.potential_types().contains(element.as_ref()),
                            };
                        if !compatible {
                            matched = false;
                            break;
                        }
                    } else if !f.potential_types().contains(&input_type) {
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
    use crate::interfaces::CalldataFrame;

    fn candidate(signature: &str) -> ResolvedFunction {
        let function = Function::parse(signature).unwrap();
        ResolvedFunction {
            name: function.name,
            signature: signature.to_string(),
            inputs: function.inputs.iter().map(|input| input.ty.clone()).collect(),
            decoded_inputs: None,
        }
    }

    #[test]
    fn resolves_upgrade_signature_with_dynamic_bytes_evidence() {
        let mut function = AnalyzedFunction::new("4f1ef286", false);
        function.arguments.insert(
            0,
            CalldataFrame {
                arg_op: "4".to_string(),
                mask_size: 20,
                heuristics: Default::default(),
            },
        );
        let mut dynamic = CalldataFrame {
            arg_op: "36".to_string(),
            mask_size: 32,
            heuristics: Default::default(),
        };
        dynamic.heuristics.insert(TypeHeuristic::DynamicBytes);
        function.arguments.insert(1, dynamic);
        let matched = match_parameters(
            vec![
                candidate("upgradeToAndCall(address,bytes)"),
                candidate("upgradeToAndCall(address,uint256)"),
            ],
            &function,
        );
        assert_eq!(matched.len(), 1);
        assert_eq!(matched[0].signature, "upgradeToAndCall(address,bytes)");
    }

    #[test]
    fn rejects_array_candidates_without_dynamic_argument_evidence() {
        let signature = "example(uint256[])";
        let selector = Function::parse(signature).unwrap().selector().to_string();
        let mut function = AnalyzedFunction::new(selector.trim_start_matches("0x"), false);
        function.arguments.insert(
            0,
            CalldataFrame {
                arg_op: "4".to_string(),
                mask_size: 20,
                heuristics: Default::default(),
            },
        );
        assert!(match_parameters(vec![candidate(signature)], &function).is_empty());
    }

    #[test]
    fn retains_single_word_static_array_candidates() {
        let signature = "example(address[1])";
        let selector = Function::parse(signature).unwrap().selector().to_string();
        let mut function = AnalyzedFunction::new(selector.trim_start_matches("0x"), false);
        function.arguments.insert(
            0,
            CalldataFrame {
                arg_op: "4".to_string(),
                mask_size: 20,
                heuristics: Default::default(),
            },
        );
        assert_eq!(match_parameters(vec![candidate(signature)], &function).len(), 1);
    }

    #[test]
    fn retains_indistinguishable_signature_collisions() {
        let signatures =
            ["approve(address,uint256)", "__$$$$___$$_$_$$__$_$_$$__$$$$(address,uint256)"];
        let mut function = AnalyzedFunction::new("095ea7b3", false);
        for (index, mask_size) in [(0, 20), (1, 32)] {
            function.arguments.insert(
                index,
                CalldataFrame {
                    arg_op: (4 + index * 32).to_string(),
                    mask_size,
                    heuristics: Default::default(),
                },
            );
        }
        let matched = match_parameters(
            signatures.iter().map(|signature| candidate(signature)).collect(),
            &function,
        );
        assert_eq!(matched.len(), 2);
    }

    #[test]
    fn accepts_array_candidates_with_dynamic_length_evidence() {
        let signature = "example(uint256[])";
        let selector = Function::parse(signature).unwrap().selector().to_string();
        let mut function = AnalyzedFunction::new(selector.trim_start_matches("0x"), false);
        let mut argument = CalldataFrame {
            arg_op: "4".to_string(),
            mask_size: 32,
            heuristics: Default::default(),
        };
        argument.heuristics.insert(TypeHeuristic::DynamicLength);
        function.arguments.insert(0, argument);
        assert_eq!(match_parameters(vec![candidate(signature)], &function).len(), 1);
        function.arguments.get_mut(&0).unwrap().heuristics.insert(TypeHeuristic::DynamicBytes);
        assert!(match_parameters(vec![candidate(signature)], &function).is_empty());
    }

    #[test]
    fn rejects_a_signature_with_the_wrong_selector_even_when_types_fit() {
        let mut function = AnalyzedFunction::new("095ea7b3", false);
        for (index, mask_size) in [(0, 20), (1, 32)] {
            function.arguments.insert(
                index,
                CalldataFrame {
                    arg_op: (4 + index * 32).to_string(),
                    mask_size,
                    heuristics: Default::default(),
                },
            );
        }
        let matched = match_parameters(
            vec![
                candidate("approve(address,uint256)"),
                candidate("transfer(address,uint256)"),
                candidate("sign_szabo_bytecode(bytes16,uint128)"),
            ],
            &function,
        );
        assert_eq!(matched.len(), 1);
        assert_eq!(matched[0].name, "approve");
    }
}
