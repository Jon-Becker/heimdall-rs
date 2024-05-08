use std::collections::HashSet;

use ethers::types::U256;

use heimdall_common::ether::evm::core::{
    types::{byte_size_to_type, convert_bitmask},
    vm::State,
};
use tracing::{debug, trace};

use crate::{
    core::analyze::{AnalyzerState, AnalyzerType},
    interfaces::{AnalyzedFunction, CalldataFrame, TypeHeuristic},
    utils::constants::{AND_BITMASK_REGEX, AND_BITMASK_REGEX_2},
    Error,
};

pub fn argument_heuristic(
    function: &mut AnalyzedFunction,
    state: &State,
    analyzer_state: &mut AnalyzerState,
) -> Result<(), Error> {
    match state.last_instruction.opcode {
        // CALLDATALOAD
        0x35 => {
            // calculate the argument index, with the 4byte signature padding removed
            // for example, CALLDATALOAD(4) -> (4-4)/32 = 0
            //              CALLDATALOAD(36) -> (36-4)/32 = 1
            let arg_index = (state.last_instruction.inputs[0].saturating_sub(U256::from(4)) / 32)
                .try_into()
                .unwrap_or(usize::MAX);

            // insert only if this argument is not already in the hashmap
            function.arguments.entry(arg_index).or_insert_with(|| {
                debug!(
                    "discovered new argument at index {} from CALLDATALOAD({})",
                    arg_index, state.last_instruction.inputs[0]
                );
                CalldataFrame {
                    arg_op: state.last_instruction.input_operations[0].to_string(),
                    mask_size: 32, // init to 32 because all CALLDATALOADs are 32 bytes
                    heuristics: HashSet::new(),
                }
            });
        }

        // CALLDATACOPY
        0x37 => {
            // TODO: implement CALLDATACOPY support
            trace!("CALLDATACOPY detected; not implemented");
        }

        // AND | OR
        0x16 | 0x17 => {
            // if this is a bitwise mask operation on CALLDATALOAD, we can use it to determine the
            // size (and consequently type) of the variable
            if let Some(calldataload_op) =
                state.last_instruction.input_operations.iter().find(|op| op.opcode.code == 0x35)
            {
                // this is a bitwise mask, we can use it to determine the size of the variable
                let (mask_size_bytes, _potential_types) =
                    convert_bitmask(state.last_instruction.clone());

                // yulify the calldataload operation, and find the associated argument index
                // this MUST exist, as we have already inserted it in the CALLDATALOAD heuristic
                let arg_op = calldataload_op.inputs[0].to_string();
                if let Some((arg_index, frame)) =
                    function.arguments.iter_mut().find(|(_, frame)| frame.arg_op == arg_op)
                {
                    debug!(
                        "instruction {} ({}) indicates argument {} is masked to {} bytes",
                        state.last_instruction.instruction,
                        state.last_instruction.opcode_details.clone().expect("impossible").name,
                        arg_index,
                        mask_size_bytes
                    );

                    frame.mask_size = mask_size_bytes;
                }
            }
        }

        // RETURN
        0xf3 => {
            // Safely convert U256 to usize
            let size: usize = state.last_instruction.inputs[1].try_into().unwrap_or(0);

            let return_memory_operations = function.get_memory_range(
                state.last_instruction.inputs[0],
                state.last_instruction.inputs[1],
            );
            let return_memory_operations_solidified = return_memory_operations
                .iter()
                .map(|x| x.operation.solidify())
                .collect::<Vec<String>>()
                .join(", ");

            // add the return statement to the function logic
            if analyzer_state.analyzer_type == AnalyzerType::Solidity {
                if return_memory_operations.len() <= 1 {
                    function.logic.push(format!("return {return_memory_operations_solidified};"));
                } else {
                    function.logic.push(format!(
                        "return abi.encodePacked({return_memory_operations_solidified});"
                    ));
                }
            } else if analyzer_state.analyzer_type == AnalyzerType::Yul {
                function.logic.push(format!(
                    "return({}, {})",
                    state.last_instruction.input_operations[0].yulify(),
                    state.last_instruction.input_operations[1].yulify()
                ));
            }

            // if we've already determined a return type, we don't want to do it again.
            // we use bytes32 as a default return type
            if function.returns.is_some() && function.returns.as_deref() != Some("bytes32") {
                return Ok(());
            }

            // if the any input op is ISZERO(x), this is a boolean return
            if return_memory_operations.iter().any(|x| x.operation.opcode.name == "ISZERO") {
                function.returns = Some(String::from("bool"));
            }
            // if the input op is any of the following, it is a uint256 return
            // this is because these push numeric values onto the stack
            else if return_memory_operations.iter().any(|x| {
                [0x31, 0x34, 0x3a, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48, 0x58, 0x5a]
                    .contains(&x.operation.opcode.code)
            }) {
                function.returns = Some(String::from("uint256"));
            }
            // if the input op is any of the following, it is an address return
            // this is because these push address values onto the stack
            else if return_memory_operations
                .iter()
                .any(|x| [0x30, 0x32, 0x33, 0x41].contains(&x.operation.opcode.code))
            {
                function.returns = Some(String::from("address"));
            }
            // if the size of returndata is > 32, it must be a bytes memory return.
            // it could be a struct, but we cant really determine that from the bytecode
            else if size > 32 {
                function.returns = Some(String::from("bytes memory"));
            } else {
                // attempt to find a return type within the return memory operations
                let byte_size = match AND_BITMASK_REGEX
                    .find(&return_memory_operations_solidified)
                    .ok()
                    .flatten()
                {
                    Some(bitmask) => {
                        let cast = bitmask.as_str();

                        cast.matches("ff").count()
                    }
                    None => match AND_BITMASK_REGEX_2
                        .find(&return_memory_operations_solidified)
                        .ok()
                        .flatten()
                    {
                        Some(bitmask) => {
                            let cast = bitmask.as_str();

                            cast.matches("ff").count()
                        }
                        None => 32,
                    },
                };

                // convert the cast size to a string
                let (_, cast_types) = byte_size_to_type(byte_size);
                function.returns = Some(cast_types[0].to_string());
            }

            debug!(
                "return type determined to be '{:?}' from ops '{}'",
                function.returns, return_memory_operations_solidified
            );
        }

        // integer type heuristics
        0x02 | 0x04 | 0x05 | 0x06 | 0x07 | 0x08 | 0x09 | 0x0b | 0x10 | 0x11 | 0x12 | 0x13 => {
            // check if this instruction is operating on a known argument.
            // if it is, add 'integer' to the list of heuristics
            // TODO: we probably want to use an enum for heuristics
            if let Some((arg_index, frame)) = function.arguments.iter_mut().find(|(_, frame)| {
                state
                    .last_instruction
                    .output_operations
                    .iter()
                    .any(|operation| operation.to_string().contains(frame.arg_op.as_str()))
            }) {
                debug!(
                    "instruction {} ({}) indicates argument {} may be a numeric type",
                    state.last_instruction.instruction,
                    state.last_instruction.opcode_details.clone().expect("impossible").name,
                    arg_index
                );

                frame.heuristics.insert(TypeHeuristic::Numeric);
            }
        }

        // bytes type heuristics
        0x18 | 0x1a | 0x1b | 0x1c | 0x1d | 0x20 => {
            // check if this instruction is operating on a known argument.
            // if it is, add 'bytes' to the list of heuristics
            // TODO: we probably want to use an enum for heuristics
            if let Some((arg_index, frame)) = function.arguments.iter_mut().find(|(_, frame)| {
                state
                    .last_instruction
                    .output_operations
                    .iter()
                    .any(|operation| operation.to_string().contains(frame.arg_op.as_str()))
            }) {
                debug!(
                    "instruction {} ({}) indicates argument {} may be a bytes type",
                    state.last_instruction.instruction,
                    state.last_instruction.opcode_details.clone().expect("impossible").name,
                    arg_index
                );

                frame.heuristics.insert(TypeHeuristic::Bytes);
            }
        }

        // boolean type heuristics
        0x15 => {
            // if this is a boolean check on CALLDATALOAD, we can add boolean to the potential types
            if let Some(calldataload_op) =
                state.last_instruction.input_operations.iter().find(|op| op.opcode.code == 0x35)
            {
                // yulify the calldataload operation, and find the associated argument index
                // this MUST exist, as we have already inserted it in the CALLDATALOAD heuristic
                let arg_op = calldataload_op.inputs[0].to_string();
                if let Some((arg_index, frame)) =
                    function.arguments.iter_mut().find(|(_, frame)| frame.arg_op == arg_op)
                {
                    debug!(
                        "instruction {} ({}) indicates argument {} may be a boolean",
                        state.last_instruction.instruction,
                        state.last_instruction.opcode_details.clone().expect("impossible").name,
                        arg_index
                    );

                    // NOTE: we don't want to update mask_size here, as we are only adding potential
                    // types
                    frame.heuristics.insert(TypeHeuristic::Boolean);
                }
            }
        }

        _ => {}
    };

    Ok(())
}
