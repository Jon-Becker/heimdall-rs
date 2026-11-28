mod jump_frame;
mod util;

use crate::{
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
            stack_contains_too_many_of_the_same_item, stack_diff, stack_item_source_depth_too_deep,
            stack_position_shows_pattern,
        },
    },
};
use eyre::Result;
use hashbrown::HashMap;
use heimdall_common::utils::strings::decode_hex;
use std::time::Instant;
use tracing::{trace, warn};

/// Represents a trace of virtual machine execution including operations and child calls
///
/// VMTrace is used to track the operations performed during VM execution, including
/// any nested calls that occur during execution (stored in the `children` field).
#[derive(Clone, Debug, Default)]
pub struct VMTrace {
    /// The instruction pointer at the start of this trace
    pub instruction: u128,

    /// The amount of gas used by this execution trace
    pub gas_used: u128,

    /// The sequence of VM states recorded during execution
    pub operations: Vec<State>,

    /// Child traces resulting from internal calls (CALL, DELEGATECALL, etc.)
    pub children: Vec<VMTrace>,
}

impl VM {
    /// Run symbolic execution on a given function selector within a contract
    pub fn symbolic_exec_selector(
        &mut self,
        selector: &str,
        entry_point: u128,
        timeout: Instant,
    ) -> Result<(VMTrace, u32)> {
        self.calldata = decode_hex(selector)?;

        // step through the bytecode until we reach the entry point
        while self.bytecode.len() >= self.instruction as usize && (self.instruction <= entry_point)
        {
            match self.step() {
                Ok(_) => {}
                Err(e) => {
                    warn!("failed to reach entry point for selector 0x{}: {:?}", selector, e);
                    return Err(e);
                }
            }

            // this shouldn't be necessary, but it's safer to have it
            if self.exitcode != 255 || !self.returndata.is_empty() {
                break;
            }
        }

        trace!("beginning symbolic execution for selector 0x{}", selector);

        // the VM is at the function entry point, begin tracing
        let mut branch_count = 0;
        let trace = match self.recursive_map(
            &mut branch_count,
            &mut HashMap::new(),
            &mut HashMap::new(),
            &timeout,
        )? {
            Some(trace) => trace,
            None => {
                warn!("symbolic execution returned no valid traces for selector 0x{}", selector);
                VMTrace {
                    instruction: self.instruction,
                    gas_used: self.gas_used,
                    operations: Vec::new(),
                    children: Vec::new(),
                }
            }
        };
        Ok((trace, branch_count))
    }

    /// Performs symbolic execution on the entire contract to map out control flow
    ///
    /// This method executes the VM symbolically, starting from the beginning of the bytecode,
    /// to build a comprehensive map of all possible execution paths within the contract.
    /// It tracks branching and records operation states throughout execution.
    ///
    /// # Arguments
    /// * `timeout` - An Instant representing when execution should time out
    ///
    /// # Returns
    /// * A Result containing a tuple with:
    ///   - The execution trace (VMTrace)
    ///   - The number of branches encountered during execution
    pub fn symbolic_exec(&mut self, timeout: Instant) -> Result<(VMTrace, u32)> {
        trace!("beginning contract-wide symbolic execution");

        // the VM is at the function entry point, begin tracing
        let mut branch_count = 0;
        let trace = match self.recursive_map(
            &mut branch_count,
            &mut HashMap::new(),
            &mut HashMap::new(),
            &timeout,
        )? {
            Some(trace) => trace,
            None => {
                warn!("symbolic execution returned no valid traces");
                VMTrace {
                    instruction: self.instruction,
                    gas_used: self.gas_used,
                    operations: Vec::new(),
                    children: Vec::new(),
                }
            }
        };
        Ok((trace, branch_count))
    }

