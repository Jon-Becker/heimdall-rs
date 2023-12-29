mod jump_frame;
mod util;

use crate::{
    debug_max,
    ether::evm::{
        core::{
            stack::Stack,
            vm::{State, VM},
        },
        ext::exec::{
            jump_frame::JumpFrame,
            util::{
                historical_diffs_approximately_equal, jump_condition_appears_recursive,
                jump_condition_contains_mutated_memory_access,
                jump_condition_contains_mutated_storage_access,
                jump_stack_depth_less_than_max_stack_depth, stack_contains_too_many_items,
                stack_contains_too_many_of_the_same_item, stack_diff,
                stack_item_source_depth_too_deep,
            },
        },
    },
    utils::strings::decode_hex,
};
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub struct VMTrace {
    pub instruction: u128,
    pub gas_used: u128,
    pub operations: Vec<State>,
    pub children: Vec<VMTrace>,
}

impl VM {
    /// Run symbolic execution on a given function selector within a contract
    pub fn symbolic_exec_selector(&mut self, selector: &str, entry_point: u128) -> (VMTrace, u32) {
        self.calldata = decode_hex(selector).unwrap();

        // step through the bytecode until we reach the entry point
        while self.bytecode.len() >= self.instruction as usize && (self.instruction <= entry_point)
        {
            self.step();

            // this shouldn't be necessary, but it's safer to have it
            if self.exitcode != 255 || !self.returndata.is_empty() {
                break
            }
        }

        debug_max!("beginning symbolic execution for selector 0x{}", selector);

        // the VM is at the function entry point, begin tracing
        let mut branch_count = 0;
        (self.recursive_map(&mut branch_count, &mut HashMap::new()), branch_count)
    }

    // build a map of function jump possibilities from the EVM bytecode
    pub fn symbolic_exec(&self) -> (VMTrace, u32) {
        let mut vm = self.clone();

        debug_max!("beginning contract-wide symbolic execution");

        // the VM is at the function entry point, begin tracing
        let mut branch_count = 0;
        (vm.recursive_map(&mut branch_count, &mut HashMap::new()), branch_count)
    }

