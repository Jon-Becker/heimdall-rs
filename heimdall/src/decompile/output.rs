use std::time::Duration;

use heimdall_common::io::{logging::{TraceFactory, Logger}, file::{short_path, write_lines_to_file, write_file}};
use indicatif::ProgressBar;

use super::{DecompilerArgs, util::Function, constants::DECOMPILED_SOURCE_HEADER};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct ABIToken {
    name: String,
    #[serde(rename = "internalType")]
    internal_type: String,
    #[serde(rename = "type")]
    type_: String,
}

#[derive(Serialize, Deserialize)]
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


pub fn build_output(
    args: &DecompilerArgs,
    output_dir: String,
    functions: Vec<Function>,
    logger: &Logger,
    trace: &mut TraceFactory,
    trace_parent: u32
) {
    let progress_bar = ProgressBar::new_spinner();
    progress_bar.enable_steady_tick(Duration::from_millis(100));
    progress_bar.set_style(logger.info_spinner());

    let abi_output_path = format!("{}/abi.json", output_dir);
    let decompiled_output_path = format!("{}/decompiled.sol", output_dir);

    // build the decompiled contract's ABI
    let mut abi = Vec::new();

    // build the ABI for each function
    for function in functions.clone() {
        progress_bar.set_message(format!("writing ABI for '0x{}'", function.selector));

        // get the function's name and parameters for both resolved and unresolved functions
        let (
            function_name,
            function_inputs,
            function_outputs,
        ) = match function.resolved_function {
            Some(resolved_function) => {

                // get the function's name and parameters from the resolved function
                let mut inputs = Vec::new();
                let mut outputs = Vec::new();

                for (index, input) in resolved_function.inputs.iter().enumerate() {
                    inputs.push(ABIToken {
                        name: format!("arg{}", index),
                        internal_type: input.to_owned(),
                        type_: input.to_owned(),
                    });
                }

                match function.returns {
                    Some(returns) => {
                        outputs.push(ABIToken {
                            name: "ret0".to_owned(),
                            internal_type: returns.to_owned(),
                            type_: returns.to_owned(),
                        });
                    }
                    None => {}
                }

                (resolved_function.name, inputs, outputs)
            },
            None => {

                // if the function is unresolved, use the decompiler's potential types
                let mut inputs = Vec::new();
                let mut outputs = Vec::new();

                for (index, (_, (_, potential_types))) in function.arguments.iter().enumerate() {
                    inputs.push(ABIToken {
                        name: format!("arg{}", index),
                        internal_type: potential_types[0].to_owned(),
                        type_: potential_types[0].to_owned(),
                    });
                }

                match function.returns {
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
                }
            },
        };

        let constant = state_mutability == "pure" && function_inputs.len() == 0;

        // add the function to the ABI
        abi.push(FunctionABI {
            type_: "function".to_string(),
            name: function_name,
            inputs: function_inputs,
            outputs: function_outputs,
            state_mutability: state_mutability.to_string(),
            constant: constant,
        });
    }

    // write the ABI to a file
    write_file(
        &abi_output_path, 
        &format!(
            "[{}]",
            abi.iter().map(|x| {
                serde_json::to_string_pretty(x).unwrap()
            }).collect::<Vec<String>>().join(",\n")
        )
    );

    // write the decompiled source to file
    let mut decompiled_output: Vec<String> = Vec::new();

    trace.add_call(
        trace_parent, 
        line!(), 
        "heimdall".to_string(), 
        "build_output".to_string(),
        vec![args.target.to_string()], 
        format!("{}", short_path(&decompiled_output_path))
    );
    
    // write the header to the output file
    decompiled_output.push(
        DECOMPILED_SOURCE_HEADER.replace(
            "{}", 
            env!("CARGO_PKG_VERSION")
        )
    );

    decompiled_output.push(String::from("contract DecompiledContract {"));

    for function in functions {
        progress_bar.set_message(format!("writing logic for '0x{}'", function.selector));

        // build the function's header and parameters
        let function_modifiers = format!(
            "public {}{}",
            if function.pure { "pure " }
            else if function.view { "view " }
            else { "" },
            if function.payable { "payable" }
            else { "" },
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

                    resolved_function.inputs.iter().enumerate().map(|(index, solidity_type)| {
                        format!(
                            "{} {}arg{}",
                            solidity_type,

                            if solidity_type.contains("[]") || 
                               solidity_type.contains("(") || 
                               ["string", "bytes"].contains(&solidity_type.as_str()) {"memory "} 
                            else { "" },

                            index
                    )
                    }).collect::<Vec<String>>().join(", "),

                    function_modifiers,
                    if function.returns.is_some() { function_returns }
                    else { String::from("{") },
                )
            },
            None => {
                format!(
                    "function Unresolved_{}() {}{}",
                    function.selector,
                    function_modifiers,
                    if function.returns.is_some() { function_returns }
                    else { String::from("{") },
                )
            }
        };
        decompiled_output.push(function_header);

        // build the function's body
        // TODO
        decompiled_output.append(function.logic.clone().as_mut());

        decompiled_output.push(String::from("}"));
    }

    decompiled_output.push(String::from("}"));


    // add indentation to the decompiled source
    let mut indentation = 0;
    for line in decompiled_output.iter_mut() {
        if line.starts_with("}") {
            indentation -= 1;
        }

        *line = format!(
            "{}{}",
            " ".repeat(indentation*4),
            line
        );
        
        if line.ends_with("{") {
            indentation += 1;
        }
        
    }

    // write the output to the file
    write_lines_to_file(&decompiled_output_path, decompiled_output)
}