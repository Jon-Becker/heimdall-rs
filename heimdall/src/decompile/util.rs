use std::{collections::HashMap, str::FromStr};

use ethers::{
    abi::{decode, AbiEncode, ParamType},
    prelude::{
        rand::{self, Rng},
        U256,
    },
};
use heimdall_common::{
    ether::{
        evm::{
            log::Log,
            opcodes::WrappedOpcode,
            vm::{State, VM}, types::convert_bitmask,
        },
        signatures::{resolve_signature, ResolvedFunction},
    },
    io::logging::TraceFactory,
    utils::strings::decode_hex,
};

#[derive(Clone, Debug)]
pub struct Function {
    // the function's 4byte selector
    pub selector: String,

    // the function's entry point in the code.
    // the entry point is the instruction the dispatcher JUMPs to when called.
    pub entry_point: u64,

    // argument structure:
    //   - key : slot of the argument. I.E: slot 0 is CALLDATALOAD(4).
    //   - value : tuple of ({value: U256, input_operation: WrappedOpcode}, potential_type)
    pub arguments: HashMap<U256, (CalldataFrame, String)>,

    // storage structure:
    //   - key : slot of the argument. I.E: slot 0 is CALLDATALOAD(4).
    //   - value : tuple of ({value: U256, operation: WrappedOpcode})
    pub storage: HashMap<U256, StorageFrame>,

    // memory structure:
    //   - key : slot of the argument. I.E: slot 0 is CALLDATALOAD(4).
    //   - value : tuple of ({value: U256, operation: WrappedOpcode})
    pub memory: HashMap<U256, StorageFrame>,

    // returns the return type for the function.
    pub returns: Option<String>,

    // holds function logic to be written to the output solidity file.
    pub logic: Vec<String>,

    // holds all emitted events. used to generate solidity event definitions
    // as well as ABI specifications.
    pub events: Vec<Log>,

    // modifiers
    pub pure: bool,
    pub view: bool,
    pub payable: bool,
    pub constant: bool,
    pub external: bool,
}

#[derive(Clone, Debug)]
pub struct StorageFrame {
    pub value: U256,
    pub operations: WrappedOpcode,
}

#[derive(Clone, Debug)]
pub struct CalldataFrame {
    pub value: String,
    pub input_operation: WrappedOpcode,
}

impl Function {
    // format and return the function's logic
}

#[derive(Clone, Debug)]
pub struct VMTrace {
    pub instruction: u128,
    pub operations: Vec<State>,
    pub children: Vec<VMTrace>,
    pub depth: usize,
}

// Find all function selectors in the given EVM.
pub fn find_function_selectors(evm: &VM, assembly: String) -> Vec<String> {
    let mut function_selectors = Vec::new();

    let mut vm = evm.clone();

    // find a selector not present in the assembly
    let selector;
    loop {
        let num = rand::thread_rng().gen_range(286331153..2147483647);
        if !vm
            .bytecode
            .contains(&format!("63{}", num.encode_hex()[58..].to_string()))
        {
            selector = num.encode_hex()[58..].to_string();
            break;
        }
    }

    // execute the EVM call to find the dispatcher revert
    let dispatcher_revert = vm.call(selector, 0).instruction - 1;

    // search through assembly for PUSH4 instructions up until the dispatcher revert
    let assembly: Vec<String> = assembly
        .split("\n")
        .map(|line| line.trim().to_string())
        .collect();
    for line in assembly.iter() {
        let instruction_args: Vec<String> = line.split(" ").map(|arg| arg.to_string()).collect();
        let program_counter: u128 = instruction_args[0].clone().parse().unwrap();
        let instruction = instruction_args[1].clone();

        if program_counter < dispatcher_revert {
            if instruction == "PUSH4" {
                let function_selector = instruction_args[2].clone();
                function_selectors.push(function_selector);
            }
        } else {
            break;
        }
    }
    function_selectors.sort();
    function_selectors.dedup();
    function_selectors
}

// resolve a list of function selectors to their possible signatures
pub fn resolve_function_selectors(
    selectors: Vec<String>,
) -> HashMap<String, Vec<ResolvedFunction>> {
    let mut resolved_functions: HashMap<String, Vec<ResolvedFunction>> = HashMap::new();

    for selector in selectors {
        match resolve_signature(&selector) {
            Some(function) => {
                resolved_functions.insert(selector, function);
            }
            None => continue,
        }
    }

    resolved_functions
}

