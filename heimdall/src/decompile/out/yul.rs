use std::{collections::HashMap, time::Duration};

use ethers::abi::AbiEncode;
use heimdall_common::{
    ether::signatures::ResolvedLog,
    io::{
        file::{short_path, write_file, write_lines_to_file},
        logging::{Logger, TraceFactory},
    },
};
use indicatif::ProgressBar;

use super::{
    super::{constants::DECOMPILED_SOURCE_HEADER_YUL, util::Function, DecompilerArgs},
    postprocessers::yul::postprocess,
};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, PartialEq)]
struct ABIToken {
    name: String,
    #[serde(rename = "internalType")]
    internal_type: String,
    #[serde(rename = "type")]
    type_: String,
}

#[derive(Serialize, Deserialize, PartialEq)]
struct FunctionABI {
    #[serde(rename = "type")]
    type_: String,
    name: String,
    inputs: Vec<ABIToken>,
    outputs: Vec<ABIToken>,
    #[serde(rename = "stateMutability")]
    state_mutability: String,
    constant: bool,
}

#[derive(Serialize, Deserialize, PartialEq)]
struct ErrorABI {
    #[serde(rename = "type")]
    type_: String,
    name: String,
    inputs: Vec<ABIToken>,
}

#[derive(Serialize, Deserialize, PartialEq)]
struct EventABI {
    #[serde(rename = "type")]
    type_: String,
    name: String,
    inputs: Vec<ABIToken>,
}

#[derive(Serialize, Deserialize, PartialEq)]
enum ABIStructure {
    Function(FunctionABI),
    Error(ErrorABI),
    Event(EventABI),
}

pub fn output(
    args: &DecompilerArgs,
    output_dir: String,
    functions: Vec<Function>,
    all_resolved_events: HashMap<String, ResolvedLog>,
    logger: &Logger,
    trace: &mut TraceFactory,
    trace_parent: u32,
) {
    let progress_bar = ProgressBar::new_spinner();
    progress_bar.enable_steady_tick(Duration::from_millis(100));
    progress_bar.set_style(logger.info_spinner());

    let abi_output_path = format!("{output_dir}/abi.json");
    let decompiled_output_path = format!("{output_dir}/decompiled.yul");

    // build the decompiled contract's ABI
    let mut abi: Vec<ABIStructure> = Vec::new();

    // build the ABI for each function
    for function in &functions {
        progress_bar.set_message(format!("writing ABI for '0x{}'", function.selector));

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

        // write the function's custom errors
        for (error_selector, resolved_error) in &function.errors {
            progress_bar.set_message(format!("writing ABI for '0x{error_selector}'"));

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

        // write the function's events
        for (event_selector, (resolved_event, _)) in &function.events {
            progress_bar.set_message(format!("writing ABI for '0x{event_selector}'"));

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

    // write the ABI to a file
    write_file(
        &abi_output_path,
        &format!(
            "[{}]",
            abi.iter()
                .map(|x| {
                    match x {
                        ABIStructure::Function(x) => serde_json::to_string_pretty(x).unwrap(),
                        ABIStructure::Error(x) => serde_json::to_string_pretty(x).unwrap(),
                        ABIStructure::Event(x) => serde_json::to_string_pretty(x).unwrap(),
                    }
                })
                .collect::<Vec<String>>()
                .join(",\n")
        ),
    );

    // log the ABI output path
    progress_bar.suspend(|| {
        logger.success(&format!("wrote decompiled ABI to '{}' .", &abi_output_path));
    });

    // write the decompiled source to file
    let mut decompiled_output: Vec<String> = Vec::new();

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
        "build_output".to_string(),
        vec![args.target.to_string()],
        short_path(&shortened_target),
    );

    // write the header to the output file
    decompiled_output.extend(
        DECOMPILED_SOURCE_HEADER_YUL
            .replace("{}", env!("CARGO_PKG_VERSION"))
            .split('\n')
            .map(|x| x.to_string()),
    );

    // write contract logic
    for function in functions {
        progress_bar.set_message(format!("writing logic for '0x{}'", function.selector));

        // build the function's header and parameters
        let function_header = match function.resolved_function {
            Some(resolved_function) => {
                format!(
                    "{}({})",
                    resolved_function.name,
                    resolved_function
                        .inputs
                        .iter()
                        .map(|solidity_type| {
                            format!(
                                "{}{}",
                                solidity_type,
                                if solidity_type.contains("[]") ||
                                    solidity_type.contains('(') ||
                                    ["string", "bytes"].contains(&solidity_type.as_str())
                                {
                                    " memory"
                                } else {
                                    ""
                                }
                            )
                        })
                        .collect::<Vec<String>>()
                        .join(", "),
                )
            }
            None => {
                // sort arguments by their calldata index
                let mut sorted_arguments: Vec<_> = function.arguments.clone().into_iter().collect();
                sorted_arguments.sort_by(|x, y| x.0.cmp(&y.0));

                format!(
                    "Unresolved_{}({})",
                    function.selector,
                    sorted_arguments
                        .iter()
                        .map(|(_, (_, potential_types))| {
                            format!(
                                "{}{}",
                                potential_types[0],
                                if potential_types[0].contains("[]") ||
                                    potential_types[0].contains('(') ||
                                    ["string", "bytes"].contains(&potential_types[0].as_str())
                                {
                                    " memory"
                                } else {
                                    ""
                                },
                            )
                        })
                        .collect::<Vec<String>>()
                        .join(", ")
                )
            }
        };

        // sort arguments by their calldata index
        let mut sorted_arguments: Vec<_> = function.arguments.into_iter().collect();
        sorted_arguments.sort_by(|x, y| x.0.cmp(&y.0));

        decompiled_output
            .push(format!("case 0x{} /* \"{}\" */ {{", function.selector, function_header));
        decompiled_output.extend(function.logic);
        decompiled_output.push(String::from("}"));
    }

    // closing brackets
    decompiled_output.append(&mut vec![
        "default { revert(0, 0) }".to_string(),
        "}".to_string(),
        "}".to_string(),
        "}".to_string(),
    ]);

    if args.include_yul {
        write_lines_to_file(
            &decompiled_output_path,
            postprocess(decompiled_output, all_resolved_events, &progress_bar),
        );
        logger.success(&format!("wrote decompiled contract to '{}' .", &decompiled_output_path));
        progress_bar.finish_and_clear();
    } else {
        progress_bar.finish_and_clear();
    }
}
