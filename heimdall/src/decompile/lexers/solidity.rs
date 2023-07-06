use ethers::{
    abi::{decode, AbiEncode, ParamType},
    prelude::U256,
};
use heimdall_common::{
    ether::evm::{
        opcodes::WrappedOpcode,
        types::{byte_size_to_type, convert_bitmask},
    },
    io::logging::TraceFactory,
    utils::strings::{decode_hex, encode_hex_reduced},
};

use super::super::{constants::AND_BITMASK_REGEX, precompile::decode_precompile, util::*};
use crate::decompile::constants::VARIABLE_SIZE_CHECK_REGEX;

impl VMTrace {
    /// Converts a VMTrace to a Function through lexical and syntactic analysis
    ///
    /// ## Parameters
    /// - `self` - The VMTrace to be analyzed
    /// - `function` - The function to be updated with the analysis results
    /// - `trace` - The TraceFactory to be updated with the analysis results
    /// - `trace_parent` - The parent of the current VMTrace
    /// - `branch` - Branch metadata for the current trace. In the format of (branch_depth,
    ///   branch_index)
    ///     - @jon-becker: This will be used later to determin if a condition is a require
    ///
    ///
    /// ## Returns
    /// - `function` - The function updated with the analysis results
    pub fn analyze_sol(
        &self,
        function: Function,
        trace: &mut TraceFactory,
        trace_parent: u32,
        conditional_map: &mut Vec<String>,
        branch: (u32, u8),
    ) -> Function {
        // make a clone of the recursed analysis function
        let mut function = function;
        let mut jumped_conditional: Option<String> = None;

        // perform analysis on the operations of the current VMTrace branch
        for operation in &self.operations {
            let instruction = operation.last_instruction.clone();
            let _storage = operation.storage.clone();
            let memory = operation.memory.clone();

            let opcode_name = instruction.opcode_details.clone().unwrap().name;
            let opcode_number = instruction.opcode;

            // if the instruction is a state-accessing instruction, the function is no longer pure
            if function.pure &&
                vec![
                    "BALANCE",
                    "ORIGIN",
                    "CALLER",
                    "GASPRICE",
                    "EXTCODESIZE",
                    "EXTCODECOPY",
                    "BLOCKHASH",
                    "COINBASE",
                    "TIMESTAMP",
                    "NUMBER",
                    "DIFFICULTY",
                    "GASLIMIT",
                    "CHAINID",
                    "SELFBALANCE",
                    "BASEFEE",
                    "SLOAD",
                    "SSTORE",
                    "CREATE",
                    "SELFDESTRUCT",
                    "CALL",
                    "CALLCODE",
                    "DELEGATECALL",
                    "STATICCALL",
                    "CREATE2",
                ]
                .contains(&opcode_name)
            {
                function.pure = false;
                trace.add_info(
                    trace_parent,
                    instruction.instruction.try_into().unwrap(),
                    format!(
                        "instruction {} ({}) indicates an non-pure function.",
                        instruction.instruction, opcode_name
                    ),
                );
            }

            // if the instruction is a state-setting instruction, the function is no longer a view
            if function.view &&
                vec![
                    "SSTORE",
                    "CREATE",
                    "SELFDESTRUCT",
                    "CALL",
                    "CALLCODE",
                    "DELEGATECALL",
                    "STATICCALL",
                    "CREATE2",
                ]
                .contains(&opcode_name)
            {
                function.view = false;
                trace.add_info(
                    trace_parent,
                    instruction.instruction.try_into().unwrap(),
                    format!(
                        "instruction {} ({}) indicates a non-view function.",
                        instruction.instruction, opcode_name
                    ),
                );
            }

            if (0xA0..=0xA4).contains(&opcode_number) {
                // LOG0, LOG1, LOG2, LOG3, LOG4
                let logged_event = match operation.events.last() {
                    Some(event) => event,
                    None => {
                        function.notices.push(format!(
                            "unable to decode event emission at instruction {}",
                            instruction.instruction
                        ));
                        continue
                    }
                };

                // check to see if the event is a duplicate
                if !function
                    .events
                    .iter()
                    .any(|(selector, _)| selector == logged_event.topics.first().unwrap())
                {
                    // add the event to the function
                    function.events.insert(
                        *logged_event.topics.first().unwrap(),
                        (None, logged_event.clone()),
                    );

                    // decode the data field
                    let data_mem_ops =
                        function.get_memory_range(instruction.inputs[0], instruction.inputs[1]);
                    let data_mem_ops_solidified = data_mem_ops
                        .iter()
                        .map(|x| x.operations.solidify())
                        .collect::<Vec<String>>()
                        .join(", ");

                    // add the event emission to the function's logic
                    // will be decoded during post-processing
                    function.logic.push(format!(
                        "emit Event_{}({}{});",
                        &logged_event
                            .topics
                            .first()
                            .unwrap_or(&U256::from(0))
                            .encode_hex()
                            .replacen("0x", "", 1)[0..8],
                        match logged_event.topics.get(1..) {
                            Some(topics) => match !logged_event.data.is_empty() &&
                                !topics.is_empty()
                            {
                                true => {
                                    let mut solidified_topics: Vec<String> = Vec::new();
                                    for (i, _) in topics.iter().enumerate() {
                                        solidified_topics
                                            .push(instruction.input_operations[i + 3].solidify());
                                    }
                                    format!("{}, ", solidified_topics.join(", "))
                                }
                                false => {
                                    let mut solidified_topics: Vec<String> = Vec::new();
                                    for (i, _) in topics.iter().enumerate() {
                                        solidified_topics
                                            .push(instruction.input_operations[i + 3].solidify());
                                    }
                                    solidified_topics.join(", ")
                                }
                            },
                            None => "".to_string(),
                        },
                        data_mem_ops_solidified
                    ));
                }
            } else if opcode_name == "JUMPI" {
                // this is an if conditional for the children branches
                let conditional = instruction.input_operations[1].solidify();

                // remove non-payable check and mark function as non-payable
                if conditional == "!msg.value" {
                    // this is marking the start of a non-payable function
                    trace.add_info(
                        trace_parent,
                        instruction.instruction.try_into().unwrap(),
                        format!(
                            "conditional at instruction {} indicates an non-payble function.",
                            instruction.instruction
                        ),
                    );
                    function.payable = false;
                    continue
                }

                // perform a series of checks to determine if the condition
                // is added by the compiler and can be ignored
                if (conditional.contains("msg.data.length") && conditional.contains("0x04")) ||
                    VARIABLE_SIZE_CHECK_REGEX.is_match(&conditional).unwrap_or(false) ||
                    (conditional.replace('!', "") == "success")
                {
                    continue
                }

                function.logic.push(format!("if ({conditional}) {{").to_string());

                // save a copy of the conditional and add it to the conditional map
                jumped_conditional = Some(conditional.clone());
                conditional_map.push(conditional);
            } else if opcode_name == "REVERT" {
                // Safely convert U256 to usize
                let offset: usize = instruction.inputs[0].try_into().unwrap_or(0);
                let size: usize = instruction.inputs[1].try_into().unwrap_or(0);
                let revert_data = memory.read(offset, size);

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
                if revert_data.starts_with(&decode_hex("08c379a0").unwrap()) {
                    let revert_string = match revert_data.get(4..) {
                        Some(hex_data) => match decode(&[ParamType::String], hex_data) {
                            Ok(revert) => revert[0].to_string(),
                            Err(_) => "decoding error".to_string(),
                        },
                        None => "decoding error".to_string(),
                    };
                    revert_logic = match jumped_conditional.clone() {
                        Some(condition) => {
                            format!("require({condition}, \"{revert_string}\");")
                        }
                        None => {
                            // loop backwards through logic to find the last IF statement
                            for i in (0..function.logic.len()).rev() {
                                if function.logic[i].starts_with("if") {
                                    let conditional = match conditional_map.pop() {
                                        Some(condition) => condition,
                                        None => break,
                                    };

                                    function.logic[i] =
                                        format!("require({conditional}, \"{revert_string}\");");
                                }
                            }
                            continue
                        }
                    }
                }
                // handle case with panics
                else if revert_data.starts_with(&decode_hex("4e487b71").unwrap()) {
                    continue
                }
                // handle case with custom error OR empty revert
                else {
                    let custom_error_placeholder = match revert_data.get(0..4) {
                        Some(selector) => {
                            function.errors.insert(U256::from(selector), None);
                            format!(
                                "CustomError_{}()",
                                encode_hex_reduced(U256::from(selector)).replacen("0x", "", 1)
                            )
                        }
                        None => "()".to_string(),
                    };

                    revert_logic = match jumped_conditional.clone() {
                        Some(condition) => {
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
                                    let conditional = match conditional_map.pop() {
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
                            continue
                        }
                    }
                }

                function.logic.push(revert_logic);
            } else if opcode_name == "RETURN" {
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
                                    .unwrap()
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
                    function.logic.push(format!(
                        "return abi.encodePacked({return_memory_operations_solidified});"
                    ));
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
                let key = instruction.inputs[0];
                let value = instruction.inputs[1];
                let operations = instruction.input_operations[1].clone();

                // add the sstore to the function's storage map
                function.storage.insert(key, StorageFrame { value: value, operations: operations });
                function.logic.push(format!(
                    "storage[{}] = {};",
                    instruction.input_operations[0].solidify(),
                    instruction.input_operations[1].solidify(),
                ));
            } else if opcode_name.contains("MSTORE") {
                let key = instruction.inputs[0];
                let value = instruction.inputs[1];
                let operation = instruction.input_operations[1].clone();

                // add the mstore to the function's memory map
                function.memory.insert(key, StorageFrame { value: value, operations: operation });
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
                let modifier = match instruction.input_operations[0] !=
                    WrappedOpcode::new(0x5A, vec![])
                {
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
                let modifier = match instruction.input_operations[0] !=
                    WrappedOpcode::new(0x5A, vec![])
                {
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
                let gas = match instruction.input_operations[0] != WrappedOpcode::new(0x5A, vec![])
                {
                    true => format!("gas: {}, ", instruction.input_operations[0].solidify()),
                    false => String::from(""),
                };
                let value =
                    match instruction.input_operations[2] != WrappedOpcode::new(0x5A, vec![]) {
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
            } else if opcode_name == "CALLDATALOAD" {
                let slot_as_usize: usize = instruction.inputs[0].try_into().unwrap_or(usize::MAX);
                let calldata_slot = (slot_as_usize.saturating_sub(4)) / 32;
                match function.arguments.get(&calldata_slot) {
                    Some(_) => {}
                    None => {
                        function.arguments.insert(
                            calldata_slot,
                            (
                                CalldataFrame {
                                    slot: calldata_slot,
                                    operation: instruction.input_operations[0].to_string(),
                                    mask_size: 32,
                                    heuristics: Vec::new(),
                                },
                                vec![
                                    "bytes".to_string(),
                                    "uint256".to_string(),
                                    "int256".to_string(),
                                    "string".to_string(),
                                    "bytes32".to_string(),
                                    "uint".to_string(),
                                    "int".to_string(),
                                ],
                            ),
                        );
                    }
                }
            } else if opcode_name == "ISZERO" {
                if let Some(calldata_slot_operation) = instruction
                    .input_operations
                    .iter()
                    .find(|operation| operation.opcode.name == "CALLDATALOAD")
                {
                    if let Some((calldata_slot, arg)) =
                        function.arguments.clone().iter().find(|(_, (frame, _))| {
                            frame.operation == calldata_slot_operation.inputs[0].to_string()
                        })
                    {
                        // copy the current potential types to a new vector and remove duplicates
                        let mut potential_types = vec![
                            "bool".to_string(),
                            "bytes1".to_string(),
                            "uint8".to_string(),
                            "int8".to_string(),
                        ];
                        potential_types.append(&mut arg.1.clone());
                        potential_types.sort();
                        potential_types.dedup();

                        // replace mask size and potential types
                        function.arguments.insert(*calldata_slot, (arg.0.clone(), potential_types));
                    }
                };
            } else if ["AND", "OR"].contains(&opcode_name) {
                if let Some(calldata_slot_operation) =
                    instruction.input_operations.iter().find(|operation| {
                        operation.opcode.name == "CALLDATALOAD" ||
                            operation.opcode.name == "CALLDATACOPY"
                    })
                {
                    // convert the bitmask to it's potential solidity types
                    let (mask_size_bytes, mut potential_types) =
                        convert_bitmask(instruction.clone());

                    if let Some((calldata_slot, arg)) =
                        function.arguments.clone().iter().find(|(_, (frame, _))| {
                            frame.operation == calldata_slot_operation.inputs[0].to_string()
                        })
                    {
                        // append the current potential types to the new vector and remove
                        // duplicates
                        potential_types.append(&mut arg.1.clone());
                        potential_types.sort();
                        potential_types.dedup();

                        // replace mask size and potential types
                        function.arguments.insert(
                            *calldata_slot,
                            (
                                CalldataFrame {
                                    slot: arg.0.slot,
                                    operation: arg.0.operation.clone(),
                                    mask_size: mask_size_bytes,
                                    heuristics: Vec::new(),
                                },
                                potential_types,
                            ),
                        );
                    }
                };
            }

            // handle type heuristics
            if [
                "MUL",
                "MULMOD",
                "ADDMOD",
                "SMOD",
                "MOD",
                "DIV",
                "SDIV",
                "EXP",
                "LT",
                "GT",
                "SLT",
                "SGT",
                "SIGNEXTEND",
            ]
            .contains(&opcode_name)
            {
                // get the calldata slot operation
                if let Some((key, (frame, potential_types))) =
                    function.arguments.clone().iter().find(|(_, (frame, _))| {
                        instruction.output_operations.iter().any(|operation| {
                            operation.to_string().contains(frame.operation.as_str()) &&
                                !frame.heuristics.contains(&"integer".to_string())
                        })
                    })
                {
                    function.arguments.insert(
                        *key,
                        (
                            CalldataFrame {
                                slot: frame.slot,
                                operation: frame.operation.clone(),
                                mask_size: frame.mask_size,
                                heuristics: vec!["integer".to_string()],
                            },
                            potential_types.to_owned(),
                        ),
                    );
                }
            } else if ["SHR", "SHL", "SAR", "XOR", "BYTE"].contains(&opcode_name) {
                // get the calldata slot operation
                if let Some((key, (frame, potential_types))) =
                    function.arguments.clone().iter().find(|(_, (frame, _))| {
                        instruction.output_operations.iter().any(|operation| {
                            operation.to_string().contains(frame.operation.as_str()) &&
                                !frame.heuristics.contains(&"bytes".to_string())
                        })
                    })
                {
                    function.arguments.insert(
                        *key,
                        (
                            CalldataFrame {
                                slot: frame.slot,
                                operation: frame.operation.clone(),
                                mask_size: frame.mask_size,
                                heuristics: vec!["bytes".to_string()],
                            },
                            potential_types.to_owned(),
                        ),
                    );
                }
            }
        }

        // recurse into the children of the VMTrace map
        for (i, child) in self.children.iter().enumerate() {
            function = child.analyze_sol(
                function,
                trace,
                trace_parent,
                conditional_map,
                (branch.0 + 1, i as u8),
            );
        }

        // check if the ending brackets are needed
        if jumped_conditional.is_some() &&
            conditional_map.contains(&jumped_conditional.clone().unwrap())
        {
            // remove the conditional
            for (i, conditional) in conditional_map.iter().enumerate() {
                if conditional == &jumped_conditional.clone().unwrap() {
                    conditional_map.remove(i);
                    break
                }
            }

            function.logic.push("}".to_string());
        }

        function
    }
}
