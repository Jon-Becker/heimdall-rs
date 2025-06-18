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
                            .cloned()
                            .unwrap_or_else(|| "bytes32".to_string()),
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
    abi: &JsonAbi,
    functions: &[AnalyzedFunction],
) -> Result<Value> {
    debug!("adding function details to abi");
    let start_time = Instant::now();

    // Serialize the standard ABI to JSON
    let mut abi_array = serde_json::to_value(abi)?;

    // Create a map of function selectors for quick lookup
    let function_map: HashMap<String, &AnalyzedFunction> = functions
        .iter()
        .filter(|f| !f.fallback)
        .map(|f| {
            let name = match f.resolved_function {
                Some(ref sig) => sig.name.clone(),
                None => format!("Unresolved_{}", f.selector),
            };
            (name, f)
        })
        .collect();

    // Add selector and signature to each function in the ABI
    if let Some(items) = abi_array.as_array_mut() {
        for item in items.iter_mut() {
            if let Some(obj) = item.as_object_mut() {
                if obj.get("type").and_then(|t| t.as_str()) == Some("function") {
                    let name = obj.get("name").and_then(|n| n.as_str()).map(|s| s.to_string());
                    if let Some(name_str) = name {
                        if let Some(analyzed_func) = function_map.get(&name_str) {
                            // Add selector
                            obj.insert(
                                "selector".to_string(),
                                json!(format!("0x{}", analyzed_func.selector)),
                            );

                            // Add signature
                            let signature = match &analyzed_func.resolved_function {
                                Some(sig) => sig.signature.clone(),
                                None => {
                                    // Build signature from the ABI inputs
                                    if let Some(inputs) =
                                        obj.get("inputs").and_then(|i| i.as_array())
                                    {
                                        let params: Vec<String> = inputs
                                            .iter()
                                            .filter_map(|input| {
                                                input
                                                    .get("type")
                                                    .and_then(|t| t.as_str())
                                                    .map(|s| s.to_string())
                                            })
                                            .collect();
                                        format!("{}({})", name_str, params.join(","))
                                    } else {
                                        format!("{name_str}()")
                                    }
                                }
                            };
                            obj.insert("signature".to_string(), json!(signature));
                        }
                    }
                }
            }
        }
    }

    debug!("adding function details took {:?}", start_time.elapsed());

    Ok(abi_array)
}