// resolve a selector's function entry point from the EVM bytecode
pub fn resolve_entry_point(evm: &VM, selector: String) -> u64 {
    let mut vm = evm.clone();
    let mut flag_next_jumpi = false;
    let mut function_entry_point = 0;

    // execute the EVM call to find the entry point for the given selector
    vm.calldata = selector.clone();
    while vm.bytecode.len() >= (vm.instruction * 2 + 2) as usize {
        let call = vm.step();

        // if the opcode is an EQ and it matched the selector, the next jumpi is the entry point
        if call.last_instruction.opcode == "14"
            && call.last_instruction.inputs[0].eq(&U256::from_str(&selector.clone()).unwrap())
            && call.last_instruction.outputs[0].eq(&U256::from_str("1").unwrap())
        {
            flag_next_jumpi = true;
        }

        // if we are flagging the next jumpi, and the opcode is a JUMPI, we have found the entry point
        if flag_next_jumpi && call.last_instruction.opcode == "57" {
            // it's safe to convert here because we know max bytecode length is ~25kb, way less than 2^64
            function_entry_point = call.last_instruction.inputs[0].as_u64();
            break;
        }

        if vm.exitcode != 255 || vm.returndata.len() as usize > 0 {
            break;
        }
    }

    function_entry_point
}

// build a map of function jump possibilities from the EVM bytecode
pub fn map_selector(
    evm: &VM,
    trace: &TraceFactory,
    trace_parent: u32,
    selector: String,
    entry_point: u64,
) -> (VMTrace, Vec<u128>) {
    let mut vm = evm.clone();
    vm.calldata = selector.clone();

    // step through the bytecode until we reach the entry point
    while (vm.bytecode.len() >= (vm.instruction * 2 + 2) as usize)
        && (vm.instruction <= entry_point.into())
    {
        vm.step();

        // this shouldn't be necessary, but it's safer to have it
        if vm.exitcode != 255 || vm.returndata.len() as usize > 0 {
            break;
        }
    }

    // the VM is at the function entry point, begin tracing
    let mut handled_jumpdests = Vec::new();
    (
        recursive_map(&vm.clone(), trace, trace_parent, &mut handled_jumpdests),
        handled_jumpdests,
    )
}

pub fn recursive_map(
    evm: &VM,
    trace: &TraceFactory,
    trace_parent: u32,
    handled_jumpdests: &mut Vec<u128>,
) -> VMTrace {
    let mut vm = evm.clone();

    // create a new VMTrace object
    let mut vm_trace = VMTrace {
        instruction: vm.instruction,
        operations: Vec::new(),
        children: Vec::new(),
        depth: 0,
    };

    // step through the bytecode until we find a JUMPI instruction
    while vm.bytecode.len() >= (vm.instruction * 2 + 2) as usize {
        let state = vm.step();
        vm_trace.operations.push(state.clone());

        // if we encounter a JUMPI, create children taking both paths and break
        if state.last_instruction.opcode == "57" {
            vm_trace.depth += 1;

            // we need to create a trace for the path that wasn't taken.
            if state.last_instruction.inputs[1] == U256::from(0) {

                // the jump was not taken, create a trace for the jump path
                // only jump if we haven't already traced this destination
                // TODO: mark as a loop?
                if !(handled_jumpdests.contains(&(state.last_instruction.inputs[0].as_u128() + 1)))
                {
                    let mut trace_vm = vm.clone();
                    trace_vm.instruction = state.last_instruction.inputs[0].as_u128() + 1;
                    handled_jumpdests.push(trace_vm.instruction.clone());
                    vm_trace.children.push(recursive_map(
                        &trace_vm,
                        trace,
                        trace_parent,
                        handled_jumpdests,
                    ));
                } else {
                    break;
                }

                // push the current path onto the stack
                vm_trace.children.push(recursive_map(
                    &vm.clone(),
                    trace,
                    trace_parent,
                    handled_jumpdests,
                ));
            } else {

                // the jump was taken, create a trace for the fallthrough path
                // only jump if we haven't already traced this destination
                if !(handled_jumpdests.contains(&(state.last_instruction.instruction + 1))) {
                    let mut trace_vm = vm.clone();
                    trace_vm.instruction = state.last_instruction.instruction + 1;
                    handled_jumpdests.push(trace_vm.instruction.clone());
                    vm_trace.children.push(recursive_map(
                        &trace_vm,
                        trace,
                        trace_parent,
                        handled_jumpdests,
                    ));
                } else {
                    break;
                }

                // push the current path onto the stack
                vm_trace.children.push(recursive_map(
                    &vm.clone(),
                    trace,
                    trace_parent,
                    handled_jumpdests,
                ));
            }
        }

        if vm.exitcode != 255 || vm.returndata.len() as usize > 0 {
            break;
        }
    }

    vm_trace
}

impl VMTrace {
    
