
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
    utils::strings::{decode_hex, encode_hex_reduced, find_balanced_encapsulator},
};

use super::{super::util::*, super::precompile::decode_precompile, super::constants::AND_BITMASK_REGEX};

impl VMTrace {
    
    // converts a VMTrace to a Funciton through lexical and syntactic analysis
    pub fn analyze_yul(
        &self,
        function: Function,
        trace: &mut TraceFactory,
        trace_parent: u32,
        conditional_map: &mut Vec<String>
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
                        function.notices.push(format!("unable to decode event emission at instruction {}", instruction.instruction));
                        continue;
                    }
                };

                // check to see if the event is a duplicate
                if !function.events.iter().any(|(selector, _)| {
                    selector == logged_event.topics.first().unwrap()
                }) {
                    
                    // add the event to the function
                    function.events.insert(logged_event.topics.first().unwrap().to_string(), (None, logged_event.clone()));

                    // add the event emission to the function's logic
                    function.logic.push(format!(
                        "log{}({})",
                        opcode_number - 0xA0,
                        instruction.input_operations.iter().map(|input| input.yulify()).collect::<Vec<String>>().join(", ")
                    ));
                }

            } else if opcode_name == "JUMPI" {

                // this is an if conditional for the children branches
                let conditional = instruction.input_operations[1].yulify();

                function.logic.push(
                    format!(
                        "if {conditional} {{"
                    ).to_string()
                );
                jumped_conditional = Some(conditional.clone());
                conditional_map.push(conditional);

            } else if opcode_name == "REVERT" {

                // Safely convert U256 to usize
                let offset: usize = instruction.inputs[0].try_into().unwrap_or(0);
                let size: usize = instruction.inputs[1].try_into().unwrap_or(0);

                let revert_data = memory.read(offset, size);

                // (1) if revert_data starts with 0x08c379a0, the folling is an error string abiencoded
                // (2) if revert_data starts with 0x4e487b71, the following is a compiler panic
                // (3) if revert_data starts with any other 4byte selector, it is a custom error and should
                //     be resolved and added to the generated ABI
                // (4) if revert_data is empty, it is an empty revert. Ex:
                //       - if (true != false) { revert() };
                //       - require(true != false)

                // handle case with error string abiencoded
                if revert_data.starts_with("4e487b71") {
                    continue;
                }

                // handle case with custom error OR empty revert
                else {
                    for i in (0..function.logic.len()).rev() {
                        if function.logic[i].starts_with("if") {

                            // get matching conditional
                            let conditional = find_balanced_encapsulator(function.logic[i].to_string(), ('(', ')'));
                            let conditional = function.logic[i].get(conditional.0+1..conditional.1-1).unwrap_or("decoding error");
                            
                            // we can negate the conditional to get the revert logic
                            // TODO: make this a require statement, if revert is rlly gross but its technically correct
                            //       I just ran into issues with ending bracket matching
                            function.logic[i] = format!("if {conditional} {{ revert({}, {}); }}", instruction.input_operations[0].yulify(), instruction.input_operations[1].yulify());

                            conditional_map.remove(conditional_map.len() - 1);

                            break;
                        }
                    }
                }
            } else if opcode_name == "RETURN" {
                function.logic.push(format!("return({}, {})", instruction.input_operations[0].yulify(), instruction.input_operations[1].yulify()));

            } else if opcode_name == "SELDFESTRUCT" {

                let addr = match decode_hex(&instruction.inputs[0].encode_hex()) {
                    Ok(hex_data) => match decode(&[ParamType::Address], &hex_data) {
                        Ok(addr) => addr[0].to_string(),
                        Err(_) => "decoding error".to_string(),
                    },
                    _ => "".to_string(),
                };

                function.logic.push(format!("selfdestruct({addr})"));

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
                        "sstore({}, {})",
                        instruction.input_operations[0].yulify(),
                        instruction.input_operations[1].yulify(),
                    )
                );

            } else if opcode_name.contains("MSTORE") || opcode_name.contains("MSTORE8") {
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
                function.logic.push(format!("{}({}, {})", opcode_name.to_lowercase(), encode_hex_reduced(key), instruction.input_operations[1].yulify()));

            } else if [
                "STATICCALL",
                "CALL",
                "DELEGATECALL",
                "CALLCODE",
                "CREATE",
                "CREATE2",
                "CALLDATACOPY",
                "CODECOPY",
                "EXTCODECOPY",
                "RETURNDATACOPY",
            ].contains(&opcode_name) {
                function.logic.push(format!(
                    "{}({})",
                    opcode_name.to_lowercase(),
                    instruction.input_operations.iter().map(|x| x.yulify()).collect::<Vec<String>>().join(", ")
                ));
            } else if opcode_name == "CALLDATALOAD" {
                let calldata_slot = (instruction.inputs[0].as_usize().saturating_sub(4)) / 32;
                match function.arguments.get(&calldata_slot) {
                    Some(_) => {}
                    None => {
                        function.arguments.insert(
                            calldata_slot,
                            (
                                CalldataFrame {
                                    slot: (instruction.inputs[0].as_usize().saturating_sub(4)) / 32,
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
                if let Some(calldata_slot_operation) = instruction.input_operations.iter().find(|operation| {
                    operation.opcode.name == "CALLDATALOAD"
                }) {
                
                    if let Some((calldata_slot, arg)) = function.arguments.clone().iter().find(|(_, (frame, _))| {
                        frame.operation == calldata_slot_operation.inputs[0].to_string()
                    }) {
                        
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
                        function.arguments.insert(
                            *calldata_slot,
                            (
                                arg.0.clone(),
                                potential_types
                            ),
                        );
                    }
                };
            } else if ["AND", "OR"].contains(&opcode_name) {
                if let Some(calldata_slot_operation) = instruction.input_operations.iter().find(|operation| {
                    operation.opcode.name == "CALLDATALOAD" || operation.opcode.name == "CALLDATACOPY"
                }) {
                    
                    // convert the bitmask to it's potential solidity types
                    let (mask_size_bytes, mut potential_types) = convert_bitmask(instruction.clone());
                    
                    if let Some((calldata_slot, arg)) = function.arguments.clone().iter().find(|(_, (frame, _))| {
                        frame.operation == calldata_slot_operation.inputs[0].to_string()
                    }) {
                        
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
                ].contains(&opcode_name) {

                // get the calldata slot operation
                if let Some ((key, (frame, potential_types))) = function.arguments.clone().iter().find(|(_, (frame, _))| {
                    instruction.output_operations.iter().any(|operation| {
                        operation.to_string().contains(frame.operation.as_str()) &&
                        !frame.heuristics.contains(&"integer".to_string())
                    })
                }) {
                    function.arguments.insert(
                        *key,
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
                }
            } else if [
                "SHR",
                "SHL",
                "SAR",
                "XOR",
                "BYTE",
            ].contains(&opcode_name) {

                // get the calldata slot operation
                if let Some ((key, (frame, potential_types))) = function.arguments.clone().iter().find(|(_, (frame, _))| {
                    instruction.output_operations.iter().any(|operation| {
                        operation.to_string().contains(frame.operation.as_str()) &&
                        !frame.heuristics.contains(&"bytes".to_string())
                    })
                }) {
                    function.arguments.insert(
                        *key,
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
                }
            }

        }

        // recurse into the children of the VMTrace map
        for (_, child) in self.children.iter().enumerate() {

            function = child.analyze_yul(function, trace, trace_parent, conditional_map);

        }

        // check if the ending brackets are needed
        if jumped_conditional.is_some() && conditional_map.contains(&jumped_conditional.clone().unwrap())
        {
             // remove the conditional
             for (i, conditional) in conditional_map.iter().enumerate() {
                if conditional == &jumped_conditional.clone().unwrap() {
                    conditional_map.remove(i);
                    break;
                }
            }
            
            // if the last line is an if statement, this branch is empty and probably stack operations we don't care about
            if function.logic.last().unwrap().contains("if") {
                function.logic.pop();
            }
            else {
                function.logic.push("}".to_string());
            }
        }

        function
    }

}
