use crate::decompile::constants::AND_BITMASK_REGEX;

use super::util::{CalldataFrame, Snapshot, StorageFrame};
use ethers::{
    abi::{decode, ParamType},
    types::U256,
};
use heimdall_common::{
    ether::evm::{
        core::{
            opcodes::WrappedOpcode,
            types::{byte_size_to_type, convert_bitmask},
        },
        ext::exec::VMTrace,
    },
    io::logging::TraceFactory,
};

/// Generates a snapshot of a VMTrace's underlying function
///
/// ## Parameters
/// - `vm_trace` - The VMTrace to be analyzed
/// - `snapshot` - The snapshot to be updated with the analysis results
/// - `trace` - The TraceFactory to be updated with the analysis results
/// - `trace_parent` - The parent of the current VMTrace
///
/// ## Returns
/// - `snapshot` - The updated snapshot
pub fn snapshot_trace(
    vm_trace: &VMTrace,
    snapshot: Snapshot,
    trace: &mut TraceFactory,
    trace_parent: u32,
) -> Snapshot {
    // make a clone of the recursed analysis function
    let mut snapshot = snapshot;

    // update snapshot.gas (min, max, avg) with the value from vm_trace.gas_used
    snapshot.gas_used.min = snapshot.gas_used.min.min(vm_trace.gas_used);
    snapshot.gas_used.max = snapshot.gas_used.max.max(vm_trace.gas_used);
    snapshot.gas_used.avg = (snapshot.gas_used.avg + vm_trace.gas_used) / 2;

    // perform analysis on the operations of the current VMTrace branch
    for operation in &vm_trace.operations {
        let instruction = operation.last_instruction.clone();
        let _storage = operation.storage.clone();
        let memory = operation.memory.clone();

        let opcode_name = instruction.opcode_details.clone().unwrap().name;
        let opcode_number = instruction.opcode;

        // if the instruction is a state-accessing instruction, the function is no longer pure
        if snapshot.pure &&
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
            snapshot.pure = false;
            trace.add_info(
                trace_parent,
                instruction.instruction.try_into().unwrap(),
                &format!(
                    "instruction {} ({}) indicates an non-pure snapshot.",
                    instruction.instruction, opcode_name
                ),
            );
        }

        // if the instruction is a state-setting instruction, the function is no longer a view
        if snapshot.view &&
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
            snapshot.view = false;
            trace.add_info(
                trace_parent,
                instruction.instruction.try_into().unwrap(),
                &format!(
                    "instruction {} ({}) indicates a non-view snapshot.",
                    instruction.instruction, opcode_name
                ),
            );
        }

        if (0xA0..=0xA4).contains(&opcode_number) {
            // LOG0, LOG1, LOG2, LOG3, LOG4
            let logged_event = match operation.events.last() {
                Some(event) => event,
                None => continue,
            };

            // check to see if the event is a duplicate
            if !snapshot
                .events
                .iter()
                .any(|(selector, _)| selector == logged_event.topics.first().unwrap())
            {
                // add the event to the function
                snapshot
                    .events
                    .insert(*logged_event.topics.first().unwrap(), (None, logged_event.clone()));
            }
        } else if opcode_name == "JUMPI" {
            // this is an if conditional for the children branches
            let _conditional = instruction.input_operations[1].yulify();
            // TODO
        } else if opcode_name == "REVERT" {
            // Safely convert U256 to usize
            let offset: usize = instruction.inputs[0].try_into().unwrap_or(0);
            let size: usize = instruction.inputs[1].try_into().unwrap_or(0);
            let revert_data = memory.read(offset, size);

            if let Some(hex_data) = revert_data.get(4..) {
                if let Ok(reverts_with) = decode(&[ParamType::String], hex_data) {
                    if !reverts_with[0].to_string().is_empty() &&
                        reverts_with[0].to_string().chars().all(|c| c != '\0')
                    {
                        snapshot.strings.insert(reverts_with[0].to_string().to_owned());
                    }
                }
            }
        } else if opcode_name == "RETURN" {
            // Safely convert U256 to usize
            let offset: usize = instruction.inputs[0].try_into().unwrap_or(0);
            let size: usize = instruction.inputs[1].try_into().unwrap_or(0);
            let return_data = memory.read(offset, size);

            if let Some(hex_data) = return_data.get(4..) {
                if let Ok(returns) = decode(&[ParamType::String], hex_data) {
                    if !returns[0].to_string().is_empty() &&
                        returns[0].to_string().chars().all(|c| c != '\0')
                    {
                        snapshot.strings.insert(returns[0].to_string());
                    }
                }
            }

            let return_memory_operations =
                snapshot.get_memory_range(instruction.inputs[0], instruction.inputs[1]);
            let return_memory_operations_solidified = return_memory_operations
                .iter()
                .map(|x| x.operations.solidify())
                .collect::<Vec<String>>()
                .join(", ");

            // we don't want to overwrite the return value if it's already been set
            if snapshot.returns == Some(String::from("uint256")) || snapshot.returns.is_none() {
                // if the return operation == ISZERO, this is a boolean return
                if return_memory_operations.len() == 1 &&
                    return_memory_operations[0].operations.opcode.name == "ISZERO"
                {
                    snapshot.returns = Some(String::from("bool"));
                } else {
                    snapshot.returns = match size > 32 {
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
        } else if opcode_name == "SSTORE" {
            let key = instruction.inputs[0];
            let value = instruction.inputs[1];
            let operations = instruction.input_operations[1].clone();

            // add the sstore to the function's storage map
            snapshot.storage.insert(key, StorageFrame { value: value, operations: operations });
        } else if opcode_name == "CALLDATALOAD" {
            let slot_as_usize: usize = instruction.inputs[0].try_into().unwrap_or(usize::MAX);
            let calldata_slot = (slot_as_usize.saturating_sub(4)) / 32;
            match snapshot.arguments.get(&calldata_slot) {
                Some(_) => {}
                None => {
                    snapshot.arguments.insert(
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
                    snapshot.arguments.clone().iter().find(|(_, (frame, _))| {
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
                    snapshot.arguments.insert(*calldata_slot, (arg.0.clone(), potential_types));
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
                let (mask_size_bytes, mut potential_types) = convert_bitmask(instruction.clone());

                if let Some((calldata_slot, arg)) =
                    snapshot.arguments.clone().iter().find(|(_, (frame, _))| {
                        frame.operation == calldata_slot_operation.inputs[0].to_string()
                    })
                {
                    // append the current potential types to the new vector and remove
                    // duplicates
                    potential_types.append(&mut arg.1.clone());
                    potential_types.sort();
                    potential_types.dedup();

                    // replace mask size and potential types
                    snapshot.arguments.insert(
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
        } else if opcode_name.contains("MSTORE") {
            let key = instruction.inputs[0];
            let value = instruction.inputs[1];
            let operation = instruction.input_operations[1].clone();

            // add the mstore to the function's memory map
            snapshot.memory.insert(key, StorageFrame { value: value, operations: operation });
        } else if opcode_name == "CODECOPY" {
            let memory_offset = &instruction.inputs[0];
            let source_offset = instruction.inputs[1].try_into().unwrap_or(usize::MAX);
            let size_bytes = instruction.inputs[2].try_into().unwrap_or(usize::MAX);

            // get the code from the source offset and size
            let code = snapshot.bytecode[source_offset..(source_offset + size_bytes)].to_vec();

            // add the code to the function's memory map in chunks of 32 bytes
            for (index, chunk) in code.chunks(32).enumerate() {
                let key = memory_offset + (index * 32);
                let value = U256::from_big_endian(chunk);

                snapshot.memory.insert(
                    key,
                    StorageFrame { value: value, operations: WrappedOpcode::new(0x39, vec![]) },
                );
            }
        } else if opcode_name == "STATICCALL" {
            // if the gas param WrappedOpcode is not GAS(), add the gas param to the function's
            // logic
            let modifier = match instruction.input_operations[0] != WrappedOpcode::new(0x5A, vec![])
            {
                true => format!("{{ gas: {} }}", instruction.input_operations[0].solidify()),
                false => String::from(""),
            };

            let address = &instruction.input_operations[1];
            let extcalldata_memory =
                snapshot.get_memory_range(instruction.inputs[2], instruction.inputs[3]);

            snapshot.external_calls.push(format!(
                "address({}).staticcall{}({});",
                address.solidify(),
                modifier,
                extcalldata_memory
                    .iter()
                    .map(|x| x.operations.solidify())
                    .collect::<Vec<String>>()
                    .join(", "),
            ));
        } else if opcode_name == "DELEGATECALL" {
            // if the gas param WrappedOpcode is not GAS(), add the gas param to the function's
            // logic
            let modifier = match instruction.input_operations[0] != WrappedOpcode::new(0x5A, vec![])
            {
                true => format!("{{ gas: {} }}", instruction.input_operations[0].solidify()),
                false => String::from(""),
            };

            let address = &instruction.input_operations[1];
            let extcalldata_memory =
                snapshot.get_memory_range(instruction.inputs[2], instruction.inputs[3]);

            snapshot.external_calls.push(format!(
                "address({}).delegatecall{}({});",
                address.solidify(),
                modifier,
                extcalldata_memory
                    .iter()
                    .map(|x| x.operations.solidify())
                    .collect::<Vec<String>>()
                    .join(", "),
            ));
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
                snapshot.get_memory_range(instruction.inputs[3], instruction.inputs[4]);

            snapshot.external_calls.push(format!(
                "address({}).call{}({});",
                address.solidify(),
                modifier,
                extcalldata_memory
                    .iter()
                    .map(|x| x.operations.solidify())
                    .collect::<Vec<String>>()
                    .join(", ")
            ));
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
                snapshot.arguments.clone().iter().find(|(_, (frame, _))| {
                    instruction.output_operations.iter().any(|operation| {
                        operation.to_string().contains(frame.operation.as_str()) &&
                            !frame.heuristics.contains(&"integer".to_string())
                    })
                })
            {
                snapshot.arguments.insert(
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
                snapshot.arguments.clone().iter().find(|(_, (frame, _))| {
                    instruction.output_operations.iter().any(|operation| {
                        operation.to_string().contains(frame.operation.as_str()) &&
                            !frame.heuristics.contains(&"bytes".to_string())
                    })
                })
            {
                snapshot.arguments.insert(
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
    for (_, child) in vm_trace.children.iter().enumerate() {
        snapshot = snapshot_trace(child, snapshot, trace, trace_parent);
    }

    snapshot
}