    // converts a VMTrace to a Funciton
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
                if !function.events.iter().any(|log| {
                    log.index == logged_event.index
                        && log.topics.first().unwrap() == logged_event.topics.first().unwrap()
                }) {
                    // add the event to the function
                    function.events.push(logged_event.clone());

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

                //println!("{}", instruction.input_operations.get(1).unwrap());

                // add closing braces to the function's logic
                //function.logic.push("if (true) {".to_string());

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
                        Some(selector) => format!(" CustomError_{}", selector),
                        None => "()".to_string(),
                    };
                    revert_logic = format!("revert{};", custom_error_placeholder);
                }

                function.logic.push(revert_logic);

            } else if opcode_name == "RETURN" {

                // Safely convert U256 to usize
                let offset: usize = match instruction.inputs[0].try_into() {
                    Ok(x) => x,
                    Err(_) => 0,
                };
                let size: usize = match instruction.inputs[1].try_into() {
                    Ok(x) => x,
                    Err(_) => 0,
                };
                let _return_data_raw = memory.read(offset, size);

                // TODO: push return type to function.returns

                function.logic.push(format!("return(memory[{}]);", offset,));

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
                function.storage.insert(
                    key,
                    StorageFrame {
                        value: value,
                        operations: operations,
                    },
                );
                function.logic.push(format!("storage[{}] = {};", key, value));

            } else if opcode_name.contains("MSTORE") {
                let key = instruction.inputs[0];
                let value = instruction.inputs[1];
                let operation = instruction.input_operations[1].clone();

                // add the mstore to the function's memory map
                function.storage.insert(
                    key,
                    StorageFrame {
                        value: value,
                        operations: operation.clone(),
                    },
                );
                function.logic.push(format!("memory[{}] = {};", key, operation));

            } else if opcode_name == "STATICCALL" {

                // if the gas param WrappedOpcode is not GAS(), add the gas param to the function's logic
                let modifier =
                    match instruction.input_operations[0] != WrappedOpcode::new(0x5A, vec![]) {
                        true => format!("{{ gas: {} }}", instruction.input_operations[0]),
                        false => String::from(""),
                    };

                let address = instruction.input_operations[1].clone();
                let data_memory_offset = instruction.inputs[2].clone();

                function.logic.push(format!(
                    "(bool success, bytes ret0) = address({}).staticcall{}(memory[{}]);",
                    address, modifier, data_memory_offset
                ));

            } else if opcode_name == "DELEGATECALL" {

                // if the gas param WrappedOpcode is not GAS(), add the gas param to the function's logic
                let modifier =
                    match instruction.input_operations[0] != WrappedOpcode::new(0x5A, vec![]) {
                        true => format!("{{ gas: {} }}", instruction.input_operations[0]),
                        false => String::from(""),
                    };

                let address = instruction.input_operations[1].clone();
                let data_memory_offset = instruction.inputs[2].clone();

                function.logic.push(format!(
                    "(bool success, bytes ret0) = address({}).delegatecall{}(memory[{}]);",
                    address, modifier, data_memory_offset
                ));

            } else if opcode_name == "CALL" || opcode_name == "CALLCODE" {

                // if the gas param WrappedOpcode is not GAS(), add the gas param to the function's logic
                let gas = match instruction.input_operations[0] != WrappedOpcode::new(0x5A, vec![])
                {
                    true => format!("gas: {}", instruction.input_operations[0]),
                    false => String::from(""),
                };
                let value =
                    match instruction.input_operations[2] != WrappedOpcode::new(0x5A, vec![]) {
                        true => format!("gas: {}", instruction.input_operations[2]),
                        false => String::from(""),
                    };
                let modifier = match gas.len() > 0 || value.len() > 0 {
                    true => format!("{{ {}, {} }}", gas, value),
                    false => String::from(""),
                };

                let address = instruction.input_operations[1].clone();
                let data_memory_offset = instruction.inputs[3].clone();

                function.logic.push(format!(
                    "(bool success, bytes ret0) = address({}).call{}(memory[{}]);",
                    address, modifier, data_memory_offset
                ));

            } else if opcode_name == "CREATE" {

                function.logic.extend(vec![
                    "".to_string(),
                    "assembly {".to_string(),
                    format!(
                        "addr := create({}, {}, {})",
                        instruction.input_operations[0].clone(),
                        instruction.input_operations[1].clone(),
                        instruction.input_operations[2].clone(),
                    ),
                    "}".to_string(),
                    "".to_string(),
                ]);

            } else if opcode_name == "CREATE2" {

                function.logic.extend(vec![
                    "".to_string(),
                    "assembly {".to_string(),
                    format!(
                        "addr := create2({}, {}, {}, {})",
                        instruction.input_operations[0].clone(),
                        instruction.input_operations[1].clone(),
                        instruction.input_operations[2].clone(),
                        instruction.input_operations[3].clone(),
                    ),
                    "}".to_string(),
                    "".to_string(),
                ]);
            } else if ["SHL", "SHR", "AND", "OR"].contains(&opcode_name.as_str()) {
                if instruction.input_operations.iter().any(|operation| {
                    operation.opcode.name == "CALLDATALOAD" || operation.opcode.name == "CALLDATACOPY"
                }) {

                    // convert the bitmask to it's potential solidity types
                    let _potential_types = convert_bitmask(instruction.clone());
                }
            }

        }

        // recurse into the children of the VMTrace map
        for child in &self.children {
            function = child.analyze(function, trace, trace_parent);
            //function.logic.push("}".to_string());
        }

        function
    }
}