    fn recursive_map(
        &mut self,
        branch_count: &mut u32,
        handled_jumps: &mut HashMap<JumpFrame, Vec<Stack>>,
    ) -> VMTrace {
        let mut vm = self.clone();

        // create a new VMTrace object
        // this will essentially be a tree of executions, with each branch being a different path
        // that symbolic execution discovered
        let mut vm_trace = VMTrace {
            instruction: vm.instruction,
            gas_used: 21000,
            operations: Vec::new(),
            children: Vec::new(),
        };

        // step through the bytecode until we find a JUMPI instruction
        while vm.bytecode.len() >= vm.instruction as usize {
            let state = vm.step();

            // update vm_trace
            vm_trace.operations.push(state.clone());
            vm_trace.gas_used = vm.gas_used;

            // if we encounter a JUMP(I), create children taking both paths and break
            if state.last_instruction.opcode == 0x57 || state.last_instruction.opcode == 0x56 {
                debug_max!(
                    "found branch due to JUMP{} instruction at {}",
                    if state.last_instruction.opcode == 0x57 { "I" } else { "" },
                    state.last_instruction.instruction
                );

                let jump_condition: Option<String> =
                    state.last_instruction.input_operations.get(1).map(|op| op.solidify());
                let jump_taken =
                    state.last_instruction.inputs.get(1).map(|op| !op.is_zero()).unwrap_or(true);

                // build hashable jump frame
                let jump_frame = JumpFrame::new(
                    state.last_instruction.instruction,
                    state.last_instruction.inputs[0],
                    vm.stack.size(),
                    jump_taken,
                );

                // if the stack contains too many items, it's probably a loop
                if stack_contains_too_many_items(&vm.stack) {
                    return vm_trace
                }

                // if the stack has over 16 items of the same source, it's probably a loop
                if stack_contains_too_many_of_the_same_item(&vm.stack) {
                    return vm_trace
                }

                // if any item on the stack has a depth > 16, it's probably a loop (because of stack
                // too deep)
                if stack_item_source_depth_too_deep(&vm.stack) {
                    return vm_trace
                }

                // if the jump stack depth is less than the max stack depth of all previous matching
                // jumps, it's probably a loop
                if jump_stack_depth_less_than_max_stack_depth(&jump_frame, handled_jumps) {
                    return vm_trace
                }

                // perform heuristic checks on historical stacks
                match handled_jumps.get_mut(&jump_frame) {
                    Some(historical_stacks) => {
                        // for every stack that we have encountered for this jump, perform some
                        // heuristic checks to determine if this might be a loop
                        if historical_stacks.iter().any(|hist_stack| {
                            if let Some(jump_condition) = &jump_condition {

                                // check if any historical stack is the same as the current stack
                                if hist_stack == &vm.stack {
                                    debug_max!(
                                        "jump matches loop-detection heuristic: 'jump_path_already_handled'"
                                    );
                                    return true
                                }

                                // calculate the difference of the current stack and the historical stack
                                let stack_diff = stack_diff(&vm.stack, hist_stack);
                                if stack_diff.is_empty() {
                                    // the stack_diff is empty (the stacks are the same), so we've
                                    // already handled this path
                                    debug_max!(
                                        "jump matches loop-detection heuristic: 'stack_diff_is_empty'"
                                    );
                                    return true
                                }

                                debug_max!("stack diff: [{}]", stack_diff.iter().map(|frame| format!("{}", frame.value)).collect::<Vec<String>>().join(", "));

                                // check if the jump condition appears to be recursive
                                if jump_condition_appears_recursive(&stack_diff, jump_condition) {
                                    return true
                                }

                                // check for mutated memory accesses in the jump condition
                                if jump_condition_contains_mutated_memory_access(
                                    &stack_diff,
                                    jump_condition,
                                ) {
                                    return true
                                }

                                // check for mutated memory accesses in the jump condition
                                if jump_condition_contains_mutated_storage_access(
                                    &stack_diff,
                                    jump_condition,
                                ) {
                                    return true
                                }

                            }
                            false
                        }) {
                            debug_max!("jump terminated.");
                            debug_max!(
                                "adding historical stack {} to jump frame {:?}",
                                &format!("{:#016x?}", vm.stack.hash()),
                                jump_frame
                            );

                            // this key exists, but the stack is different, so the jump is new
                            historical_stacks.push(vm.stack.clone());
                            return vm_trace
                        }

                        if historical_diffs_approximately_equal(&vm.stack, historical_stacks) {
                            debug_max!("jump terminated.");
                            debug_max!(
                                "adding historical stack {} to jump frame {:?}",
                                &format!("{:#016x?}", vm.stack.hash()),
                                jump_frame
                            );

                            // this key exists, but the stack is different, so the jump is new
                            historical_stacks.push(vm.stack.clone());
                            return vm_trace
                        } else {
                            debug_max!(
                                "adding historical stack {} to jump frame {:?}",
                                &format!("{:#016x?}", vm.stack.hash()),
                                jump_frame
                            );
                            debug_max!(
                                " - jump condition: {:?}\n        - stack: {}\n        - historical stacks: {}",
                                jump_condition,
                                vm.stack,
                                historical_stacks.iter().map(|stack| format!("{}", stack)).collect::<Vec<String>>().join("\n            - ")
                            );

                            // this key exists, but the stack is different, so the jump is new
                            historical_stacks.push(vm.stack.clone());
                        }
                    }
                    None => {
                        // this key doesnt exist, so the jump is new
                        debug_max!("added new jump frame: {:?}", jump_frame);
                        handled_jumps.insert(jump_frame, vec![vm.stack.clone()]);
                    }
                }

                if state.last_instruction.opcode == 0x56 {
                    continue
                }

                // we didnt break out, so now we crate branching paths to cover all possibilities
                *branch_count += 1;
                debug_max!(
                    "creating branching paths at instructions {} (JUMPDEST) and {} (CONTINUE)",
                    state.last_instruction.inputs[0],
                    state.last_instruction.instruction + 1
                );

                // we need to create a trace for the path that wasn't taken.
                if !jump_taken {
                    // push a new vm trace to the children
                    let mut trace_vm = vm.clone();
                    trace_vm.instruction = state.last_instruction.inputs[0].as_u128() + 1;
                    vm_trace.children.push(trace_vm.recursive_map(branch_count, handled_jumps));

                    // push the current path onto the stack
                    vm_trace.children.push(vm.recursive_map(branch_count, handled_jumps));
                    break
                } else {
                    // push a new vm trace to the children
                    let mut trace_vm = vm.clone();
                    trace_vm.instruction = state.last_instruction.instruction + 1;
                    vm_trace.children.push(trace_vm.recursive_map(branch_count, handled_jumps));

                    // push the current path onto the stack
                    vm_trace.children.push(vm.recursive_map(branch_count, handled_jumps));
                    break
                }
            }

            // when the vm exits, this path is complete
            if vm.exitcode != 255 || !vm.returndata.is_empty() {
                break
            }
        }

        vm_trace
    }
}
