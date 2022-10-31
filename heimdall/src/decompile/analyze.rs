
use std::{str::FromStr};

use ethers::{
    abi::{decode, AbiEncode, ParamType},
    prelude::{
        U256,
    },
};
use heimdall_common::{
    ether::{
        evm::{
            opcodes::WrappedOpcode,
            types::{convert_bitmask, byte_size_to_type},
        },
    },
    io::logging::TraceFactory,
    utils::strings::{decode_hex, encode_hex_reduced},
};

use super::{util::*, precompile::decode_precompile, constants::AND_BITMASK_REGEX};

impl VMTrace {
    
    // converts a VMTrace to a Function which can be written to the decompiled output
    pub fn analyze(
        &self,
        function: Function,
        trace: &mut TraceFactory,
        trace_parent: u32,
    ) -> Function {

        // make a clone of the recursed analysis function
        let mut function = function.clone();

        // perform analysis on the operations of the current VMTrace branch
        for operation in &self.operations {
            let instruction = operation.last_instruction.clone();
            let _storage = operation.storage.clone();
            let memory = operation.memory.clone();

            let opcode_name = instruction.opcode_details.clone().unwrap().name;
            let opcode_number = U256::from_str(&instruction.opcode).unwrap().as_usize();

            // if the instruction is a state-accessing instruction, the function is no longer pure
            if function.pure
                && vec![
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
                .contains(&opcode_name.as_str())
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
            if function.view
                && vec![
                    "SSTORE",
                    "CREATE",
                    "SELFDESTRUCT",
                    "CALL",
                    "CALLCODE",
                    "DELEGATECALL",
                    "STATICCALL",
                    "CREATE2",
                ]
                .contains(&opcode_name.as_str())
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

            if opcode_number >= 0xA0 && opcode_number <= 0xA4 {

                // LOG0, LOG1, LOG2, LOG3, LOG4
                let logged_event = operation.events.last().unwrap().to_owned();

                // check to see if the event is a duplicate
                if !function.events.iter().any(|(selector, _)| {
                    selector == logged_event.topics.first().unwrap()
                }) {
                    // add the event to the function
                    function.events.insert(logged_event.topics.first().unwrap().to_string(), (None, logged_event.clone()));

                    // add the event emission to the function's logic
                    // will be decoded during post-processing
                    function.logic.push(format!(
                        "emit Event_{}({}{});",
                        
                        match &logged_event.topics.first() {
                            Some(topic) => topic.get(0..8).unwrap(),
                            None => "00000000",
                        },
                        match logged_event.topics.get(1..) {
                            Some(topics) => match logged_event.data.len() > 0 && topics.len() > 0 {
                                true => format!("{}, ", topics.join(", ")),
                                false => topics.join(", "),
                            },
                            None => "".to_string(),
                        },
                        logged_event.data
                    ));
                }

            } else if opcode_name == "JUMPI" {
            
                // add closing braces to the function's logic
                // TODO: add braces
                function.logic.push(
                    format!(
                        "if ({}) ",
                        
                        instruction.input_operations[1].solidify()
                    ).to_string()
                );

            } else if opcode_name == "REVERT" {

                // Safely convert U256 to usize
                let offset: usize = match instruction.inputs[0].try_into() {
                    Ok(x) => x,
                    Err(_) => 0,
                };
                let size: usize = match instruction.inputs[1].try_into() {
                    Ok(x) => x,
                    Err(_) => 0,
                };

                let revert_data = memory.read(offset, size);

                // (1) if revert_data starts with 0x08c379a0, the folling is an error string abiencoded
                // (2) if revert_data starts with any other 4byte selector, it is a custom error and should
                //     be resolved and added to the generated ABI
                // (3) if revert_data is empty, it is an empty revert. Ex:
                //       - if (true != false) { revert() };
                //       - require(true == false)

                let revert_logic;

                // handle case with error string abiencoded
                if revert_data.starts_with("08c379a0") {
                    let revert_string = match revert_data.get(8..) {
                        Some(data) => match decode_hex(data) {
                            Ok(hex_data) => match decode(&[ParamType::String], &hex_data) {
                                Ok(revert) => revert[0].to_string(),
                                Err(_) => "decoding error".to_string(),
                            },
                            Err(_) => "decoding error".to_string(),
                        },
                        None => "".to_string(),
                    };

                    revert_logic = format!("revert(\"{}\");", revert_string);
                }

                // handle case with custom error OR empty revert
                else {
                    let custom_error_placeholder = match revert_data.get(0..8) {
                        Some(selector) => {
                            function.errors.insert(selector.to_string(), None);
                            format!(" CustomError_{}()", selector)
                        },
                        None => "()".to_string(),
                    };
                    revert_logic = format!("revert{};", custom_error_placeholder);
                }

                function.logic.push(revert_logic);

            } else if opcode_name == "RETURN" {

                // Safely convert U256 to usize
                let size: usize = match instruction.inputs[1].try_into() {
                    Ok(x) => x,
                    Err(_) => 0,
                };
                
                let return_memory_operations = function.get_memory_range(instruction.inputs[0], instruction.inputs[1]);
                let return_memory_operations_solidified = return_memory_operations.iter().map(|x| x.operations.solidify()).collect::<Vec<String>>().join(" + ");

                // we don't want to overwrite the return value if it's already been set
                if function.returns == Some(String::from("uint256")) || function.returns == None {

                    // if the return operation == ISZERO, this is a boolean return
                    if return_memory_operations.len() == 1 && return_memory_operations[0].operations.opcode.name == "ISZERO" {
                        function.returns = Some(String::from("bool"));
                    }
                    else {
                        function.returns = match size > 32 {

                            // if the return data is > 32 bytes, we append "memory" to the return type
                            true => Some(format!("{} memory", "bytes")),
                            false => {
        
                                // attempt to find a return type within the return memory operations
                                let byte_size = match AND_BITMASK_REGEX.find(&return_memory_operations_solidified) {
                                    Some(bitmask) => {
                                        let cast = bitmask.as_str();

                                        cast.matches("ff").count()
                                    },
                                    None => 32
                                };
        
                                // convert the cast size to a string
                                let (_, cast_types) = byte_size_to_type(byte_size);
                                Some(format!("{}", cast_types[0]))
                            },
                        };
                    }
                }

                function.logic.push(format!("return({});", return_memory_operations_solidified));

            } else if opcode_name == "SELDFESTRUCT" {

                let addr = match decode_hex(&instruction.inputs[0].encode_hex()) {
                    Ok(hex_data) => match decode(&[ParamType::Address], &hex_data) {
                        Ok(addr) => addr[0].to_string(),
                        Err(_) => "decoding error".to_string(),
                    },
                    _ => "".to_string(),
                };

                function.logic.push(format!("selfdestruct({addr});", ));

            } else if opcode_name == "SSTORE" {
                let key = instruction.inputs[0];
                let value = instruction.inputs[1];
                let operations = instruction.input_operations[1].clone();

                // add the sstore to the function's storage map
                function.storage.insert(
                    key,
                    StorageFrame {
                        value: value,
                        operations: operations,
                    },
                );
                function.logic.push(
                    format!(
                        "storage[{}] = {};",
                        
                        instruction.input_operations[0].solidify(),
                        instruction.input_operations[1].solidify(),
                    )
                );

            } else if opcode_name.contains("MSTORE") {
                let key = instruction.inputs[0];
                let value = instruction.inputs[1];
                let operation = instruction.input_operations[1].clone();

                // add the mstore to the function's memory map
                function.memory.insert(
                    key,
                    StorageFrame {
                        value: value,
                        operations: operation,
                    },
                );
                function.logic.push(format!("memory[{}] = {};", encode_hex_reduced(key), instruction.input_operations[1].solidify()));

            } else if opcode_name == "STATICCALL" {

                // if the gas param WrappedOpcode is not GAS(), add the gas param to the function's logic
                let modifier =
                    match instruction.input_operations[0] != WrappedOpcode::new(0x5A, vec![]) {
                        true => format!("{{ gas: {} }}", instruction.input_operations[0].solidify()),
                        false => String::from(""),
                    };

                let address = &instruction.input_operations[1];
                let extcalldata_memory = function.get_memory_range(instruction.inputs[2], instruction.inputs[3]);

                // check if the external call is a precompiled contract
                match decode_precompile(
                    instruction.inputs[1],
                    extcalldata_memory.clone(),
                    instruction.input_operations[2].clone()
                ) {
                    (true, precompile_logic) => {
                        function.logic.push(precompile_logic);
                    },
                    _ => {
                        function.logic.push(format!(
                            "(bool success, bytes memory ret0) = address({}).staticcall{}({});",
                            
                            address.solidify(),
                            modifier,
                            extcalldata_memory.iter().map(|x| x.operations.solidify()).collect::<Vec<String>>().join(" + "),

                        ));
                    }
                }

            } else if opcode_name == "DELEGATECALL" {

                // if the gas param WrappedOpcode is not GAS(), add the gas param to the function's logic
                let modifier =
                    match instruction.input_operations[0] != WrappedOpcode::new(0x5A, vec![]) {
                        true => format!("{{ gas: {} }}", instruction.input_operations[0].solidify()),
                        false => String::from(""),
                    };

                let address = &instruction.input_operations[1];
                let extcalldata_memory = function.get_memory_range(instruction.inputs[2], instruction.inputs[3]);

                // check if the external call is a precompiled contract
                match decode_precompile(
                    instruction.inputs[1],
                    extcalldata_memory.clone(),
                    instruction.input_operations[2].clone()
                ) {
                    (true, precompile_logic) => {
                        function.logic.push(precompile_logic);
                    },
                    _ => {
                        function.logic.push(format!(
                            "(bool success, bytes memory ret0) = address({}).delegatecall{}({});",
                            
                            address.solidify(),
                            modifier,
                            extcalldata_memory.iter().map(|x| x.operations.solidify()).collect::<Vec<String>>().join(" + "),

                        ));
                    }
                }

            } else if opcode_name == "CALL" || opcode_name == "CALLCODE" {

                // if the gas param WrappedOpcode is not GAS(), add the gas param to the function's logic
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
                let modifier = match gas.len() > 0 || value.len() > 0 {
                    true => format!("{{ {}{} }}", gas, value),
                    false => String::from(""),
                };

                let address = &instruction.input_operations[1];
                let extcalldata_memory = function.get_memory_range(instruction.inputs[3], instruction.inputs[4]);

                // check if the external call is a precompiled contract
                match decode_precompile(
                    instruction.inputs[1],
                    extcalldata_memory.clone(),
                    instruction.input_operations[5].clone()
                ) {
                    (is_precompile, precompile_logic) if is_precompile=> {
                        function.logic.push(precompile_logic);
                    },
                    _ => {
                        function.logic.push(format!(
                            "(bool success, bytes memory ret0) = address({}).call{}({});",
                            
                            address.solidify(),
                            modifier,
                            extcalldata_memory.iter().map(|x| x.operations.solidify()).collect::<Vec<String>>().join(" + ")),
        
                        );
                    }
                }
                
            } else if opcode_name == "CREATE" {

                function.logic.push(
                    format!(
                        "assembly {{ addr := create({}, {}, {}) }}",
                        
                        instruction.input_operations[0].solidify(),
                        instruction.input_operations[1].solidify(),
                        instruction.input_operations[2].solidify(),
                    )
                );

            } else if opcode_name == "CREATE2" {

                function.logic.push(
                    format!(
                        "assembly {{ addr := create({}, {}, {}, {}) }}",
                        
                        instruction.input_operations[0].solidify(),
                        instruction.input_operations[1].solidify(),
                        instruction.input_operations[2].solidify(),
                        instruction.input_operations[3].solidify(),
                    )
                );
            } else if opcode_name == "CALLDATALOAD" {

                let calldata_slot = (instruction.inputs[0].as_usize() - 4) / 32;
                match function.arguments.get(&calldata_slot) {
                    Some(_) => {}
                    None => {
                        function.arguments.insert(
                            calldata_slot,
                            (
                                CalldataFrame {
                                    slot: (instruction.inputs[0].as_usize() - 4) / 32,
                                    operation: instruction.input_operations[0].to_string(),
                                    mask_size: 32,
                                    heuristics: Vec::new(),
                                },
                                vec!["bytes".to_string(),
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

                match instruction.input_operations.iter().find(|operation| {
                    operation.opcode.name == "CALLDATALOAD"
                }) {
                    Some(calldata_slot_operation) => {

                        match function.arguments.clone().iter().find(|(_, (frame, _))| {
                            frame.operation == calldata_slot_operation.inputs[0].to_string()
                        }) {
                            Some((calldata_slot, arg)) => {
    
                                // copy the current potential types to a new vector and remove duplicates
                                let mut potential_types = 
                                    vec![
                                        "bool".to_string(),
                                        "bytes1".to_string(),
                                        "uint8".to_string(),
                                        "int8".to_string(),
                                    ];
                                potential_types.append(&mut arg.1.clone());
                                potential_types.sort();
                                potential_types.dedup();
                                
                                // replace mask size and potential types
                                function.arguments.insert(
                                    *calldata_slot,
                                    (
                                        arg.0.clone(),
                                        potential_types
                                    ),
                                );
    
                            },
                            None => {}
                        }
                    },
                    None => {},
                };
            } else if ["AND", "OR"].contains(&opcode_name.as_str()) {

                match instruction.input_operations.iter().find(|operation| {
                    operation.opcode.name == "CALLDATALOAD" || operation.opcode.name == "CALLDATACOPY"
                }) {
                    Some(calldata_slot_operation) => {
                        
                        // convert the bitmask to it's potential solidity types
                        let (mask_size_bytes, mut potential_types) = convert_bitmask(instruction.clone());
                        
                        match function.arguments.clone().iter().find(|(_, (frame, _))| {
                            frame.operation == calldata_slot_operation.inputs[0].to_string()
                        }) {
                            Some((calldata_slot, arg)) => {
    
                                // append the current potential types to the new vector and remove duplicates
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
    
                            },
                            None => {}
                        }
                    }
                    None => {}
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
                ].contains(&opcode_name.as_str()) {

                // get the calldata slot operation
                match function.arguments.clone().iter().find(|(_, (frame, _))| {
                    instruction.output_operations.iter().any(|operation| {
                        operation.to_string().contains(frame.operation.as_str()) &&
                        !frame.heuristics.contains(&"integer".to_string())
                    })
                }) {
                   Some ((key, (frame, potential_types))) => {
                        function.arguments.insert(
                            key.clone(),
                            (
                                CalldataFrame {
                                    slot: frame.slot,
                                    operation: frame.operation.clone(),
                                    mask_size: frame.mask_size,
                                    heuristics: vec!["integer".to_string()],
                                },
                                potential_types.to_owned()
                            ),
                        );
                   },
                   None => {}
                }
            } else if [
                "SHR",
                "SHL",
                "SAR",
                "XOR",
                "BYTE",
            ].contains(&opcode_name.as_str()) {

                // get the calldata slot operation
                match function.arguments.clone().iter().find(|(_, (frame, _))| {
                    instruction.output_operations.iter().any(|operation| {
                        operation.to_string().contains(frame.operation.as_str()) &&
                        !frame.heuristics.contains(&"bytes".to_string())
                    })
                }) {
                    Some ((key, (frame, potential_types))) => {
                            function.arguments.insert(
                                key.clone(),
                                (
                                    CalldataFrame {
                                        slot: frame.slot,
                                        operation: frame.operation.clone(),
                                        mask_size: frame.mask_size,
                                        heuristics: vec!["bytes".to_string()],
                                    },
                                    potential_types.to_owned()
                                ),
                            );
                    },
                    None => {}
                }
            }

        }

        // recurse into the children of the VMTrace map
        function.logic.push("{".to_string());
        for child in &self.children {

            function = child.analyze(function, trace, trace_parent);

        }
        function.logic.push("}".to_string());

        // TODO: indentation

        function
    }

}
