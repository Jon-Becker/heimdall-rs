mod jump_frame;
mod loop_analysis;
mod util;

use crate::{
    core::{
        stack::Stack,
        vm::{State, VM},
    },
    ext::exec::{
        jump_frame::JumpFrame,
        loop_analysis::{detect_induction_variable, is_tautologically_false_condition},
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

pub use loop_analysis::{InductionDirection, InductionVariable, LoopInfo};

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

    /// Detected loops during symbolic execution
    pub detected_loops: Vec<LoopInfo>,
}

/// Collect detected loops from a child trace into the parent's loop list.
/// Only adds loops that aren't already present (by header_pc and condition_pc).
fn collect_child_loops(child: &VMTrace, parent_loops: &mut Vec<LoopInfo>) {
    // Early exit if child has no loops
    if child.detected_loops.is_empty() {
        return;
    }

    // Pre-reserve capacity for potential additions
    parent_loops.reserve(child.detected_loops.len());

    // Use extend with filter for cleaner code and potential optimization
    for loop_info in &child.detected_loops {
        // Check existence using tuple key for fast comparison
        let key = (loop_info.header_pc, loop_info.condition_pc);
        let already_exists = parent_loops.iter().any(|l| (l.header_pc, l.condition_pc) == key);
        if !already_exists {
            parent_loops.push(loop_info.clone());
        }
    }
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
        let trace = match self.recursive_map(&mut branch_count, &mut HashMap::new(), &timeout)? {
            Some(trace) => trace,
            None => {
                warn!("symbolic execution returned no valid traces for selector 0x{}", selector);
                VMTrace {
                    instruction: self.instruction,
                    gas_used: self.gas_used,
                    operations: Vec::new(),
                    children: Vec::new(),
                    detected_loops: Vec::new(),
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
        let trace = match self.recursive_map(&mut branch_count, &mut HashMap::new(), &timeout)? {
            Some(trace) => trace,
            None => {
                warn!("symbolic execution returned no valid traces");
                VMTrace {
                    instruction: self.instruction,
                    gas_used: self.gas_used,
                    operations: Vec::new(),
                    children: Vec::new(),
                    detected_loops: Vec::new(),
                }
            }
        };
        Ok((trace, branch_count))
    }

    fn recursive_map(
        &mut self,
        branch_count: &mut u32,
        handled_jumps: &mut HashMap<JumpFrame, Vec<Stack>>,
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
            detected_loops: Vec::new(),
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

                // perform heuristic checks on historical stacks
                match handled_jumps.get_mut(&jump_frame) {
                    Some(historical_stacks) => {
                        // Check if this is a loop - find the first historical stack that matches
                        // loop patterns and capture the stack diff for induction variable detection
                        let mut detected_loop_info: Option<(Vec<_>, String)> = None;

                        for hist_stack in historical_stacks.iter() {
                            if let Some(ref cond) = jump_condition {
                                // check if any historical stack is the same as the current stack
                                if hist_stack == &vm.stack {
                                    trace!(
                                        "jump matches loop-detection heuristic: 'jump_path_already_handled'"
                                    );
                                    detected_loop_info = Some((Vec::new(), cond.clone()));
                                    break;
                                }

                                // calculate the difference of the current stack and the historical
                                // stack
                                let diff = stack_diff(&vm.stack, hist_stack);
                                if diff.is_empty() {
                                    trace!(
                                        "jump matches loop-detection heuristic: 'stack_diff_is_empty'"
                                    );
                                    detected_loop_info = Some((diff, cond.clone()));
                                    break;
                                }

                                trace!(
                                    "stack diff: [{}]",
                                    diff.iter()
                                        .map(|frame| format!("{}", frame.value))
                                        .collect::<Vec<String>>()
                                        .join(", ")
                                );

                                // check if the jump condition appears to be recursive
                                if jump_condition_appears_recursive(&diff, cond) {
                                    detected_loop_info = Some((diff, cond.clone()));
                                    break;
                                }

                                // check for mutated memory accesses in the jump condition
                                if jump_condition_contains_mutated_memory_access(&diff, cond) {
                                    detected_loop_info = Some((diff, cond.clone()));
                                    break;
                                }

                                // check for mutated storage accesses in the jump condition
                                if jump_condition_contains_mutated_storage_access(&diff, cond) {
                                    detected_loop_info = Some((diff, cond.clone()));
                                    break;
                                }
                            }
                        }

                        // If a loop was detected, capture the LoopInfo and return the trace
                        if let Some((diff, condition)) = detected_loop_info {
                            // Skip loops with tautologically false conditions (e.g., "0 > 1")
                            // These are not real loops but rather overflow checks or dead code
                            if is_tautologically_false_condition(&condition) {
                                trace!(
                                    "skipping loop with tautologically false condition: {}",
                                    condition
                                );
                                historical_stacks.push(vm.stack.clone());
                                // Continue execution without creating a loop
                            } else {
                                trace!("loop detected, capturing LoopInfo");
                                trace!(
                                    "adding historical stack {} to jump frame {:?}",
                                    &format!("{:#016x?}", vm.stack.hash()),
                                    jump_frame
                                );
                                historical_stacks.push(vm.stack.clone());

                                // Create LoopInfo with header_pc (jump target) and condition_pc
                                // (JUMPI)
                                let header_pc: u128 =
                                    last_instruction.inputs[0].try_into().unwrap_or(0);
                                let condition_pc = last_instruction.instruction;

                                // Try to detect induction variable from the stack diff
                                let induction_var =
                                    detect_induction_variable(&diff, &Some(condition.clone()));

                                let mut loop_info =
                                    LoopInfo::new(header_pc, condition_pc, condition);

                                if let Some(iv) = induction_var {
                                    loop_info.induction_var = Some(iv);
                                    loop_info.is_bounded = true;
                                }

                                trace!(
                                    "detected loop: header_pc={}, condition_pc={}, condition={}",
                                    header_pc,
                                    condition_pc,
                                    loop_info.condition
                                );

                                vm_trace.detected_loops.push(loop_info);

                                // Return the trace with the loop info (not None)
                                return Ok(Some(vm_trace));
                            }
                        }

                        // check if any stack position shows a consistent pattern
                        // (increasing/decreasing/alternating)
                        if stack_position_shows_pattern(&vm.stack, historical_stacks) {
                            let condition =
                                jump_condition.clone().unwrap_or_else(|| "true".to_string());

                            // Skip tautologically false conditions
                            if is_tautologically_false_condition(&condition) {
                                trace!(
                                    "skipping loop (stack pattern) with false condition: {}",
                                    condition
                                );
                                historical_stacks.push(vm.stack.clone());
                            } else {
                                trace!("loop detected via stack pattern");
                                trace!(
                                    "adding historical stack {} to jump frame {:?}",
                                    &format!("{:#016x?}", vm.stack.hash()),
                                    jump_frame
                                );
                                historical_stacks.push(vm.stack.clone());

                                // Create basic loop info even without detailed condition
                                let header_pc: u128 =
                                    last_instruction.inputs[0].try_into().unwrap_or(0);
                                let condition_pc = last_instruction.instruction;

                                let loop_info = LoopInfo::new(header_pc, condition_pc, condition);
                                vm_trace.detected_loops.push(loop_info);

                                return Ok(Some(vm_trace));
                            }
                        }

                        if historical_diffs_approximately_equal(&vm.stack, historical_stacks) {
                            let condition =
                                jump_condition.clone().unwrap_or_else(|| "true".to_string());

                            // Skip tautologically false conditions
                            if is_tautologically_false_condition(&condition) {
                                trace!(
                                    "skipping loop (approx diffs) with false condition: {}",
                                    condition
                                );
                                historical_stacks.push(vm.stack.clone());
                            } else {
                                trace!("loop detected via approximate diffs");
                                trace!(
                                    "adding historical stack {} to jump frame {:?}",
                                    &format!("{:#016x?}", vm.stack.hash()),
                                    jump_frame
                                );
                                historical_stacks.push(vm.stack.clone());

                                // Create basic loop info
                                let header_pc: u128 =
                                    last_instruction.inputs[0].try_into().unwrap_or(0);
                                let condition_pc = last_instruction.instruction;

                                let loop_info = LoopInfo::new(header_pc, condition_pc, condition);
                                vm_trace.detected_loops.push(loop_info);

                                return Ok(Some(vm_trace));
                            }
                        } else {
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
                    match trace_vm.recursive_map(branch_count, handled_jumps, timeout_at) {
                        Ok(Some(child_trace)) => {
                            collect_child_loops(&child_trace, &mut vm_trace.detected_loops);
                            vm_trace.children.push(child_trace);
                        }
                        Ok(None) => {}
                        Err(e) => {
                            warn!("error executing branch: {:?}", e);
                            return Ok(None);
                        }
                    }

                    // push the current path onto the stack
                    match vm.recursive_map(branch_count, handled_jumps, timeout_at) {
                        Ok(Some(child_trace)) => {
                            collect_child_loops(&child_trace, &mut vm_trace.detected_loops);
                            vm_trace.children.push(child_trace);
                        }
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
                    match trace_vm.recursive_map(branch_count, handled_jumps, timeout_at) {
                        Ok(Some(child_trace)) => {
                            collect_child_loops(&child_trace, &mut vm_trace.detected_loops);
                            vm_trace.children.push(child_trace);
                        }
                        Ok(None) => {}
                        Err(e) => {
                            warn!("error executing branch: {:?}", e);
                            return Ok(None);
                        }
                    }

                    // push the current path onto the stack
                    match vm.recursive_map(branch_count, handled_jumps, timeout_at) {
                        Ok(Some(child_trace)) => {
                            collect_child_loops(&child_trace, &mut vm_trace.detected_loops);
                            vm_trace.children.push(child_trace);
                        }
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
    // TODO: add tests for symbolic execution & recursive_map
}
