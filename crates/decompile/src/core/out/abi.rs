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

use crate::interfaces::AnalyzedFunction;

pub(crate) fn build_abi(
    functions: &[AnalyzedFunction],
    all_resolved_errors: &HashMap<String, ResolvedError>,
    all_resolved_logs: &HashMap<String, ResolvedLog>,
) -> Result<JsonAbi> {
    debug!("constructing abi");
    let start_time = Instant::now();
    let mut abi = JsonAbi::new();

    // add functions
    functions.iter().filter(|f| !f.fallback).for_each(|f| {
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
        let name = match f.resolved_function {
            Some(ref sig) => sig.name.clone(),
            None => format!("Unresolved_{}", f.selector),
        };

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
                            .unwrap_or(&"bytes32".to_string())
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
                        ty: r.replacen("memory", "", 1).trim().to_string(),
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

            abi.errors.insert(error.name.clone(), vec![error]);
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

            abi.events.insert(event.name.clone(), vec![event]);
        });

        abi.functions.insert(name, vec![function]);
    });

    debug!("constructing abi took {:?}", start_time.elapsed());

    Ok(abi)
}

pub(crate) fn build_abi_with_details(
    functions: &[AnalyzedFunction],
    all_resolved_errors: &HashMap<String, ResolvedError>,
    all_resolved_logs: &HashMap<String, ResolvedLog>,
) -> Result<Vec<Value>> {
    debug!("constructing abi with function details");
    let start_time = Instant::now();

    let mut abi_items = Vec::new();

    // Process each function to create extended ABI items
    functions.iter().filter(|f| !f.fallback).for_each(|f| {
        // determine the state mutability of the function
        let state_mutability = match f.pure {
            true => "pure",
            false => match f.view {
                true => "view",
                false => match f.payable {
                    true => "payable",
                    false => "nonpayable",
                },
            },
        };

        // determine the name of the function
        let name = match f.resolved_function {
            Some(ref sig) => sig.name.clone(),
            None => format!("Unresolved_{}", f.selector),
        };

        // Get the signature
        let signature = match f.resolved_function {
            Some(ref sig) => sig.signature.clone(),
            None => {
                // Build signature from analyzed function
                let params: Vec<String> = f
                    .sorted_arguments()
                    .iter()
                    .enumerate()
                    .map(|(i, (_, arg))| match f.resolved_function {
                        Some(ref sig) => {
                            sig.inputs.get(i).unwrap_or(&"bytes32".to_string()).clone()
                        }
                        None => arg
                            .potential_types()
                            .first()
                            .unwrap_or(&"bytes32".to_string())
                            .to_string(),
                    })
                    .collect();
                format!("{}({})", name, params.join(","))
            }
        };

        let inputs: Vec<Value> = f
            .sorted_arguments()
            .iter()
            .enumerate()
            .map(|(i, (_, arg))| {
                json!({
                    "name": format!("arg{i}"),
                    "type": match f.resolved_function {
                        Some(ref sig) => {
                            to_abi_string(sig.inputs().get(i).unwrap_or(&DynSolType::Bytes))
                        }
                        None => arg
                            .potential_types()
                            .first()
                            .unwrap_or(&"bytes32".to_string())
                            .to_string(),
                    },
                    "internalType": match f.resolved_function {
                        Some(ref sig) => {
                            to_abi_string(sig.inputs().get(i).unwrap_or(&DynSolType::Bytes))
                        }
                        None => arg
                            .potential_types()
                            .first()
                            .unwrap_or(&"bytes32".to_string())
                            .to_string(),
                    }
                })
            })
            .collect();

        let outputs: Vec<Value> = f
            .returns
            .as_ref()
            .map(|r| {
                vec![json!({
                    "name": "",
                    "type": r.replacen("memory", "", 1).trim().to_string(),
                    "internalType": r.replacen("memory", "", 1).trim().to_string()
                })]
            })
            .unwrap_or_default();

        // Create extended function object with selector and signature
        let function_item = json!({
            "type": "function",
            "name": name,
            "inputs": inputs,
            "outputs": outputs,
            "stateMutability": state_mutability,
            "selector": format!("0x{}", f.selector),
            "signature": signature,
        });

        abi_items.push(function_item);

        // Add function errors
        f.errors.iter().for_each(|error_selector| {
            let (name, inputs) = match all_resolved_errors
                .get(&encode_hex_reduced(*error_selector).replacen("0x", "", 1))
            {
                Some(error) => {
                    let inputs: Vec<Value> = error
                        .inputs()
                        .iter()
                        .enumerate()
                        .map(|(i, input)| {
                            json!({
                                "name": format!("arg{i}"),
                                "type": to_abi_string(input),
                                "internalType": to_abi_string(input)
                            })
                        })
                        .collect();
                    (error.name.clone(), inputs)
                }
                None => (format!("CustomError_{}", error_selector.to_lower_hex()), vec![]),
            };

            let error_item = json!({
                "type": "error",
                "name": name,
                "inputs": inputs
            });

            abi_items.push(error_item);
        });

        // Add function events
        f.events.iter().for_each(|event_selector| {
            let (name, inputs) = match all_resolved_logs
                .get(&encode_hex_reduced(*event_selector).replacen("0x", "", 1))
            {
                Some(event) => {
                    let inputs: Vec<Value> = event
                        .inputs()
                        .iter()
                        .enumerate()
                        .map(|(i, input)| {
                            json!({
                                "name": format!("arg{i}"),
                                "type": to_abi_string(input),
                                "internalType": to_abi_string(input),
                                "indexed": false
                            })
                        })
                        .collect();
                    (event.name.clone(), inputs)
                }
                None => (format!("Event_{}", event_selector.to_lower_hex()), vec![]),
            };

            let event_item = json!({
                "type": "event",
                "name": name,
                "inputs": inputs,
                "anonymous": event_selector.is_zero()
            });

            abi_items.push(event_item);
        });
    });

    debug!("constructing abi with details took {:?}", start_time.elapsed());

    Ok(abi_items)
}
