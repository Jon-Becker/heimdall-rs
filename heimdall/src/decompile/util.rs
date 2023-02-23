use std::{collections::{HashMap, VecDeque}};

use ethers::{
    prelude::{
        U256,
    },
};
use heimdall_common::{
    ether::{
        evm::{
            log::Log,
            opcodes::WrappedOpcode,
            vm::{State, VM}, stack::{StackFrame}
        }, signatures::{ResolvedFunction, ResolvedError, ResolvedLog},
    }, constants::{MEMORY_REGEX, STORAGE_REGEX},
};

#[derive(Clone, Debug)]
pub struct Function {
    // the function's 4byte selector
    pub selector: String,

    // the function's entry point in the code.
    // the entry point is the instruction the dispatcher JUMPs to when called.
    pub entry_point: u64,

    // argument structure:
    //   - key : slot operations of the argument.
    //   - value : tuple of ({slot: U256, mask: usize}, potential_types)
    pub arguments: HashMap<usize, (CalldataFrame, Vec<String>)>,

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

    // holds all found events used to generate solidity error definitions
    // as well as ABI specifications.
    pub events: HashMap<String, (Option<ResolvedLog>, Log)>,

    // holds all found custom errors used to generate solidity error definitions
    // as well as ABI specifications.
    pub errors: HashMap<String, Option<ResolvedError>>,

    // stores the matched resolved function for this Functon
    pub resolved_function: Option<ResolvedFunction>,

    // stores the current indent depth, used for formatting and removing unnecessary closing brackets.
    pub indent_depth: usize,

    // stores decompiler notices
    pub notices: Vec<String>,

    // modifiers
    pub pure: bool,
    pub view: bool,
    pub payable: bool,
}

#[derive(Clone, Debug)]
pub struct StorageFrame {
    pub value: U256,
    pub operations: WrappedOpcode,
}

#[derive(Clone, Debug)]
pub struct CalldataFrame {
    pub slot: usize,
    pub operation: String,
    pub mask_size: usize,
    pub heuristics: Vec<String>,
}

impl Function {
    // get a specific memory slot

    pub fn get_memory_range(&self, _offset: U256, _size: U256) -> Vec<StorageFrame> {
        let mut memory_slice: Vec<StorageFrame> = Vec::new();

        // Safely convert U256 to usize
        let mut offset: usize = _offset.try_into().unwrap_or(0);
        let mut size: usize = _size.try_into().unwrap_or(0);

        // get the memory range
        while size > 0 {
            match self.memory.get(&U256::from(offset)) {
                Some(memory) => {
                    memory_slice.push(memory.clone());
                }
                None => {}
            }
            offset += 32;
            size = size.saturating_sub(32);
        }

        memory_slice
    }
}

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

// find all function selectors in the given EVM assembly.
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

    // execute the EVM call to find the entry point for the given selector
    vm.calldata = selector.clone();
    while vm.bytecode.len() >= (vm.instruction * 2 + 2) as usize {
        let call = vm.step();

        // if the opcode is an JUMPI and it matched the selector, the next jumpi is the entry point
        if call.last_instruction.opcode == "57" {
            let jump_condition = call.last_instruction.input_operations[1].solidify();
            let jump_taken = call.last_instruction.inputs[1].as_u64();

            if jump_condition.contains(&selector) &&
               jump_condition.contains("msg.data[0]") &&
               jump_condition.contains(" == ") &&
               jump_taken == 1
            {
                return call.last_instruction.inputs[0].as_u64();
            }
        }

        if vm.exitcode != 255 || !vm.returndata.is_empty() {
            break;
        }
    }

    0
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