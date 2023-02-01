use std::{str::FromStr, collections::{HashMap, VecDeque}};

use ethers::{
    prelude::{
        U256,
    },
};
use heimdall_common::{
    ether::{
        evm::{
            vm::{State, VM}, stack::StackFrame
        },
    },
};

#[derive(Clone, Debug)]
pub struct VMTrace {
    pub instruction: u128,
    pub operations: Vec<State>,
    pub children: Vec<VMTrace>,
    pub loop_detected: bool
}

// returns the compiler version used to compile the contract.
// for example: (solc, 0.8.10) or (vyper, 0.2.16)
pub fn detect_compiler(bytecode: String) -> (String, String) {
    
    let mut compiler = "unknown".to_string();
    let mut version = "unknown".to_string();

    // perfom prefix check for rough version matching
    if bytecode.starts_with("363d3d373d3d3d363d73") {
        compiler = "proxy".to_string();
        version = "minimal".to_string();
    }
    else if bytecode.starts_with("366000600037611000600036600073") {
        compiler = "proxy".to_string();
        version = "vyper".to_string();
    }
    else if bytecode.starts_with("6004361015") {
        compiler = "vyper".to_string();
        version = "0.2.0-0.2.4,0.2.11-0.3.3".to_string();
    }
    else if bytecode.starts_with("341561000a") {
        compiler = "vyper".to_string();
        version = "0.2.5-0.2.8".to_string();
    }
    else if bytecode.starts_with("731bf797") {
        compiler = "solc".to_string();
        version = "0.4.10-0.4.24".to_string();
    }
    else if bytecode.starts_with("6080604052") {
        compiler = "solc".to_string();
        version = "0.4.22+".to_string();
    }
    else if bytecode.starts_with("6060604052") {
        compiler = "solc".to_string();
        version = "0.4.11-0.4.21".to_string();
    }
    else if bytecode.contains("7679706572") {
        compiler = "vyper".to_string();
    }
    else if bytecode.contains("736f6c63") {
        compiler = "solc".to_string();
    }

    // perform metadata check
    if compiler == "solc" {
        let compiler_version = bytecode.split("736f6c6343").collect::<Vec<&str>>();
        
        if compiler_version.len() > 1 {
            match compiler_version[1].get(0..6) {
                Some(encoded_version) => {
                    let version_array = encoded_version.chars()
                        .collect::<Vec<char>>()
                        .chunks(2)
                        .map(|c| c.iter().collect::<String>())
                        .collect::<Vec<String>>();

                    version = String::new();
                    for version_part in version_array {
                        version.push_str(&format!("{}.", u8::from_str_radix(&version_part, 16).unwrap()));
                    }
                },
                None => {},
            }
        }
    }
    else if compiler == "vyper" {
        let compiler_version = bytecode.split("767970657283").collect::<Vec<&str>>();
        
        if compiler_version.len() > 1 {
            match compiler_version[1].get(0..6) {
                Some(encoded_version) => {
                    let version_array = encoded_version.chars()
                        .collect::<Vec<char>>()
                        .chunks(2)
                        .map(|c| c.iter().collect::<String>())
                        .collect::<Vec<String>>();

                    version = String::new();
                    for version_part in version_array {
                        version.push_str(&format!("{}.", u8::from_str_radix(&version_part, 16).unwrap()));
                    }
                },
                None => {},
            }
        }
    }


    (compiler, version.trim_end_matches('.').to_string())
}

// find all function selectors in the given EVM.
pub fn find_function_selectors(assembly: String) -> Vec<String> {
    let mut function_selectors = Vec::new();

    // search through assembly for PUSH4 instructions, optimistically assuming that they are function selectors
    let assembly: Vec<String> = assembly
        .split('\n')
        .map(|line| line.trim().to_string())
        .collect();
    for line in assembly.iter() {
        let instruction_args: Vec<String> = line.split(' ').map(|arg| arg.to_string()).collect();

        if instruction_args.len() >= 2 {
            let instruction = instruction_args[1].clone();

            if instruction == "PUSH4" {
                let function_selector = instruction_args[2].clone();
                function_selectors.push(function_selector);
            }
        }
    }
    function_selectors.sort();
    function_selectors.dedup();
    function_selectors
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

        if vm.exitcode != 255 || !vm.returndata.is_empty() {
            break;
        }
    }

    function_entry_point
}

