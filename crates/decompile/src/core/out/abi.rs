use alloy_dyn_abi::DynSolType;
use hashbrown::HashMap;
use std::time::Instant;

use alloy_json_abi::{Error, Event, EventParam, Function, JsonAbi, Param, StateMutability};

use eyre::Result;
use heimdall_common::{
    ether::{
        signatures::{ResolvedError, ResolvedLog},
        types::{to_abi_string, to_components},
    },
    utils::{hex::ToLowerHex, strings::encode_hex_reduced},
};
use serde_json::{json, Value};

use tracing::debug;

use crate::{core::types::SolidityType, interfaces::AnalyzedFunction};

pub(crate) fn build_abi(
    functions: &[AnalyzedFunction],
    all_resolved_errors: &HashMap<String, ResolvedError>,
    all_resolved_logs: &HashMap<String, ResolvedLog>,
) -> Result<JsonAbi> {
    debug!("constructing abi");
    let start_time = Instant::now();
    let mut abi = JsonAbi::new();

    // add functions
    functions.iter().for_each(|f| {
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

        // determine the name of the function
        let name = f.emitted_name();

        let function = Function {
            name: name.clone(),
            inputs: f
                .sorted_arguments()
                .iter()
                .enumerate()
                .map(|(i, (_, arg))| Param {
                    name: format!("arg{i}"),
                    internal_type: None,
                    ty: match f.resolved_function {
                        Some(ref sig) => {
                            to_abi_string(sig.inputs().get(i).unwrap_or(&DynSolType::Bytes))
                        }
                        None => arg
                            .potential_types()
                            .first()
                            .cloned()
                            .unwrap_or(SolidityType::FixedBytes(32))
                            .to_string(),
                    },
                    components: match f.resolved_function {
                        Some(ref sig) => {
                            to_components(sig.inputs().get(i).unwrap_or(&DynSolType::Bytes))
                        }
                        None => vec![],
                    },
                })
                .collect(),
            outputs: f
                .returns
                .as_ref()
                .map(|r| {
                    vec![Param {
                        name: "".to_string(),
                        internal_type: None,
                        ty: r.without_location().to_string(),
                        components: vec![],
                    }]
                })
                .unwrap_or_default(),
            state_mutability,
        };

        // add functions errors
        f.errors.iter().for_each(|error_selector| {
            // determine the name of the error
            let (name, inputs) = match all_resolved_errors
                .get(&encode_hex_reduced(*error_selector).replacen("0x", "", 1))
            {
                Some(error) => (
                    error.name.clone(),
                    error
                        .inputs()
                        .iter()
                        .enumerate()
                        .map(|(i, input)| Param {
                            name: format!("arg{i}"),
                            internal_type: None,
                            ty: to_abi_string(input),
                            components: to_components(input),
                        })
                        .collect(),
                ),
                None => (format!("CustomError_{}", error_selector.to_lower_hex()), vec![]),
            };

            let error = Error { name, inputs };

            let overloads = abi.errors.entry(error.name.clone()).or_default();
            if !overloads.contains(&error) {
                overloads.push(error);
            }
        });

        // add functions events
        f.events.iter().for_each(|event_selector| {
            // determine the name of the event
            let (name, inputs) = match all_resolved_logs
                .get(&encode_hex_reduced(*event_selector).replacen("0x", "", 1))
            {
                Some(event) => (
                    event.name.clone(),
                    event
                        .inputs()
                        .iter()
                        .enumerate()
                        .map(|(i, input)| EventParam {
                            name: format!("arg{i}"),
                            internal_type: None,
                            ty: to_abi_string(input),
                            components: to_components(input),
                            indexed: false,
                        })
                        .collect(),
                ),
                None => (format!("Event_{}", event_selector.to_lower_hex()), vec![]),
            };

            let event = Event { name, inputs, anonymous: event_selector.is_zero() };

            let overloads = abi.events.entry(event.name.clone()).or_default();
            if !overloads.contains(&event) {
                overloads.push(event);
            }
        });

        if !f.fallback {
            let overloads = abi.functions.entry(name).or_default();
            if !overloads.contains(&function) {
                overloads.push(function);
            }
        }
    });

    debug!("constructing abi took {:?}", start_time.elapsed());

    Ok(abi)
}

