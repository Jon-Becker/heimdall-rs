

use ethers::{
    abi::{decode, AbiEncode, ParamType},
    types::U256,
};
use eyre::eyre;
use heimdall_common::{
    ether::evm::core::{
        opcodes::{WrappedOpcode},
        types::{byte_size_to_type},
        vm::State,
    },
    utils::strings::{decode_hex, encode_hex_reduced},
};


use crate::{
    core::analyze::AnalyzerState,
    interfaces::{AnalyzedFunction, StorageFrame},
    utils::{
        constants::{AND_BITMASK_REGEX, VARIABLE_SIZE_CHECK_REGEX},
        precompile::decode_precompile,
    },
    Error,
};

pub fn solidity_heuristic(
    function: &mut AnalyzedFunction,
    state: &State,
    analyzer_state: &mut AnalyzerState,
) -> Result<(), Error> {
    let instruction = state.last_instruction.clone();
    let memory = state.memory.clone();

    let opcode_name =
        instruction.opcode_details.clone().ok_or(eyre!("opcode_details is None"))?.name;

    match instruction.opcode {
        // JUMPI
        0x57 => {
            let conditional = instruction.input_operations[1].solidify();

            // perform a series of checks to determine if the condition
            // is added by the compiler and can be ignored
            if (conditional.contains("msg.data.length") && conditional.contains("0x04")) ||
                VARIABLE_SIZE_CHECK_REGEX.is_match(&conditional).unwrap_or(false) ||
                (conditional.replace('!', "") == "success")
            {
                return Ok(());
            }

            function.logic.push(format!("if ({conditional}) {{").to_string());
            analyzer_state.jumped_conditional = Some(conditional.clone());
            analyzer_state.conditional_stack.push(conditional);
        }

        // REVERT
        0xfd => {
            let revert_data = memory.read(
                instruction.inputs[0].try_into().unwrap_or(0),
                instruction.inputs[1].try_into().unwrap_or(0),
            );
            let hex_data = revert_data.get(4..).unwrap_or(&[]);

            // find the cause of the revert
            let revert_condition = match analyzer_state.jumped_conditional.clone() {
                Some(conditional) => conditional,
                None => {
                    let mut conditional = "".to_string();
                    // loop backwards through logic to find the last IF statement
                    for i in (0..function.logic.len()).rev() {
                        if function.logic[i].starts_with("if") {
                            conditional =
                                analyzer_state.conditional_stack.pop().unwrap_or_default();
                            break;
                        }
                    }
                    conditional
                }
            };

            // handle string reverts
            if revert_data.starts_with(&[0x08, 0xc3, 0x79, 0xa0]) {
                let revert_string = decode(&[ParamType::String], hex_data)
                    .map(|x| x[0].to_string())
                    .unwrap_or("decoding error".to_string());
                function
                    .logic
                    .push(format!("require({revert_condition}, \"{revert_string}\");").to_string());
            }
            // handle custom errors and empty reverts
            else if !revert_data.starts_with(&[0x4e, 0x48, 0x7b, 0x71]) {
                let custom_error = revert_data
                    .get(0..4)
                    .map(|selector| {
                        function.errors.insert(U256::from(selector));
                        format!(
                            "CustomError_{}()",
                            encode_hex_reduced(U256::from(selector)).replacen("0x", "", 1)
                        )
                    })
                    .unwrap_or("UnknownError()".to_string());
                function
                    .logic
                    .push(format!("require({revert_condition}, {custom_error});").to_string());
            }
        }

        _ => {}
    };

    if opcode_name == "RETURN" {
        // Safely convert U256 to usize
        let size: usize = instruction.inputs[1].try_into().unwrap_or(0);

        let return_memory_operations =
            function.get_memory_range(instruction.inputs[0], instruction.inputs[1]);
        let return_memory_operations_solidified = return_memory_operations
            .iter()
            .map(|x| x.operations.solidify())
            .collect::<Vec<String>>()
            .join(", ");

        // we don't want to overwrite the return value if it's already been set
        if function.returns == Some(String::from("uint256")) || function.returns.is_none() {
            // if the return operation == ISZERO, this is a boolean return
            if return_memory_operations.len() == 1 &&
                return_memory_operations[0].operations.opcode.name == "ISZERO"
            {
                function.returns = Some(String::from("bool"));
            } else {
                function.returns = match size > 32 {
                    // if the return data is > 32 bytes, we append "memory" to the return
                    // type
                    true => Some(format!("{} memory", "bytes")),
                    false => {
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
                            None => 32,
                        };

                        // convert the cast size to a string
                        let (_, cast_types) = byte_size_to_type(byte_size);
                        Some(cast_types[0].to_string())
                    }
                };
            }
        }
        if return_memory_operations.len() <= 1 {
            function.logic.push(format!("return {return_memory_operations_solidified};"));
        } else {
            function
                .logic
                .push(format!("return abi.encodePacked({return_memory_operations_solidified});"));
        }
    } else if opcode_name == "SELDFESTRUCT" {
        let addr = match decode_hex(&instruction.inputs[0].encode_hex()) {
            Ok(hex_data) => match decode(&[ParamType::Address], &hex_data) {
                Ok(addr) => addr[0].to_string(),
                Err(_) => "decoding error".to_string(),
            },
            _ => "".to_string(),
        };

        function.logic.push(format!("selfdestruct({addr});"));
    } else if opcode_name == "SSTORE" {
        function.logic.push(format!(
            "storage[{}] = {};",
            instruction.input_operations[0].solidify(),
            instruction.input_operations[1].solidify(),
        ));
    } else if opcode_name == "TSTORE" {
        function.logic.push(format!(
            "transient[{}] = {};",
            instruction.input_operations[0].solidify(),
            instruction.input_operations[1].solidify()
        ));
    } else if opcode_name.contains("MSTORE") {
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
    } else if opcode_name == "CALLDATACOPY" {
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
    } else if opcode_name == "CODECOPY" {
        let memory_offset = &instruction.input_operations[0];
        let source_offset = instruction.inputs[1];
        let size_bytes = instruction.inputs[2];

        // add the mstore to the function's memory map
        function.logic.push(format!(
            "memory[{}] = this.code[{}:{}]",
            memory_offset.solidify(),
            source_offset,
            source_offset.saturating_add(size_bytes)
        ));
    } else if opcode_name == "EXTCODECOPY" {
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
    } else if opcode_name == "STATICCALL" {
        // if the gas param WrappedOpcode is not GAS(), add the gas param to the function's
        // logic
        let modifier = match instruction.input_operations[0] != WrappedOpcode::new(0x5A, vec![]) {
            true => format!("{{ gas: {} }}", instruction.input_operations[0].solidify()),
            false => String::from(""),
        };

        let address = &instruction.input_operations[1];
        let extcalldata_memory =
            function.get_memory_range(instruction.inputs[2], instruction.inputs[3]);

        // check if the external call is a precompiled contract
        match decode_precompile(
            instruction.inputs[1],
            extcalldata_memory.clone(),
            instruction.input_operations[2].clone(),
        ) {
            (true, precompile_logic) => {
                function.logic.push(precompile_logic);
            }
            _ => {
                function.logic.push(format!(
                    "(bool success, bytes memory ret0) = address({}).staticcall{}(abi.encode({}));",
                    address.solidify(),
                    modifier,
                    extcalldata_memory
                        .iter()
                        .map(|x| x.operations.solidify())
                        .collect::<Vec<String>>()
                        .join(", "),
                ));
            }
        }
    } else if opcode_name == "DELEGATECALL" {
        // if the gas param WrappedOpcode is not GAS(), add the gas param to the function's
        // logic
        let modifier = match instruction.input_operations[0] != WrappedOpcode::new(0x5A, vec![]) {
            true => format!("{{ gas: {} }}", instruction.input_operations[0].solidify()),
            false => String::from(""),
        };

        let address = &instruction.input_operations[1];
        let extcalldata_memory =
            function.get_memory_range(instruction.inputs[2], instruction.inputs[3]);

        // check if the external call is a precompiled contract
        match decode_precompile(
            instruction.inputs[1],
            extcalldata_memory.clone(),
            instruction.input_operations[2].clone(),
        ) {
            (true, precompile_logic) => {
                function.logic.push(precompile_logic);
            }
            _ => {
                function.logic.push(format!(
                    "(bool success, bytes memory ret0) = address({}).delegatecall{}(abi.encode({}));",
                    address.solidify(),
                    modifier,
                    extcalldata_memory
                        .iter()
                        .map(|x| x.operations.solidify())
                        .collect::<Vec<String>>()
                        .join(", "),
                ));
            }
        }
    } else if opcode_name == "CALL" || opcode_name == "CALLCODE" {
        // if the gas param WrappedOpcode is not GAS(), add the gas param to the function's
        // logic
        let gas = match instruction.input_operations[0] != WrappedOpcode::new(0x5A, vec![]) {
            true => format!("gas: {}, ", instruction.input_operations[0].solidify()),
            false => String::from(""),
        };
        let value = match instruction.input_operations[2] != WrappedOpcode::new(0x5A, vec![]) {
            true => format!("value: {}", instruction.input_operations[2].solidify()),
            false => String::from(""),
        };
        let modifier = match !gas.is_empty() || !value.is_empty() {
            true => format!("{{ {gas}{value} }}"),
            false => String::from(""),
        };

        let address = &instruction.input_operations[1];
        let extcalldata_memory =
            function.get_memory_range(instruction.inputs[3], instruction.inputs[4]);

        // check if the external call is a precompiled contract
        match decode_precompile(
            instruction.inputs[1],
            extcalldata_memory.clone(),
            instruction.input_operations[5].clone(),
        ) {
            (is_precompile, precompile_logic) if is_precompile => {
                function.logic.push(precompile_logic);
            }
            _ => {
                function.logic.push(format!(
                    "(bool success, bytes memory ret0) = address({}).call{}(abi.encode({}));",
                    address.solidify(),
                    modifier,
                    extcalldata_memory
                        .iter()
                        .map(|x| x.operations.solidify())
                        .collect::<Vec<String>>()
                        .join(", ")
                ));
            }
        }
    } else if opcode_name == "CREATE" {
        function.logic.push(format!(
            "assembly {{ addr := create({}, {}, {}) }}",
            instruction.input_operations[0].solidify(),
            instruction.input_operations[1].solidify(),
            instruction.input_operations[2].solidify(),
        ));
    } else if opcode_name == "CREATE2" {
        function.logic.push(format!(
            "assembly {{ addr := create({}, {}, {}, {}) }}",
            instruction.input_operations[0].solidify(),
            instruction.input_operations[1].solidify(),
            instruction.input_operations[2].solidify(),
            instruction.input_operations[3].solidify(),
        ));
    }

    Ok(())
}
