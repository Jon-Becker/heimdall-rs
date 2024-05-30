use std::{collections::HashMap, time::Instant};

use alloy_json_abi::{Error, Event, EventParam, Function, JsonAbi, Param, StateMutability};

use eyre::Result;
use heimdall_common::{
    ether::signatures::{ResolvedError, ResolvedLog},
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
    functions.iter().for_each(|f| {
        // determine the state mutability of the function
        let state_mutability = match f.payable {
            true => StateMutability::Payable,
            false => match f.pure {
                true => StateMutability::Pure,
                false => match f.view {
                    true => StateMutability::View,
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
                        Some(ref sig) => sig.inputs[i].clone(),
                        None => arg
                            .potential_types()
                            .first()
                            .unwrap_or(&"bytes32".to_string())
                            .to_string(),
                    },
                    components: vec![],
                })
                .collect(),
            outputs: f
                .returns
                .as_ref()
                .map(|r| {
                    vec![Param {
                        name: "".to_string(),
                        internal_type: None,
                        ty: if r == "bytes memory" { "bytes".to_string() } else { r.clone() },
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
                        .inputs
                        .iter()
                        .enumerate()
                        .map(|(i, input)| Param {
                            name: format!("arg{i}"),
                            internal_type: None,
                            ty: input.clone(),
                            components: vec![],
                        })
                        .collect(),
                ),
                None => (format!("CustomError_{}", error_selector.to_lower_hex()), vec![]),
            };

            let error = Error { name: name.clone(), inputs };

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
                        .inputs
                        .iter()
                        .enumerate()
                        .map(|(i, input)| EventParam {
                            name: format!("arg{i}"),
                            internal_type: None,
                            ty: input.clone(),
                            components: vec![],
                            indexed: false,
                        })
                        .collect(),
                ),
                None => (format!("Event_{}", event_selector.to_lower_hex()), vec![]),
            };

            let event = Event { name: name.clone(), inputs, anonymous: event_selector.is_zero() };

            abi.events.insert(event.name.clone(), vec![event]);
        });

        abi.functions.insert(name, vec![function]);
    });

    debug!("constructing abi took {:?}", start_time.elapsed());

    Ok(abi)
}
