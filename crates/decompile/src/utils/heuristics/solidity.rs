use ethers::{
    abi::{decode, ParamType},
    types::U256,
};

use heimdall_common::{
    ether::evm::core::{
        opcodes::{WrappedInput, WrappedOpcode},
        vm::State,
    },
    utils::strings::encode_hex_reduced,
};

use crate::{
    core::analyze::AnalyzerState,
    interfaces::{AnalyzedFunction, StorageFrame},
    utils::{constants::VARIABLE_SIZE_CHECK_REGEX, precompile::decode_precompile},
    Error,
};

pub fn solidity_heuristic(
    function: &mut AnalyzedFunction,
    state: &State,
    analyzer_state: &mut AnalyzerState,
) -> Result<(), Error> {
    let instruction = &state.last_instruction;

    match instruction.opcode {
        // CALLDATACOPY
        0x37 => {
            let memory_offset = &instruction.input_operations[0];
            let source_offset = instruction.inputs[1];
            let size_bytes = instruction.inputs[2];

            // add the mstore to the function's memory map
            function.logic.push(format!(
                "memory[{}] = msg.data[{}:{}];",
                memory_offset.solidify(),
                source_offset,
                source_offset.saturating_add(size_bytes)
            ));
        }

        // CODECOPY
        0x39 => {
            let memory_offset = &instruction.input_operations[0];
            let source_offset = instruction.inputs[1];
            let size_bytes = instruction.inputs[2];

            // add the mstore to the function's memory map
            function.logic.push(format!(
                "memory[{}] = this.code[{}:{}];",
                memory_offset.solidify(),
                source_offset,
                source_offset.saturating_add(size_bytes)
            ));
        }

        // EXTCODECOPY
        0x3C => {
            let address = &instruction.input_operations[0];
            let memory_offset = &instruction.input_operations[1];
            let source_offset = instruction.inputs[2];
            let size_bytes = instruction.inputs[3];

            // add the mstore to the function's memory map
            function.logic.push(format!(
                "memory[{}] = address({}).code[{}:{}]",
                memory_offset.solidify(),
                address.solidify(),
                source_offset,
                source_offset.saturating_add(size_bytes)
            ));
        }

        // MSTORE / MSTORE8
        0x52 | 0x53 => {
            let key = instruction.inputs[0];
            let value = instruction.inputs[1];
            let operation = instruction.input_operations[1].clone();

            // add the mstore to the function's memory map
            function.memory.insert(key, StorageFrame { value, operations: operation });
            function.logic.push(format!(
                "memory[{}] = {};",
                encode_hex_reduced(key),
                instruction.input_operations[1].solidify()
            ));
        }

        // SSTORE
        0x55 => {
            function.logic.push(format!(
                "storage[{}] = {};",
                instruction.input_operations[0].solidify(),
                instruction.input_operations[1].solidify(),
            ));
        }

        // JUMPI
        0x57 => {
            // this is an if conditional for the children branches
            let conditional = instruction.input_operations[1].solidify();

            // perform a series of checks to determine if the condition
            // is added by the compiler and can be ignored
            if (conditional.contains("msg.data.length") && conditional.contains("0x04")) ||
                VARIABLE_SIZE_CHECK_REGEX.is_match(&conditional).unwrap_or(false) ||
                (conditional.replace('!', "") == "success") ||
                (conditional == "!msg.value")
            {
                return Ok(())
            }

            function.logic.push(format!("if ({conditional}) {{").to_string());

            // save a copy of the conditional and add it to the conditional map
            analyzer_state.jumped_conditional = Some(conditional.clone());
            analyzer_state.conditional_stack.push(conditional);
        }

        // TSTORE
        0x5d => {
            function.logic.push(format!(
                "transient[{}] = {};",
                instruction.input_operations[0].solidify(),
                instruction.input_operations[1].solidify(),
            ));
        }

        // CREATE / CREATE2
        0xf0 | 0xf5 => {
            function.logic.push(format!(
                "assembly {{ addr := create({}) }}",
                instruction
                    .input_operations
                    .iter()
                    .map(|x| x.solidify())
                    .collect::<Vec<String>>()
                    .join(", ")
            ));
        }

        // CALL / CALLCODE
        0xf1 | 0xf2 => {
            let gas = format!("gas: {}", instruction.input_operations[0].solidify());
            let address = instruction.input_operations[1].solidify();
            let value = format!("value: {}", instruction.input_operations[2].solidify());
            let calldata = function.get_memory_range(instruction.inputs[3], instruction.inputs[4]);

            // build the modifier w/ gas and value
            let modifier = format!("{{ {}, {} }}", gas, value);

            // check if the external call is a precompiled contract
            match decode_precompile(
                instruction.inputs[1],
                calldata.clone(),
                instruction.input_operations[5].clone(),
            ) {
                (true, precompile_logic) => {
                    function.logic.push(precompile_logic);
                }
                _ => {
                    function.logic.push(format!(
                        "(bool success, bytes memory ret0) = address({}).call{}(abi.encode({}));",
                        address,
                        modifier,
                        calldata
                            .iter()
                            .map(|x| x.operations.solidify())
                            .collect::<Vec<String>>()
                            .join(", ")
                    ));
                }
            }
        }

        // STATICCALL / DELEGATECALL
        0xfa | 0xf4 => {
            let gas = format!("gas: {}", instruction.input_operations[0].solidify());
            let address = instruction.input_operations[1].solidify();
            let calldata = function.get_memory_range(instruction.inputs[2], instruction.inputs[3]);

            // build the modifier w/ gas
            let modifier = format!("{{ {} }}", gas);

            // check if the external call is a precompiled contract
            match decode_precompile(
                instruction.inputs[1],
                calldata.clone(),
                instruction.input_operations[4].clone(),
            ) {
                (true, precompile_logic) => {
                    function.logic.push(precompile_logic);
                }
                _ => {
                    function.logic.push(format!(
                        "(bool success, bytes memory ret0) = address({}).{}{}(abi.encode({}));",
                        address,
                        modifier,
                        instruction.opcode_details.clone().expect("impossible").name.to_lowercase(),
                        calldata
                            .iter()
                            .map(|x| x.operations.solidify())
                            .collect::<Vec<String>>()
                            .join(", ")
                    ));
                }
            }
        }

        // REVERT
        0xfd => {
            // Safely convert U256 to usize
            let offset: usize = instruction.inputs[0].try_into().unwrap_or(0);
            let size: usize = instruction.inputs[1].try_into().unwrap_or(0);
            let revert_data = state.memory.read(offset, size);

            // (1) if revert_data starts with 0x08c379a0, the folling is an error string
            // abiencoded (2) if revert_data starts with 0x4e487b71, the
            // following is a compiler panic (3) if revert_data starts with any
            // other 4byte selector, it is a custom error and should
            //     be resolved and added to the generated ABI
            // (4) if revert_data is empty, it is an empty revert. Ex:
            //       - if (true != false) { revert() };
            //       - require(true != false)
            let revert_logic;

            // handle case with error string abiencoded
            if revert_data.starts_with(&[0x08, 0xc3, 0x79, 0xa0]) {
                let revert_string = match revert_data.get(4..) {
                    Some(hex_data) => match decode(&[ParamType::String], hex_data) {
                        Ok(revert) => revert[0].to_string(),
                        Err(_) => "decoding error".to_string(),
                    },
                    None => "decoding error".to_string(),
                };
                revert_logic = match analyzer_state.jumped_conditional.clone() {
                    Some(condition) => {
                        analyzer_state.jumped_conditional = None;
                        format!("require({condition}, \"{revert_string}\");")
                    }
                    None => {
                        // loop backwards through logic to find the last IF statement
                        for i in (0..function.logic.len()).rev() {
                            if function.logic[i].starts_with("if") {
                                let conditional = match analyzer_state.conditional_stack.pop() {
                                    Some(condition) => condition,
                                    None => break,
                                };

                                function.logic[i] =
                                    format!("require({conditional}, \"{revert_string}\");");
                            }
                        }
                        return Ok(())
                    }
                }
            }
            // handle case with custom error OR empty revert
            else if !revert_data.starts_with(&[0x4e, 0x48, 0x7b, 0x71]) {
                let custom_error_placeholder = match revert_data.get(0..4) {
                    Some(selector) => {
                        function.errors.insert(U256::from(selector));
                        format!(
                            "CustomError_{}()",
                            encode_hex_reduced(U256::from(selector)).replacen("0x", "", 1)
                        )
                    }
                    None => "()".to_string(),
                };

                revert_logic = match analyzer_state.jumped_conditional.clone() {
                    Some(condition) => {
                        analyzer_state.jumped_conditional = None;
                        if custom_error_placeholder == *"()" {
                            format!("require({condition});",)
                        } else {
                            format!("require({condition}, {custom_error_placeholder});")
                        }
                    }
                    None => {
                        // loop backwards through logic to find the last IF statement
                        for i in (0..function.logic.len()).rev() {
                            if function.logic[i].starts_with("if") {
                                let conditional = match analyzer_state.conditional_stack.pop() {
                                    Some(condition) => condition,
                                    None => break,
                                };

                                if custom_error_placeholder == *"()" {
                                    function.logic[i] = format!("require({conditional});",);
                                } else {
                                    function.logic[i] = format!(
                                        "require({conditional}, {custom_error_placeholder});"
                                    );
                                }
                            }
                        }
                        return Ok(())
                    }
                }
            } else {
                return Ok(())
            }

            function.logic.push(revert_logic);
        }

        // SELFDESTRUCT
        0xff => {
            function
                .logic
                .push(format!("selfdestruct({});", instruction.input_operations[0].solidify()));
        }

        _ => {}
    };

    Ok(())
}
