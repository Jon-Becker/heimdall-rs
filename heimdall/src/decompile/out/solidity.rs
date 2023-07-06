use std::{collections::HashMap, time::Duration};

use ethers::abi::AbiEncode;
use heimdall_common::{
    ether::signatures::{ResolvedError, ResolvedLog},
    io::{
        file::{short_path, write_file, write_lines_to_file},
        logging::{Logger, TraceFactory},
    },
    utils::strings::find_balanced_encapsulator,
};
use indicatif::ProgressBar;

use super::{
    super::{
        constants::{DECOMPILED_SOURCE_HEADER_SOL, STORAGE_ACCESS_REGEX},
        util::Function,
        DecompilerArgs,
    },
    postprocessers::solidity::postprocess,
};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, PartialEq, Clone)]
struct ABIToken {
    name: String,
    #[serde(rename = "internalType")]
    internal_type: String,
    #[serde(rename = "type")]
    type_: String,
}

#[derive(Serialize, Deserialize, PartialEq, Clone)]
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

#[derive(Serialize, Deserialize, PartialEq, Clone)]
struct ErrorABI {
    #[serde(rename = "type")]
    type_: String,
    name: String,
    inputs: Vec<ABIToken>,
}

#[derive(Serialize, Deserialize, PartialEq, Clone)]
struct EventABI {
    #[serde(rename = "type")]
    type_: String,
    name: String,
    inputs: Vec<ABIToken>,
}

#[derive(Serialize, Deserialize, PartialEq, Clone)]
enum ABIStructure {
    Function(FunctionABI),
    Error(ErrorABI),
    Event(EventABI),
}