// build a map of function jump possibilities from the EVM bytecode
pub fn map_selector(
    evm: &VM,
    selector: String,
    entry_point: u64,
) -> (VMTrace, u32) {
    let mut vm = evm.clone();
    vm.calldata = selector;

    // step through the bytecode until we reach the entry point
    while (vm.bytecode.len() >= (vm.instruction * 2 + 2) as usize)
        && (vm.instruction <= entry_point.into())
    {
        vm.step();

        // this shouldn't be necessary, but it's safer to have it
        if vm.exitcode != 255 || !vm.returndata.is_empty() {
            break;
        }
    }

    // the VM is at the function entry point, begin tracing
    let mut branch_count = 0;
    (
        recursive_map(
            &vm.clone(),
            &mut branch_count,
            &mut HashMap::new()
        ),
        branch_count
    )
}

pub fn recursive_map(
    evm: &VM,
    branch_count: &mut u32,
    handled_jumps: &mut HashMap<(u128, U256, usize, bool), VecDeque<StackFrame>>,
) -> VMTrace {
    let mut vm = evm.clone();

    // create a new VMTrace object
    let mut vm_trace = VMTrace {
        instruction: vm.instruction,
        operations: Vec::new(),
        children: Vec::new(),
        loop_detected: false,
    };


    // step through the bytecode until we find a JUMPI instruction
    while vm.bytecode.len() >= (vm.instruction * 2 + 2) as usize {
        let state = vm.step();
        vm_trace.operations.push(state.clone());

        // if we encounter a JUMPI, create children taking both paths and break
        if state.last_instruction.opcode == "57" {

            let jump_frame: (u128, U256, usize, bool) = (
                state.last_instruction.instruction,
                state.last_instruction.inputs[0],
                vm.stack.size(),
                state.last_instruction.inputs[1] == U256::from(0)
            );

            // break out of loops
            match handled_jumps.get(&jump_frame) {
                Some(stack) => {
                    
                    // compare stacks
                    let mut stack_diff = Vec::new();
                    for (i, frame) in vm.stack.stack.iter().enumerate() {
                        if frame != &stack[i] {
                            stack_diff.push(frame);
                        }
                    }

                    if !stack_diff.is_empty() {
                    
                        // check if all stack diff values are in the jump condition
                        let jump_condition = state.last_instruction.input_operations[1].solidify();
                        if stack_diff.iter().any(|frame| jump_condition.contains(&frame.operation.solidify())) {

                            vm_trace.loop_detected = true;
                            break;
                        }
                    }

                    // this key exists, but the stack is different, so the jump is new
                    handled_jumps.insert(jump_frame, vm.stack.stack.clone());
                },
                None => {
                    
                    // this key doesnt exist, so the jump is new
                    handled_jumps.insert(jump_frame, vm.stack.stack.clone());
                }
            }

            *branch_count += 1;

            // we need to create a trace for the path that wasn't taken.
            if state.last_instruction.inputs[1] == U256::from(0) {                

                // push a new vm trace to the children
                let mut trace_vm = vm.clone();
                trace_vm.instruction = state.last_instruction.inputs[0].as_u128() + 1;
                vm_trace.children.push(recursive_map(
                    &trace_vm,
                    branch_count,
                    handled_jumps
                ));

                // push the current path onto the stack
                vm_trace.children.push(recursive_map(
                    &vm.clone(),
                    branch_count,
                    handled_jumps
                ));
                break;
            } else {

                // push a new vm trace to the children
                let mut trace_vm = vm.clone();
                trace_vm.instruction = state.last_instruction.instruction + 1;
                vm_trace.children.push(recursive_map(
                    &trace_vm,
                    branch_count,
                    handled_jumps
                ));

                // push the current path onto the stack
                vm_trace.children.push(recursive_map(
                    &vm.clone(),
                    branch_count,
                    handled_jumps
                ));
                break;
            }
        }

        if vm.exitcode != 255 || !vm.returndata.is_empty() {
            break;
        }
    }

    vm_trace
}