pub(crate) fn build_abi_with_details(
    abi: &JsonAbi,
    functions: &[AnalyzedFunction],
) -> Result<Value> {
    debug!("adding function details to abi");
    let start_time = Instant::now();

    // Serialize the standard ABI to JSON
    let mut abi_array = serde_json::to_value(abi)?;

    let function_map: HashMap<String, &AnalyzedFunction> = functions
        .iter()
        .filter(|function| !function.fallback)
        .filter_map(|function| {
            let signature = match &function.resolved_function {
                Some(resolved) => Function::parse(&resolved.signature).ok()?.signature(),
                None => {
                    let types = function
                        .sorted_arguments()
                        .iter()
                        .map(|(_, argument)| {
                            argument
                                .potential_types()
                                .first()
                                .cloned()
                                .unwrap_or(SolidityType::FixedBytes(32))
                                .to_string()
                        })
                        .collect::<Vec<_>>();
                    format!("{}({})", function.emitted_name(), types.join(","))
                }
            };
            Some((signature, function))
        })
        .collect();

    if let Some(items) = abi_array.as_array_mut() {
        for item in items.iter_mut() {
            if item.get("type").and_then(Value::as_str) != Some("function") {
                continue;
            }
            let function: Function = serde_json::from_value(item.clone())?;
            let signature = function.signature();
            if let Some(analyzed) = function_map.get(&signature) {
                if let Some(obj) = item.as_object_mut() {
                    obj.insert("selector".to_string(), json!(format!("0x{}", analyzed.selector)));
                    obj.insert("signature".to_string(), json!(signature));
                }
            }
        }
    }

    debug!("adding function details took {:?}", start_time.elapsed());

    Ok(abi_array)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interfaces::CalldataFrame;
    use alloy::primitives::{keccak256, U256};
    use heimdall_common::ether::signatures::ResolvedFunction;

    fn analyzed(signature: &str) -> AnalyzedFunction {
        let parsed = Function::parse(signature).unwrap();
        let selector = parsed.selector().to_string().trim_start_matches("0x").to_string();
        let mut function = AnalyzedFunction::new(&selector, false);
        for index in 0..parsed.inputs.len() {
            function.arguments.insert(
                index,
                CalldataFrame {
                    arg_op: (4 + 32 * index).to_string(),
                    mask_size: 32,
                    heuristics: Default::default(),
                },
            );
        }
        function.resolved_function = Some(ResolvedFunction {
            name: parsed.name,
            signature: signature.to_string(),
            inputs: parsed.inputs.iter().map(|input| input.ty.clone()).collect(),
            decoded_inputs: None,
        });
        function
    }

    #[test]
    fn preserves_observed_overload_families_and_detailed_selectors() {
        let families = [
            [
                "setTargetFunctionRole(address,bytes4[],uint64)",
                "setTargetFunctionRole(address,bytes4,uint8)",
            ],
            ["getGnsPriceUsd(uint8)", "getGnsPriceUsd(uint8,uint256)"],
            ["isClassificationValid(address,bytes32)", "isClassificationValid(address)"],
            ["deactivatePoints()", "deactivatePoints(address)"],
            ["isPaused(bytes4)", "isPaused()"],
            ["pause(uint256)", "pause(bytes4,uint256)"],
            ["lpUnrealizedPnlUsd(address)", "lpUnrealizedPnlUsd()"],
        ];
        let mut functions: Vec<_> = families.iter().flatten().map(|sig| analyzed(sig)).collect();
        functions.push(functions[0].clone());
        for functions in [functions.clone(), functions.into_iter().rev().collect()] {
            let abi = build_abi(&functions, &HashMap::new(), &HashMap::new()).unwrap();
            assert_eq!(abi.functions().count(), 14);
            let detailed = build_abi_with_details(&abi, &functions).unwrap();
            for signature in families.iter().flatten() {
                let item = detailed
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|item| item["signature"] == *signature)
                    .unwrap();
                assert_eq!(
                    item["selector"],
                    Function::parse(signature).unwrap().selector().to_string()
                );
            }
        }
    }

    #[test]
    fn retains_observed_selectors_for_unresolved_functions() {
        let mut function = analyzed("approve(address,uint256)");
        function.resolved_function = None;
        let functions = [function];
        let abi = build_abi(&functions, &HashMap::new(), &HashMap::new()).unwrap();
        let details = build_abi_with_details(&abi, &functions).unwrap();
        assert_eq!(details[0]["selector"], "0x095ea7b3");
        assert!(details[0]["signature"].as_str().unwrap().starts_with("Unresolved_095ea7b3("));
    }

    #[test]
    fn preserves_event_and_error_overloads_including_fallback_observations() {
        let mut functions = vec![analyzed("example()"), AnalyzedFunction::new("00000000", true)];
        let mut errors = HashMap::new();
        let mut events = HashMap::new();
        for (index, ty) in ["address", "uint256"].iter().enumerate() {
            let error = Error::parse(&format!("Denied({ty})")).unwrap();
            let error_selector = U256::from_be_slice(error.selector().as_slice());
            errors.insert(
                encode_hex_reduced(error_selector).trim_start_matches("0x").to_string(),
                ResolvedError {
                    name: error.name,
                    signature: format!("Denied({ty})"),
                    inputs: vec![ty.to_string()],
                },
            );
            let signature = format!("Changed({ty})");
            let topic = U256::from_be_slice(keccak256(signature.as_bytes()).as_slice());
            events.insert(
                encode_hex_reduced(topic).trim_start_matches("0x").to_string(),
                ResolvedLog {
                    name: "Changed".to_string(),
                    signature,
                    inputs: vec![ty.to_string()],
                },
            );
            functions[index].errors.insert(error_selector);
            functions[index].events.insert(topic);
        }
        functions.push(functions[0].clone());
        let abi = build_abi(&functions, &errors, &events).unwrap();
        assert_eq!(abi.functions().count(), 1);
        assert_eq!(abi.errors["Denied"].len(), 2);
        assert_eq!(abi.events["Changed"].len(), 2);
    }
}