    fn recursive_map(
        &mut self,
        branch_count: &mut u32,
        handled_jumps: &mut HashMap<JumpFrame, Vec<Stack>>,
        decoder_progress: &mut HashMap<JumpFrame, Vec<Stack>>,
        timeout_at: &Instant,
    ) -> Result<Option<VMTrace>> {
        let vm = self;

        // create a new VMTrace object
        // this will essentially be a tree of executions, with each branch being a different path
        // that symbolic execution discovered
        let mut vm_trace = VMTrace {
            instruction: vm.instruction,
            gas_used: 0,
            operations: Vec::new(),
            children: Vec::new(),
        };

        // step through the bytecode until we find a JUMPI instruction
        while vm.bytecode.len() >= vm.instruction as usize {
            // if we have reached the timeout, return None
            if Instant::now() >= *timeout_at {
                return Ok(None);
            }

            // execute the next instruction. if the instruction panics, invalidate this path
            let state = match vm.step() {
                Ok(state) => state,
                Err(e) => {
                    warn!("executing branch failed during step: {:?}", e);
                    return Ok(None);
                }
            };
            let last_instruction = state.last_instruction.clone();

            // update vm_trace
            vm_trace.operations.push(state);
            vm_trace.gas_used = vm.gas_used;

            // if we encounter a JUMP(I), create children taking both paths and break
            if last_instruction.opcode == 0x57 {
                trace!(
                    "found branch due to JUMP{} instruction at {}",
                    if last_instruction.opcode == 0x57 { "I" } else { "" },
                    last_instruction.instruction
                );

                let jump_condition: Option<String> =
                    last_instruction.input_operations.get(1).map(|op| op.solidify());
                let jump_taken =
                    last_instruction.inputs.get(1).map(|op| !op.is_zero()).unwrap_or(true);

                // build hashable jump frame
                let jump_frame = JumpFrame::new(
                    last_instruction.instruction,
                    last_instruction.inputs[0],
                    vm.stack.size(),
                    jump_taken,
                    last_instruction.input_operations.get(1),
                );

                // if the stack contains too many items, it's probably a loop
                if stack_contains_too_many_items(&vm.stack) {
                    return Ok(None);
                }

                // if the stack has over 16 items of the same source, it's probably a loop
                if stack_contains_too_many_of_the_same_item(&vm.stack) {
                    return Ok(None);
                }

                // if any item on the stack has a depth > 16, it's probably a loop (because of stack
                // too deep)
                if stack_item_source_depth_too_deep(&vm.stack) {
                    return Ok(None);
                }

                // if the jump stack depth is less than the max stack depth of all previous matching
                // jumps, it's probably a loop
                if jump_stack_depth_less_than_max_stack_depth(&jump_frame, handled_jumps) {
                    return Ok(None);
                }

                // A loop can scan successive statically folded calldata offsets, producing a
                // new apparent ABI head each time. Keep numeric-progress checks across decoder
                // contexts so changing the inferred owner cannot bypass loop termination.
                if !jump_frame.calldata_heads.is_empty() {
                    let mut progress_frame = jump_frame.clone();
                    progress_frame.calldata_heads.clear();
                    let prior_contexts = decoder_progress.entry(progress_frame).or_default();
                    if historical_diffs_approximately_equal(&vm.stack, prior_contexts) ||
                        stack_position_shows_pattern(&vm.stack, prior_contexts)
                    {
                        return Ok(None);
                    }
                    prior_contexts.push(vm.stack.clone());
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
                                    trace!(
                                        "jump matches loop-detection heuristic: 'jump_path_already_handled'"
                                    );
                                    return true
                                }

                                // calculate the difference of the current stack and the historical stack
                                let stack_diff = stack_diff(&vm.stack, hist_stack);
                                if stack_diff.is_empty() {
                                    // the stack_diff is empty (the stacks are the same), so we've
                                    // already handled this path
                                    trace!(
                                        "jump matches loop-detection heuristic: 'stack_diff_is_empty'"
                                    );
                                    return true
                                }

                                trace!("stack diff: [{}]", stack_diff.iter().map(|frame| format!("{}", frame.value)).collect::<Vec<String>>().join(", "));

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
                            trace!("jump terminated.");
                            trace!(
                                "adding historical stack {} to jump frame {:?}",
                                &format!("{:#016x?}", vm.stack.hash()),
                                jump_frame
                            );

                            // this key exists, but the stack is different, so the jump is new
                            historical_stacks.push(vm.stack.clone());
                            return Ok(None);
                        }

                        // check if any stack position shows a consistent pattern
                        // (increasing/decreasing/alternating)
                        if stack_position_shows_pattern(&vm.stack, historical_stacks) {
                            trace!("jump terminated.");
                            trace!(
                                "adding historical stack {} to jump frame {:?}",
                                &format!("{:#016x?}", vm.stack.hash()),
                                jump_frame
                            );

                            // this key exists, but the stack is different, so the jump is new
                            historical_stacks.push(vm.stack.clone());
                            return Ok(None);
                        }

                        if historical_diffs_approximately_equal(&vm.stack, historical_stacks) {
                            trace!("jump terminated.");
                            trace!(
                                "adding historical stack {} to jump frame {:?}",
                                &format!("{:#016x?}", vm.stack.hash()),
                                jump_frame
                            );

                            // this key exists, but the stack is different, so the jump is new
                            historical_stacks.push(vm.stack.clone());
                            return Ok(None);
                        }

                        {
                            trace!(
                                "adding historical stack {} to jump frame {:?}",
                                &format!("{:#016x?}", vm.stack.hash()),
                                jump_frame
                            );
                            trace!(
                                " - jump condition: {:?}\n        - stack: {}\n        - historical stacks: {}",
                                jump_condition,
                                vm.stack,
                                historical_stacks.iter().map(|stack| format!("{stack}")).collect::<Vec<String>>().join("\n            - ")
                            );

                            // this key exists, but the stack is different, so the jump is new
                            historical_stacks.push(vm.stack.clone());
                        }
                    }
                    None => {
                        // this key doesnt exist, so the jump is new
                        trace!("added new jump frame: {:?}", jump_frame);
                        handled_jumps.insert(jump_frame, vec![vm.stack.clone()]);
                    }
                }

                if last_instruction.opcode == 0x56 {
                    continue;
                }

