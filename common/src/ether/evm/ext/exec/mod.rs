mod util;

use self::util::{
    jump_condition_appears_recursive, jump_condition_contains_mutated_memory_access,
    jump_condition_contains_mutated_storage_access, stack_contains_too_many_of_the_same_item,
    stack_diff, stack_item_source_depth_too_deep,
};
use crate::{
    ether::evm::core::{
        stack::{Stack, StackFrame},
        vm::{State, VM},
    },
    io::logging::Logger,
    utils::strings::decode_hex,
};
use ethers::types::U256;
use std::collections::{HashMap, VecDeque};

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

        // get a new logger
        let level = std::env::var("RUST_LOG").unwrap_or_else(|_| "INFO".into());
        let (logger, _) = Logger::new(&level);

        logger.debug_max(&format!("beginning symbolic execution for selector 0x{}", selector));

        // the VM is at the function entry point, begin tracing
        let mut branch_count = 0;
        (self.recursive_map(&mut branch_count, &mut HashMap::new(), &logger), branch_count)
    }

    // build a map of function jump possibilities from the EVM bytecode
    pub fn symbolic_exec(&self) -> (VMTrace, u32) {
        let mut vm = self.clone();

        // get a new logger
        let level = std::env::var("RUST_LOG").unwrap_or_else(|_| "INFO".into());
        let (logger, _) = Logger::new(&level);

        logger.debug_max("beginning contract-wide symbolic execution");

        // the VM is at the function entry point, begin tracing
        let mut branch_count = 0;
        (vm.recursive_map(&mut branch_count, &mut HashMap::new(), &logger), branch_count)
    }

    fn recursive_map(
        &mut self,
        branch_count: &mut u32,
        handled_jumps: &mut HashMap<(u128, U256, usize, bool), Vec<VecDeque<StackFrame>>>,
        logger: &Logger,
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

            // if we encounter a JUMPI, create children taking both paths and break
            if state.last_instruction.opcode == 0x57 {
                logger.debug_max(&format!(
                    "found branch due to JUMPI instruction at {}",
                    state.last_instruction.instruction
                ));

                // jump frame contains:
                //  1. the instruction (PC) of the JUMPI
                //  2. the jump destination
                //  3. the stack size at the time of the JUMPI
                //  4. whether the jump condition is zero
                let jump_frame: (u128, U256, usize, bool) = (
                    state.last_instruction.instruction,
                    state.last_instruction.inputs[0],
                    vm.stack.size(),
                    state.last_instruction.inputs[1].is_zero(),
                );

                // if the stack has over 16 items of the same source, it's probably a loop
                if stack_contains_too_many_of_the_same_item(&vm.stack) {
                    return vm_trace
                }

                // if any item on the stack has a depth > 16, it's probably a loop (because of stack
                // too deep)
                if stack_item_source_depth_too_deep(&vm.stack) {
                    return vm_trace
                }

                // break out of loops
                match handled_jumps.get(&jump_frame) {
                    Some(historical_stacks) => {
                        // for every stack that we have encountered for this jump, perform some
                        // heuristic checks to determine if this might be a loop
                        if historical_stacks.iter().any(|stack| {
                            // get a solidity repr of the jump condition
                            let jump_condition =
                                state.last_instruction.input_operations[1].solidify();

                            // calculate the difference of the current stack and the historical
                            // stack
                            let stack_diff = stack_diff(&vm.stack, &Stack { stack: stack.clone() });
                            if stack_diff.is_empty() {
                                // the stack_diff is empty (the stacks are the same), so we've
                                // already handled this path
                                logger.debug_max(&format!(
                                    "jump matches loop-detection heuristic: 'stack_diff_is_empty'"
                                ));
                                return true
                            }

                            // check if the jump condition appears to be recursive
                            if jump_condition_appears_recursive(&stack_diff, &jump_condition) {
                                return true
                            }

                            // check for mutated memory accesses in the jump condition
                            if jump_condition_contains_mutated_memory_access(
                                &stack_diff,
                                &jump_condition,
                            ) {
                                return true
                            }

                            // check for mutated memory accesses in the jump condition
                            if jump_condition_contains_mutated_storage_access(
                                &stack_diff,
                                &jump_condition,
                            ) {
                                return true
                            }

                            return false
                        }) {
                            logger.debug_max(&format!("jump terminated.",));
                            return vm_trace
                        } else {
                            logger.debug_max(&format!(
                                "adding historical stack {} to jump frame {:?}",
                                &format!("{:#016x?}", vm.stack.hash()),
                                jump_frame
                            ));

                            // this key exists, but the stack is different, so the jump is new
                            let historical_stacks: &mut Vec<VecDeque<StackFrame>> =
                                &mut historical_stacks.clone();
                            historical_stacks.push(vm.stack.stack.clone());
                            handled_jumps.insert(jump_frame, historical_stacks.to_vec());
                        }
                    }
                    None => {
                        // this key doesnt exist, so the jump is new
                        logger.debug_max(&format!("added new jump frame: {:?}", jump_frame));
                        handled_jumps.insert(jump_frame, vec![vm.stack.stack.clone()]);
                    }
                }

                // we didnt break out, so now we crate branching paths to cover all possibilities
                *branch_count += 1;
                logger.debug_max(&format!(
                    "creating branching paths at instructions {} (JUMPDEST) and {} (CONTINUE)",
                    state.last_instruction.inputs[0],
                    state.last_instruction.instruction + 1
                ));

                // we need to create a trace for the path that wasn't taken.
                if state.last_instruction.inputs[1].is_zero() {
                    // push a new vm trace to the children
                    let mut trace_vm = vm.clone();
                    trace_vm.instruction = state.last_instruction.inputs[0].as_u128() + 1;
                    vm_trace.children.push(trace_vm.recursive_map(
                        branch_count,
                        handled_jumps,
                        logger,
                    ));

                    // push the current path onto the stack
                    vm_trace.children.push(vm.recursive_map(branch_count, handled_jumps, logger));
                    break
                } else {
                    // push a new vm trace to the children
                    let mut trace_vm = vm.clone();
                    trace_vm.instruction = state.last_instruction.instruction + 1;
                    vm_trace.children.push(trace_vm.recursive_map(
                        branch_count,
                        handled_jumps,
                        logger,
                    ));

                    // push the current path onto the stack
                    vm_trace.children.push(vm.recursive_map(branch_count, handled_jumps, logger));
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