pub fn output(
    args: &DecompilerArgs,
    output_dir: String,
    functions: Vec<Function>,
    all_resolved_errors: HashMap<String, ResolvedError>,
    all_resolved_events: HashMap<String, ResolvedLog>,
    logger: &Logger,
    trace: &mut TraceFactory,
    trace_parent: u32,
) {
    let mut functions = functions;

    let progress_bar = ProgressBar::new_spinner();
    progress_bar.enable_steady_tick(Duration::from_millis(100));
    progress_bar.set_style(logger.info_spinner());

    let abi_output_path = format!("{output_dir}/abi.json");
    let decompiled_output_path = format!("{output_dir}/decompiled.sol");

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

    trace.add_call(
        trace_parent,
        line!(),
        "heimdall".to_string(),
        "build_output".to_string(),
        vec![shortened_target],
        short_path(&decompiled_output_path),
    );

    // write the header to the output file
    decompiled_output.push(DECOMPILED_SOURCE_HEADER_SOL.replace("{}", env!("CARGO_PKG_VERSION")));
    decompiled_output.push(String::from("contract DecompiledContract {"));

    // add blank line if there are events
    if abi.iter().any(|x| matches!(x, ABIStructure::Event(_))) {
        decompiled_output.push(String::from(""));
    }

    // write the contract's events
    for event in abi.iter().filter(|x| matches!(x, ABIStructure::Event(_))) {
        if let ABIStructure::Event(event) = event {
            decompiled_output.push(format!(
                "event {}({});",
                event.name,
                event
                    .inputs
                    .iter()
                    .map(|x| format!("{} {}", x.type_, x.name))
                    .collect::<Vec<String>>()
                    .join(", ")
            ));
        }
    }

    // add blank line if there are errors
    if abi.iter().any(|x| matches!(x, ABIStructure::Error(_))) {
        decompiled_output.push(String::from(""));
    }

    // write the contract's errors
    for error in abi.iter().filter(|x| matches!(x, ABIStructure::Error(_))) {
        if let ABIStructure::Error(error) = error {
            decompiled_output.push(format!(
                "error {}({});",
                error.name,
                error
                    .inputs
                    .iter()
                    .map(|x| format!("{} {}", x.type_, x.name))
                    .collect::<Vec<String>>()
                    .join(", ")
            ));
        }
    }

    // check for any constants or storage getters
    for function in functions.iter_mut() {
        if function.payable || (!function.pure && !function.view) || !function.arguments.is_empty()
        {
            continue
        }

        // check for RLP encoding. very naive check, but it works for now
        if function.logic.iter().any(|line| line.contains("0x0100 *")) &&
            function.logic.iter().any(|line| line.contains("0x01) &"))
        {
            // find any storage accesses
            let joined = function.logic.join(" ");
            let storage_access = match STORAGE_ACCESS_REGEX.find(&joined).unwrap() {
                Some(x) => x.as_str(),
                None => continue,
            };

            let storage_access_loc = find_balanced_encapsulator(storage_access, ('[', ']'));

            function.logic = vec![format!(
                "return string(rlp.encodePacked(storage[{}]));",
                storage_access[storage_access_loc.0 + 1..storage_access_loc.1 - 1].to_string()
            )]
        }
    }

    for function in functions {
        progress_bar.set_message(format!("writing logic for '0x{}'", function.selector));

        // build the function's header and parameters
        let function_modifiers = format!(
            "public {}{}",
            if function.pure {
                "pure "
            } else if function.view {
                "view "
            } else {
                ""
            },
            if function.payable { "payable " } else { "" },
        );
        let function_returns = format!(
            "returns ({}) {{",
            if function.returns.is_some() {
                function.returns.clone().unwrap()
            } else {
                String::from("")
            }
        );

        let function_header = match function.resolved_function {
            Some(resolved_function) => {
                format!(
                    "function {}({}) {}{}",
                    resolved_function.name,
                    resolved_function
                        .inputs
                        .iter()
                        .enumerate()
                        .map(|(index, solidity_type)| {
                            format!(
                                "{} {}arg{}",
                                solidity_type,
                                if solidity_type.contains("[]") ||
                                    solidity_type.contains('(') ||
                                    ["string", "bytes"].contains(&solidity_type.as_str())
                                {
                                    "memory "
                                } else {
                                    ""
                                },
                                index
                            )
                        })
                        .collect::<Vec<String>>()
                        .join(", "),
                    function_modifiers,
                    if function.returns.is_some() { function_returns } else { String::from("{") },
                )
            }
            None => {
                // sort arguments by their calldata index
                let mut sorted_arguments: Vec<_> = function.arguments.clone().into_iter().collect();
                sorted_arguments.sort_by(|x, y| x.0.cmp(&y.0));

                format!(
                    "function Unresolved_{}({}) {}{}",
                    function.selector,
                    sorted_arguments
                        .iter()
                        .map(|(index, (_, potential_types))| {
                            format!(
                                "{} {}arg{}",
                                potential_types[0],
                                if potential_types[0].contains("[]") ||
                                    potential_types[0].contains('(') ||
                                    ["string", "bytes"].contains(&potential_types[0].as_str())
                                {
                                    "memory "
                                } else {
                                    ""
                                },
                                index
                            )
                        })
                        .collect::<Vec<String>>()
                        .join(", "),
                    function_modifiers,
                    if function.returns.is_some() { function_returns } else { String::from("{") },
                )
            }
        };

        // print natspec header for the function
        decompiled_output.extend(vec![
            String::new(),
            format!("/// @custom:selector    0x{}", function.selector),
            format!(
                "/// @custom:name        {}",
                function_header.replace("function ", "").split('(').next().unwrap()
            ),
        ]);

        for notice in function.notices {
            decompiled_output.push(format!("/// @notice             {notice}"));
        }

        // sort arguments by their calldata index
        let mut sorted_arguments: Vec<_> = function.arguments.into_iter().collect();
        sorted_arguments.sort_by(|x, y| x.0.cmp(&y.0));

        for (index, (_, solidity_type)) in sorted_arguments {
            decompiled_output.push(format!("/// @param              arg{index} {solidity_type:?}"));
        }

        decompiled_output.push(function_header);

        // build the function's body
        decompiled_output.extend(function.logic);

        decompiled_output.push(String::from("}"));
    }

    decompiled_output.push(String::from("}"));

    if args.include_solidity {
        write_lines_to_file(
            &decompiled_output_path,
            postprocess(decompiled_output, all_resolved_errors, all_resolved_events, &progress_bar),
        );
        logger.success(&format!("wrote decompiled contract to '{}' .", &decompiled_output_path));
        progress_bar.finish_and_clear();
    } else {
        progress_bar.finish_and_clear();
    }
}
