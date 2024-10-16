use alloy::primitives::U256;
use alloy_dyn_abi::{DynSolType, DynSolValue};
use heimdall_common::utils::{
    hex::ToLowerHex,
    strings::{encode_hex, encode_hex_reduced},
};
use heimdall_vm::core::{opcodes::opcode_name, vm::State};

use crate::{
    core::analyze::AnalyzerState, interfaces::AnalyzedFunction,
    utils::precompile::decode_precompile, Error,
};

pub fn extcall_heuristic(
    function: &mut AnalyzedFunction,
    state: &State,
    analyzer_state: &mut AnalyzerState,
) -> Result<(), Error> {
    let instruction = &state.last_instruction;

    match instruction.opcode {
        // CALL / CALLCODE
        0xf1 | 0xf2 => {
            let gas = format!("gas: {}", instruction.input_operations[0].solidify());
            let address = instruction.input_operations[1].solidify();
            let value = format!("value: {}", instruction.input_operations[2].solidify());
            let memory = function.get_memory_range(instruction.inputs[3], instruction.inputs[4]);
            let extcalldata = memory
                .iter()
                .map(|x| x.value.to_lower_hex().trim_start_matches("0x").to_owned())
                .collect::<Vec<String>>()
                .join("");

            // build the modifier w/ gas and value
            let modifier = format!("{{ {}, {} }}", gas, value);

            // check if the external call is a precompiled contract
            match decode_precompile(
                instruction.inputs[1],
                &memory,
                &instruction.input_operations[5],
            ) {
                (true, precompile_logic) => {
                    function.logic.push(precompile_logic);
                }
                _ => {
                    function.logic.push(format!(
                        "(bool success, bytes memory ret0) = address({}).call{}(abi.encode({}));",
                        address, modifier, extcalldata
                    ));
                }
            }
        }

        // STATICCALL / DELEGATECALL
        0xfa | 0xf4 => {
            let gas = format!("gas: {}", instruction.input_operations[0].solidify());
            let address = instruction.input_operations[1].solidify();
            let memory = function.get_memory_range(instruction.inputs[2], instruction.inputs[3]);
            let extcalldata = memory
                .iter()
                .map(|x| x.value.to_lower_hex().trim_start_matches("0x").to_owned())
                .collect::<Vec<String>>()
                .join("");

            // build the modifier w/ gas
            let modifier = format!("{{ {} }}", gas);

            // check if the external call is a precompiled contract
            match decode_precompile(
                instruction.inputs[1],
                &memory,
                &instruction.input_operations[4],
            ) {
                (true, precompile_logic) => {
                    function.logic.push(precompile_logic);
                }
                _ => {
                    function.logic.push(format!(
                        "(bool success, bytes memory ret0) = address({}).{}{}(abi.encode({}));",
                        address,
                        opcode_name(instruction.opcode).to_lowercase(),
                        modifier,
                        extcalldata
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
                    Some(hex_data) => match DynSolType::String.abi_decode(hex_data) {
                        Ok(revert) => match revert {
                            DynSolValue::String(revert) => revert,
                            _ => "decoding error".to_string(),
                        },
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
                        return Ok(());
                    }
                }
            }
            // handle case with custom error OR empty revert
            else if !revert_data.starts_with(&[0x4e, 0x48, 0x7b, 0x71]) {
                let custom_error_placeholder = match revert_data.get(0..4) {
                    Some(selector) => {
                        function.errors.insert(U256::from_be_slice(selector));
                        format!(
                            "CustomError_{}()",
                            encode_hex_reduced(U256::from_be_slice(selector)).replacen("0x", "", 1)
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
                        return Ok(());
                    }
                }
            } else {
                return Ok(());
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
