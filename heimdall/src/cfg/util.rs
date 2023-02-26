use std::{collections::{HashMap, VecDeque}};

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
    }, constants::{STORAGE_REGEX, MEMORY_REGEX},
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

// build a map of function jump possibilities from the EVM bytecode
pub fn map_contract(
    evm: &VM,
) -> (VMTrace, u32) {
    let vm = evm.clone();

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
    handled_jumps: &mut HashMap<(u128, U256, usize, bool), Vec<VecDeque<StackFrame>>>,
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

            // if the stack has over 16 items of the same source, it's probably a loop
            if vm.stack.size() > 16 {
               for frame in vm.stack.stack.iter() {
                    let solidified_frame_source = frame.operation.solidify();
                    if vm.stack.stack.iter().filter(|f| f.operation.solidify() == solidified_frame_source).count() >= 16 {
                        vm_trace.loop_detected = true;
                        return vm_trace;
                    }
               }
            }

            // break out of loops
            match handled_jumps.get(&jump_frame) {
                Some(historical_stacks) => {
                    if historical_stacks.iter().any(|stack| {

                        // compare stacks
                        let mut stack_diff = Vec::new();
                        for (i, frame) in vm.stack.stack.iter().enumerate() {
                            if frame != &stack[i] {
                                stack_diff.push(frame);
                            }
                        }

                        // println!("\nStack: ");
                        // for (i, frame) in stack.iter().enumerate() {
                        //     println!("  {} {} {}", i, frame.value, frame.operation.solidify());
                        // }

                        // println!("Stack Diff: ");
                        // for (i, frame) in stack_diff.iter().enumerate() {
                        //     println!("  {} {} {}", i, frame.value, frame.operation.solidify());
                        // }

                        if !stack_diff.is_empty() {
                    
                            // check if all stack diff values are in the jump condition
                            let jump_condition = state.last_instruction.input_operations[1].solidify();
                            
                            // if the stack diff is within the jump condition, its likely that we are in a loop
                            if stack_diff.iter().any(|frame| jump_condition.contains(&frame.operation.solidify())) {
                                return true;
                            }
                            
                            // if a memory access in the jump condition is modified by the stack diff, its likely that we are in a loop
                            let mut memory_accesses = MEMORY_REGEX.find_iter(&jump_condition);
                            if stack_diff.iter().any(|frame| {
                                return memory_accesses.any(|_match| {
                                    if _match.is_err() { return false; }
                                    let memory_access = _match.unwrap();
                                    let slice = &jump_condition[memory_access.start()..memory_access.end()];
                                    return frame.operation.solidify().contains(slice);
                                })
                            }) {
                                return true;
                            }

                            // if a storage access in the jump condition is modified by the stack diff, its likely that we are in a loop
                            let mut storage_accesses = STORAGE_REGEX.find_iter(&jump_condition);
                            if stack_diff.iter().any(|frame| {
                                return storage_accesses.any(|_match| {
                                    if _match.is_err() { return false; }
                                    let storage_access = _match.unwrap();
                                    let slice = &jump_condition[storage_access.start()..storage_access.end()];
                                    return frame.operation.solidify().contains(slice);
                                })
                            }) {
                                return true;
                            }

                            return false
                        }
                        else {
                            return true
                        }
                    }) {
                        vm_trace.loop_detected = true;
                        return vm_trace;
                    }
                    else {
                        // this key exists, but the stack is different, so the jump is new
                        let historical_stacks: &mut Vec<VecDeque<StackFrame>> = &mut historical_stacks.clone();
                        historical_stacks.push(vm.stack.stack.clone());
                        handled_jumps.insert(jump_frame, historical_stacks.to_vec());
                    }
                },
                None => {
                    
                    // this key doesnt exist, so the jump is new
                    handled_jumps.insert(jump_frame, vec![vm.stack.stack.clone()]);
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