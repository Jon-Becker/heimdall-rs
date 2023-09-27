
use std::{collections::HashMap, time::Duration};

use ethers::abi::AbiEncode;
use heimdall_common::{
    ether::signatures::ResolvedLog,
    io::{logging::{Logger, TraceFactory}, file::short_path},
};
use indicatif::ProgressBar;
use serde::{Deserialize, Serialize};

use crate::decompile::{DecompilerArgs, util::Function};

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct ABIToken {
    pub name: String,
    #[serde(rename = "internalType")]
    pub internal_type: String,
    #[serde(rename = "type")]
    pub type_: String,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct FunctionABI {
    #[serde(rename = "type")]
    pub type_: String,
    pub name: String,
    pub inputs: Vec<ABIToken>,
    pub outputs: Vec<ABIToken>,
    #[serde(rename = "stateMutability")]
    pub state_mutability: String,
    pub constant: bool,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct ErrorABI {
    #[serde(rename = "type")]
    pub type_: String,
    pub name: String,
    pub inputs: Vec<ABIToken>,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct EventABI {
    #[serde(rename = "type")]
    pub type_: String,
    pub name: String,
    pub inputs: Vec<ABIToken>,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub enum ABIStructure {
    Function(FunctionABI),
    Error(ErrorABI),
    Event(EventABI),
}


pub fn build_abi(
    args: &DecompilerArgs,
    functions: Vec<Function>,
    all_resolved_events: HashMap<String, ResolvedLog>,
    trace: &mut TraceFactory,
    trace_parent: u32,
) -> Result<Vec<ABIStructure>, Box<dyn std::error::Error>> {
    // get a new logger
    let level = std::env::var("RUST_LOG").unwrap_or_else(|_| "INFO".into());
    let (logger, _) = Logger::new(&level);

    // get a new progress bar
    let progress_bar = ProgressBar::new_spinner();
    progress_bar.enable_steady_tick(Duration::from_millis(100));
    progress_bar.set_style(logger.info_spinner());

    // truncate target for prettier display
    let mut shortened_target = args.target.clone();
    if shortened_target.len() > 66 {
        shortened_target = shortened_target.chars().take(66).collect::<String>() +
            "..." +
            &shortened_target.chars().skip(shortened_target.len() - 16).collect::<String>();
    }

    // add the call to the trace
    trace.add_call(
        trace_parent,
        line!(),
        "heimdall".to_string(),
        "build_abi".to_string(),
        vec![args.target.to_string()],
        short_path(&shortened_target),
    );

    // build the decompiled contract's ABI
    let mut abi: Vec<ABIStructure> = Vec::new();

    // build the ABI for each function
    for function in &functions {
        progress_bar.set_message(format!("building ABI for '0x{}'", function.selector));

        // get the function's name parameters for both resolved and unresolved functions
        let (function_name, function_inputs, function_outputs) = match &function.resolved_function {
            Some(resolved_function) => {
                // get the function's name and parameters from the resolved function
                let mut inputs = Vec::new();
                let mut outputs = Vec::new();

                for (index, input) in resolved_function.inputs.iter().enumerate() {
                    inputs.push(ABIToken {
                        name: format!("arg{index}"),
                        internal_type: input.to_owned(),
                        type_: input.to_owned(),
                    });
                }

                match &function.returns {
                    Some(returns) => {
                        outputs.push(ABIToken {
                            name: "ret0".to_owned(),
                            internal_type: returns.to_owned(),
                            type_: returns.to_owned(),
                        });
                    }
                    None => {}
                }

                (resolved_function.name.clone(), inputs, outputs)
            }
            None => {
                // if the function is unresolved, use the decompiler's potential types
                let mut inputs = Vec::new();
                let mut outputs = Vec::new();

                for (index, (_, (_, potential_types))) in
                    function.arguments.clone().iter().enumerate()
                {
                    inputs.push(ABIToken {
                        name: format!("arg{index}"),
                        internal_type: potential_types[0].to_owned(),
                        type_: potential_types[0].to_owned(),
                    });
                }

                match &function.returns {
                    Some(returns) => {
                        outputs.push(ABIToken {
                            name: "ret0".to_owned(),
                            internal_type: returns.to_owned(),
                            type_: returns.to_owned(),
                        });
                    }
                    None => {}
                }

                (format!("Unresolved_{}", function.selector), inputs, outputs)
            }
        };

        // determine the state mutability of the function
        let state_mutability = match function.payable {
            true => "payable",
            false => match function.pure {
                true => "pure",
                false => match function.view {
                    true => "view",
                    false => "nonpayable",
                },
            },
        };

        let constant = state_mutability == "pure" && function_inputs.is_empty();

        // add the function to the ABI
        abi.push(ABIStructure::Function(FunctionABI {
            type_: "function".to_string(),
            name: function_name,
            inputs: function_inputs,
            outputs: function_outputs,
            state_mutability: state_mutability.to_string(),
            constant: constant,
        }));

        // build the function's custom errors
        for (error_selector, resolved_error) in &function.errors {
            progress_bar.set_message(format!("building ABI for '0x{error_selector}'"));

            match resolved_error {
                Some(resolved_error) => {
                    let mut inputs = Vec::new();

                    for (index, input) in resolved_error.inputs.iter().enumerate() {
                        if !input.is_empty() {
                            inputs.push(ABIToken {
                                name: format!("arg{index}"),
                                internal_type: input.to_owned(),
                                type_: input.to_owned(),
                            });
                        }
                    }

                    // check if the error is already in the ABI
                    if abi.iter().any(|x| match x {
                        ABIStructure::Error(x) => x.name == resolved_error.name,
                        _ => false,
                    }) {
                        continue
                    }

                    abi.push(ABIStructure::Error(ErrorABI {
                        type_: "error".to_string(),
                        name: resolved_error.name.clone(),
                        inputs: inputs,
                    }));
                }
                None => {
                    // check if the error is already in the ABI
                    if abi.iter().any(|x| match x {
                        ABIStructure::Error(x) => {
                            x.name ==
                                format!(
                                    "CustomError_{}",
                                    &error_selector.encode_hex().replacen("0x", "", 1)
                                )
                        }
                        _ => false,
                    }) {
                        continue
                    }

                    abi.push(ABIStructure::Error(ErrorABI {
                        type_: "error".to_string(),
                        name: format!(
                            "CustomError_{}",
                            &error_selector.encode_hex().replacen("0x", "", 1)
                        ),
                        inputs: Vec::new(),
                    }));
                }
            }
        }

        // build the function's events
        for (event_selector, (resolved_event, _)) in &function.events {
            progress_bar.set_message(format!("building ABI for '0x{event_selector}'"));

            match resolved_event {
                Some(resolved_event) => {
                    let mut inputs = Vec::new();

                    for (index, input) in resolved_event.inputs.iter().enumerate() {
                        if !input.is_empty() {
                            inputs.push(ABIToken {
                                name: format!("arg{index}"),
                                internal_type: input.to_owned(),
                                type_: input.to_owned(),
                            });
                        }
                    }

                    // check if the event is already in the ABI
                    if abi.iter().any(|x| match x {
                        ABIStructure::Event(x) => x.name == resolved_event.name,
                        _ => false,
                    }) {
                        continue
                    }

                    abi.push(ABIStructure::Event(EventABI {
                        type_: "event".to_string(),
                        name: resolved_event.name.clone(),
                        inputs: inputs,
                    }));
                }
                None => {
                    // check if the event is already in the ABI
                    if abi.iter().any(|x| match x {
                        ABIStructure::Event(x) => {
                            x.name ==
                                format!(
                                    "Event_{}",
                                    &event_selector.encode_hex().replacen("0x", "", 1)[0..8]
                                )
                        }
                        _ => false,
                    }) {
                        continue
                    }

                    abi.push(ABIStructure::Event(EventABI {
                        type_: "event".to_string(),
                        name: format!(
                            "Event_{}",
                            &event_selector.encode_hex().replacen("0x", "", 1)[0..8]
                        ),
                        inputs: Vec::new(),
                    }));
                }
            }
        }
    }

    Ok(abi)
}
