use std::{collections::HashMap, time::Instant};

use alloy_json_abi::{Error, Event, EventParam, Function, JsonAbi, Param, StateMutability};

use eyre::Result;
use heimdall_common::{
    ether::{
        signatures::{ResolvedError, ResolvedLog},
        types::{to_abi_string, to_components},
    },
    utils::{hex::ToLowerHex, strings::encode_hex_reduced},
};

use tracing::debug;

use crate::interfaces::AnalyzedFunction;

pub fn build_abi(
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
            // Some(ref sig) => sig.name.clone(),
            Some(_) => format!("Unresolved_{}", f.selector),
            None => format!("Unresolved_{}", f.selector),
        };

        // // if f.resolved_function.as_ref().is_some_and(|x| x.inputs.len() != f.arguments.len()) 
        // if f.resolved_function.is_some()
        // {
        //     println!("resolved: {:?} vs args: {:?}", f.resolved_function.as_ref().unwrap(), f.sorted_arguments());
        // }
        // let name = f.selector.clone(); // we bad hackers

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
                        Some(ref sig) => to_abi_string(&sig.inputs()[i]),
                        None => arg
                            .potential_types()
                            .first()
                            .unwrap_or(&"bytes32".to_string())
                            .to_string(),
                    },
                    components: match f.resolved_function {
                        Some(ref sig) => to_components(&sig.inputs()[i]),
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