                // we didnt break out, so now we crate branching paths to cover all possibilities
                *branch_count += 1;
                trace!(
                    "creating branching paths at instructions {} (JUMPDEST) and {} (CONTINUE)",
                    last_instruction.inputs[0],
                    last_instruction.instruction + 1
                );

                // we need to create a trace for the path that wasn't taken.
                if !jump_taken {
                    // push a new vm trace to the children
                    let mut trace_vm = vm.clone();
                    trace_vm.instruction =
                        last_instruction.inputs[0].try_into().unwrap_or(u128::MAX) + 1;
                    match trace_vm.recursive_map(
                        branch_count,
                        handled_jumps,
                        decoder_progress,
                        timeout_at,
                    ) {
                        Ok(Some(child_trace)) => vm_trace.children.push(child_trace),
                        Ok(None) => {}
                        Err(e) => {
                            warn!("error executing branch: {:?}", e);
                            return Ok(None);
                        }
                    }

                    // push the current path onto the stack
                    match vm.recursive_map(
                        branch_count,
                        handled_jumps,
                        decoder_progress,
                        timeout_at,
                    ) {
                        Ok(Some(child_trace)) => vm_trace.children.push(child_trace),
                        Ok(None) => {}
                        Err(e) => {
                            warn!("error executing branch: {:?}", e);
                            return Ok(None);
                        }
                    }
                    break;
                } else {
                    // push a new vm trace to the children
                    let mut trace_vm = vm.clone();
                    trace_vm.instruction = last_instruction.instruction + 1;
                    match trace_vm.recursive_map(
                        branch_count,
                        handled_jumps,
                        decoder_progress,
                        timeout_at,
                    ) {
                        Ok(Some(child_trace)) => vm_trace.children.push(child_trace),
                        Ok(None) => {}
                        Err(e) => {
                            warn!("error executing branch: {:?}", e);
                            return Ok(None);
                        }
                    }

                    // push the current path onto the stack
                    match vm.recursive_map(
                        branch_count,
                        handled_jumps,
                        decoder_progress,
                        timeout_at,
                    ) {
                        Ok(Some(child_trace)) => vm_trace.children.push(child_trace),
                        Ok(None) => {}
                        Err(e) => {
                            warn!("error executing branch: {:?}", e);
                            return Ok(None);
                        }
                    }
                    break;
                }
            }

            // when the vm exits, this path is complete
            if vm.exitcode != 255 || !vm.returndata.is_empty() {
                break;
            }
        }

        Ok(Some(vm_trace))
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use alloy::primitives::Address;

    use super::*;
    use crate::core::opcodes::{CALLDATALOAD, STOP};

    fn execute(bytecode: &str) -> (VMTrace, u32) {
        VM::new(
            &decode_hex(bytecode).unwrap(),
            &[],
            Address::ZERO,
            Address::ZERO,
            Address::ZERO,
            0,
            u128::MAX,
        )
        .symbolic_exec(Instant::now() + Duration::from_secs(5))
        .unwrap()
    }

    fn opcodes(trace: &VMTrace) -> Vec<u8> {
        trace
            .operations
            .iter()
            .map(|state| state.last_instruction.opcode)
            .chain(trace.children.iter().flat_map(opcodes))
            .collect()
    }

    #[test]
    fn shared_calldata_decoder_reaches_each_argument_and_the_function_body() {
        // Four calls pass 4 + calldataload(head) to one helper, with head offsets
        // 4, 36, 68, and 100. The helper checks that the length word fits within calldata,
        // then returns to its caller. STOP is the body following all four decodes.
        let (trace, _) = execute(concat!(
            "600b6004356004016031565b",
            "60176024356004016031565b",
            "60236044356004016031565b",
            "602f6064356004016031565b00",
            "5b80601f013611603e575f5ffd5b5056",
        ));
        let operations = opcodes(&trace);
        assert_eq!(operations.iter().filter(|opcode| **opcode == CALLDATALOAD).count(), 4);
        assert!(operations.contains(&STOP));
    }

    #[test]
    fn loop_over_elements_of_one_calldata_argument_still_terminates() {
        // Start at 4 + calldataload(4), read a word, advance by 32, and repeat while
        // below calldatasize(). Symbolic offsets change, but the owning head does not.
        let (trace, branches) = execute("6004356004015b80355060200180361060065700");
        assert!(branches <= 2, "loop should be bounded by its existing history");
        assert!(opcodes(&trace).len() < 32);
    }

    #[test]
    fn scanning_calldata_words_cannot_escape_loop_history_by_changing_heads() {
        for bytecode in [
            // Branch on 4 + calldataload(index).
            "60045b8035600401906020019060025700",
            // The same scan with a calldatasize() comparison resembles a decoder guard.
            "60045b80356004013610906020019060025700",
        ] {
            let (trace, branches) = execute(bytecode);
            assert!(branches <= 5, "successive head offsets must not reset loop history");
            assert!(opcodes(&trace).len() < 100);
        }
    }
}
