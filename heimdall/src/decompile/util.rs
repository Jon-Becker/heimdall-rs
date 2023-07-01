use std::collections::{HashMap, VecDeque};
use strsim::normalized_damerau_levenshtein as similarity;

use ethers::prelude::U256;
use heimdall_common::{
    constants::{MEMORY_REGEX, STORAGE_REGEX},
    ether::{
        evm::{
            log::Log,
            opcodes::WrappedOpcode,
            stack::StackFrame,
            vm::{State, VM},
        },
        signatures::{ResolvedError, ResolvedFunction, ResolvedLog},
    },
    utils::strings::decode_hex,
};

#[derive(Clone, Debug)]
pub struct Function {
    // the function's 4byte selector
    pub selector: String,

    // the function's entry point in the code.
    // the entry point is the instruction the dispatcher JUMPs to when called.
    pub entry_point: u128,

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
    pub events: HashMap<U256, (Option<ResolvedLog>, Log)>,

    // holds all found custom errors used to generate solidity error definitions
    // as well as ABI specifications.
    pub errors: HashMap<U256, Option<ResolvedError>>,

    // stores the matched resolved function for this Functon
    pub resolved_function: Option<ResolvedFunction>,

    // stores the current indent depth, used for formatting and removing unnecessary closing
    // brackets.
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
        let mut offset: usize = std::cmp::min(_offset.try_into().unwrap_or(0), 2048);
        let mut size: usize = std::cmp::min(_size.try_into().unwrap_or(0), 2048);

        // get the memory range
        while size > 0 {
            if let Some(memory) = self.memory.get(&U256::from(offset)) {
                memory_slice.push(memory.clone());
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
    pub loop_detected: bool,
}

// build a map of function jump possibilities from the EVM bytecode
pub fn map_selector(evm: &VM, selector: String, entry_point: u128) -> (VMTrace, u32) {
    let mut vm = evm.clone();
    vm.calldata = decode_hex(&selector).unwrap();

    // step through the bytecode until we reach the entry point
    while vm.bytecode.len() >= vm.instruction as usize && (vm.instruction <= entry_point) {
        vm.step();

        // this shouldn't be necessary, but it's safer to have it
        if vm.exitcode != 255 || !vm.returndata.is_empty() {
            break
        }
    }

    // the VM is at the function entry point, begin tracing
    let mut branch_count = 0;
    (recursive_map(&vm.clone(), &mut branch_count, &mut HashMap::new()), branch_count)
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
    while vm.bytecode.len() >= vm.instruction as usize {
        let state = vm.step();
        vm_trace.operations.push(state.clone());

        // if we encounter a JUMP, print the jumpdest source
        // if state.last_instruction.opcode == 0x56 {
        //
        // }

        // if we encounter a JUMPI, create children taking both paths and break
        if state.last_instruction.opcode == 0x57 {
            let jump_frame: (u128, U256, usize, bool) = (
                state.last_instruction.instruction,
                state.last_instruction.inputs[0],
                vm.stack.size(),
                state.last_instruction.inputs[1].is_zero(),
            );

            // if the stack has over 16 items of the same source, it's probably a loop
            if vm.stack.size() > 16 &&
                vm.stack.stack.iter().any(|frame| {
                    let solidified_frame_source = frame.operation.solidify();
                    vm.stack
                        .stack
                        .iter()
                        .filter(|f| f.operation.solidify() == solidified_frame_source)
                        .count() >=
                        16
                })
            {
                vm_trace.loop_detected = true;
                return vm_trace
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

                                // check similarity of stack diff values against the stack, using
                                // normalized Levenshtein distance
                                for stack_frame in stack.iter() {
                                    let solidified_frame = frame.operation.solidify();
                                    let solidified_stack_frame = stack_frame.operation.solidify();

                                    if similarity(&solidified_frame, &solidified_stack_frame) > 0.9
                                    {
                                        return true
                                    }
                                }
                            }
                        }

                        if !stack_diff.is_empty() {
                            // check if all stack diff values are in the jump condition
                            let jump_condition =
                                state.last_instruction.input_operations[1].solidify();

                            // if the stack diff is within the jump condition, its likely that we
                            // are in a loop
                            if stack_diff
                                .iter()
                                .map(|frame| frame.operation.solidify())
                                .any(|solidified| jump_condition.contains(&solidified))
                            {
                                return true
                            }

                            // if we repeat conditionals, its likely that we are in a loop
                            if stack_diff.iter().any(|frame| {
                                let solidified = frame.operation.solidify();
                                jump_condition.contains(&solidified) &&
                                    jump_condition.matches(&solidified).count() > 1
                            }) {
                                return true
                            }

                            // if a memory access in the jump condition is modified by the stack
                            // diff, its likely that we are in a loop
                            let mut memory_accesses = MEMORY_REGEX.find_iter(&jump_condition);
                            if stack_diff.iter().any(|frame| {
                                memory_accesses.any(|_match| {
                                    if _match.is_err() {
                                        return false
                                    }
                                    let memory_access = _match.unwrap();
                                    let slice =
                                        &jump_condition[memory_access.start()..memory_access.end()];
                                    frame.operation.solidify().contains(slice)
                                })
                            }) {
                                return true
                            }

                            // if a storage access in the jump condition is modified by the stack
                            // diff, its likely that we are in a loop
                            let mut storage_accesses = STORAGE_REGEX.find_iter(&jump_condition);
                            if stack_diff.iter().any(|frame| {
                                storage_accesses.any(|_match| {
                                    if _match.is_err() {
                                        return false
                                    }
                                    let storage_access = _match.unwrap();
                                    let slice = &jump_condition
                                        [storage_access.start()..storage_access.end()];
                                    frame.operation.solidify().contains(slice)
                                })
                            }) {
                                return true
                            }

                            false
                        } else {
                            true
                        }
                    }) {
                        vm_trace.loop_detected = true;
                        return vm_trace
                    } else {
                        // this key exists, but the stack is different, so the jump is new
                        let historical_stacks: &mut Vec<VecDeque<StackFrame>> =
                            &mut historical_stacks.clone();
                        historical_stacks.push(vm.stack.stack.clone());
                        handled_jumps.insert(jump_frame, historical_stacks.to_vec());
                    }
                }
                None => {
                    // this key doesnt exist, so the jump is new
                    handled_jumps.insert(jump_frame, vec![vm.stack.stack.clone()]);
                }
            }

            *branch_count += 1;

            // we need to create a trace for the path that wasn't taken.
            if state.last_instruction.inputs[1].is_zero() {
                // push a new vm trace to the children
                let mut trace_vm = vm.clone();
                trace_vm.instruction = state.last_instruction.inputs[0].as_u128() + 1;
                vm_trace.children.push(recursive_map(&trace_vm, branch_count, handled_jumps));

                // push the current path onto the stack
                vm_trace.children.push(recursive_map(&vm, branch_count, handled_jumps));
                break
            } else {
                // push a new vm trace to the children
                let mut trace_vm = vm.clone();
                trace_vm.instruction = state.last_instruction.instruction + 1;
                vm_trace.children.push(recursive_map(&trace_vm, branch_count, handled_jumps));

                // push the current path onto the stack
                vm_trace.children.push(recursive_map(&vm, branch_count, handled_jumps));
                break
            }
        }

        if vm.exitcode != 255 || !vm.returndata.is_empty() {
            break
        }
    }

    vm_trace
}
