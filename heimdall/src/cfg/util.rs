use std::collections::{HashMap, VecDeque};
use strsim::normalized_damerau_levenshtein as similarity;

use ethers::prelude::U256;
use heimdall_common::{
    constants::{MEMORY_REGEX, STORAGE_REGEX},
    ether::evm::{
        stack::StackFrame,
        vm::{State, VM},
    },
};

#[derive(Clone, Debug)]
pub struct VMTrace {
    pub instruction: u128,
    pub operations: Vec<State>,
    pub children: Vec<VMTrace>,
    pub loop_detected: bool,
}

// build a map of function jump possibilities from the EVM bytecode
pub fn map_contract(evm: &VM) -> (VMTrace, u32) {
    let vm = evm.clone();

    // the VM is at the function entry point, begin tracing
    let mut branch_count = 0;
    (recursive_map(&vm, &mut branch_count, &mut HashMap::new()), branch_count)
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

        // if we encounter a JUMPI, create children taking both paths and break
        if state.last_instruction.opcode == 0x57 {
            let jump_frame: (u128, U256, usize, bool) = (
                state.last_instruction.instruction,
                state.last_instruction.inputs[0],
                vm.stack.size(),
                state.last_instruction.inputs[1] == U256::from(0),
            );

            // if the stack has over 16 items of the same source, it's probably a loop
            if vm.stack.size() > 16 {
                for frame in vm.stack.stack.iter() {
                    let solidified_frame_source = frame.operation.solidify();
                    if vm
                        .stack
                        .stack
                        .iter()
                        .filter(|f| f.operation.solidify() == solidified_frame_source)
                        .count() >=
                        16
                    {
                        vm_trace.loop_detected = true;
                        return vm_trace
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
            if state.last_instruction.inputs[1] == U256::from(0) {
                // push a new vm trace to the children
                let mut trace_vm = vm.clone();
                trace_vm.instruction = state.last_instruction.inputs[0].as_u128() + 1;
                vm_trace.children.push(recursive_map(&trace_vm, branch_count, handled_jumps));

                // push the current path onto the stack
                vm_trace.children.push(recursive_map(&vm.clone(), branch_count, handled_jumps));
                break
            } else {
                // push a new vm trace to the children
                let mut trace_vm = vm.clone();
                trace_vm.instruction = state.last_instruction.instruction + 1;
                vm_trace.children.push(recursive_map(&trace_vm, branch_count, handled_jumps));

                // push the current path onto the stack
                vm_trace.children.push(recursive_map(&vm.clone(), branch_count, handled_jumps));
                break
            }
        }

        if vm.exitcode != 255 || !vm.returndata.is_empty() {
            break
        }
    }

    vm_trace
}
