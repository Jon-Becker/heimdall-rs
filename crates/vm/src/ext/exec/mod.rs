mod jump_frame;
mod util;

use crate::{
    core::{
        program::Program,
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
use eyre::{eyre, Result};
use hashbrown::HashMap;
use heimdall_common::utils::strings::decode_hex;
use std::time::Instant;
use tracing::{trace, warn};

/// A segment of VM execution and its explored conditional continuations.
#[derive(Clone, Debug, Default)]
pub struct VMTrace {
    /// The instruction pointer at the start of this trace
    pub instruction: u128,

    /// The amount of gas used by this execution trace
    pub gas_used: u128,

    /// The sequence of VM states recorded during execution
    pub operations: Vec<State>,

    /// Retained paths following conditional jumps.
    pub children: Vec<VMTrace>,
}

/// The reason symbolic exploration stopped at a trace node.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TraceEnd {
    /// Both alternatives of a conditional jump were explored.
    Branch,
    /// The VM halted during execution.
    Halt,
    /// Execution reached the end of the bytecode.
    EndOfCode,
    /// Exploration was stopped by its deadline.
    Timeout,
    /// A loop-detection heuristic stopped this path.
    LoopCutoff,
    /// The VM could not execute an instruction on this path.
    StepError,
    /// A branch destination could not be represented.
    InvalidDestination,
}

/// An explored conditional alternative. `child_index` refers to `VMTrace::children`;
/// it is absent when that alternative was cut off and produced no legacy trace.
#[derive(Clone, Debug)]
pub struct TraceBranch {
    /// Whether the conditional jump was taken.
    pub taken: bool,
    /// Index of the corresponding child in the legacy trace, if retained.
    pub child_index: Option<usize>,
    /// Exploration details for this alternative.
    pub details: Box<TraceDetails>,
}

/// Exploration information accompanying an unchanged [`VMTrace`].
#[derive(Clone, Debug)]
pub struct TraceDetails {
    /// How this node ended.
    pub end: TraceEnd,
    /// Conditional alternatives, including paths discarded by the legacy API.
    pub branches: Vec<TraceBranch>,
}

impl TraceDetails {
    fn ended(end: TraceEnd) -> Self {
        Self { end, branches: Vec::new() }
    }
}

struct TraceOutcome {
    trace: Option<VMTrace>,
    details: TraceDetails,
}

impl TraceOutcome {
    fn cutoff(end: TraceEnd) -> Self {
        Self { trace: None, details: TraceDetails::ended(end) }
    }

    fn append_to(self, parent: &mut VMTrace, details: &mut TraceDetails, taken: bool) {
        let child_index = self.trace.map(|trace| {
            let index = parent.children.len();
            parent.children.push(trace);
            index
        });
        details.branches.push(TraceBranch { taken, child_index, details: Box::new(self.details) });
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
        let (trace, _, branches) =
            self.symbolic_exec_selector_detailed(selector, entry_point, timeout)?;
        Ok((trace, branches))
    }

    /// Run selector exploration with branch and cutoff details.
    pub fn symbolic_exec_selector_detailed(
        &mut self,
        selector: &str,
        entry_point: u128,
        timeout: Instant,
    ) -> Result<(VMTrace, TraceDetails, u32)> {
        self.calldata = decode_hex(selector)?;

        while self.bytecode.len() >= self.instruction as usize && (self.instruction <= entry_point)
        {
            if Instant::now() >= timeout {
                return Err(eyre!(
                    "timed out while reaching entry point for selector 0x{}",
                    selector
                ));
            }
            match self.step() {
                Ok(_) => {}
                Err(e) => {
                    warn!("failed to reach entry point for selector 0x{}: {:?}", selector, e);
                    return Err(e);
                }
            }

            if self.exitcode != 255 || !self.returndata.is_empty() {
                break;
            }
        }

        trace!("beginning symbolic execution for selector 0x{}", selector);

        let mut branch_count = 0;
        let outcome = self.recursive_map(&mut branch_count, &mut HashMap::new(), &timeout);
        let trace = match outcome.trace {
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
        Ok((trace, outcome.details, branch_count))
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
        let (trace, _, branches) = self.symbolic_exec_detailed(timeout)?;
        Ok((trace, branches))
    }

    /// Perform contract-wide exploration with branch and cutoff details.
    pub fn symbolic_exec_detailed(
        &mut self,
        timeout: Instant,
    ) -> Result<(VMTrace, TraceDetails, u32)> {
        trace!("beginning contract-wide symbolic execution");

        let mut branch_count = 0;
        let outcome = self.recursive_map(&mut branch_count, &mut HashMap::new(), &timeout);
        let trace = match outcome.trace {
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
        Ok((trace, outcome.details, branch_count))
    }

    fn recursive_map(
        &mut self,
        branch_count: &mut u32,
        handled_jumps: &mut HashMap<JumpFrame, Vec<Stack>>,
        timeout_at: &Instant,
    ) -> TraceOutcome {
        let vm = self;

        let mut vm_trace = VMTrace {
            instruction: vm.instruction,
            gas_used: 0,
            operations: Vec::new(),
            children: Vec::new(),
        };

        let mut details = TraceDetails::ended(TraceEnd::EndOfCode);

        while vm.bytecode.len() >= vm.instruction as usize {
            if Instant::now() >= *timeout_at {
                return TraceOutcome::cutoff(TraceEnd::Timeout);
            }

            let stack_depth = vm.stack.size();
            let state = match vm.step() {
                Ok(state) => state,
                Err(e) => {
                    warn!("executing branch failed during step: {:?}", e);
                    return TraceOutcome::cutoff(TraceEnd::StepError);
                }
            };
            let last_instruction = state.last_instruction.clone();

            vm_trace.operations.push(state);
            vm_trace.gas_used = vm.gas_used;

            if last_instruction.opcode == 0x57 && vm.exitcode == 255 {
                if stack_depth < 2 {
                    return TraceOutcome::cutoff(TraceEnd::StepError);
                }
                trace!(
                    "found branch due to JUMP{} instruction at {}",
                    if last_instruction.opcode == 0x57 { "I" } else { "" },
                    last_instruction.instruction
                );

                let jump_condition: Option<String> =
                    last_instruction.input_operations.get(1).map(|op| op.solidify());
                let jump_taken =
                    last_instruction.inputs.get(1).map(|op| !op.is_zero()).unwrap_or(true);

                let jump_frame = JumpFrame::new(
                    last_instruction.instruction,
                    last_instruction.inputs[0],
                    vm.stack.size(),
                    jump_taken,
                );

                if stack_contains_too_many_items(&vm.stack) {
                    return TraceOutcome::cutoff(TraceEnd::LoopCutoff);
                }

                if stack_contains_too_many_of_the_same_item(&vm.stack) {
                    return TraceOutcome::cutoff(TraceEnd::LoopCutoff);
                }

                if stack_item_source_depth_too_deep(&vm.stack) {
                    return TraceOutcome::cutoff(TraceEnd::LoopCutoff);
                }

                if jump_stack_depth_less_than_max_stack_depth(&jump_frame, handled_jumps) {
                    return TraceOutcome::cutoff(TraceEnd::LoopCutoff);
                }

                match handled_jumps.get_mut(&jump_frame) {
                    Some(historical_stacks) => {
                        if historical_stacks.iter().any(|hist_stack| {
                            if let Some(jump_condition) = &jump_condition {

                                if hist_stack == &vm.stack {
                                    trace!(
                                        "jump matches loop-detection heuristic: 'jump_path_already_handled'"
                                    );
                                    return true
                                }

                                let stack_diff = stack_diff(&vm.stack, hist_stack);
                                if stack_diff.is_empty() {
                                    trace!(
                                        "jump matches loop-detection heuristic: 'stack_diff_is_empty'"
                                    );
                                    return true
                                }

                                trace!("stack diff: [{}]", stack_diff.iter().map(|frame| format!("{}", frame.value)).collect::<Vec<String>>().join(", "));

                                if jump_condition_appears_recursive(&stack_diff, jump_condition) {
                                    return true
                                }

                                if jump_condition_contains_mutated_memory_access(
                                    &stack_diff,
                                    jump_condition,
                                ) {
                                    return true
                                }

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

                            historical_stacks.push(vm.stack.clone());
                            return TraceOutcome::cutoff(TraceEnd::LoopCutoff);
                        }

                        if stack_position_shows_pattern(&vm.stack, historical_stacks) {
                            trace!("jump terminated.");
                            trace!(
                                "adding historical stack {} to jump frame {:?}",
                                &format!("{:#016x?}", vm.stack.hash()),
                                jump_frame
                            );

                            historical_stacks.push(vm.stack.clone());
                            return TraceOutcome::cutoff(TraceEnd::LoopCutoff);
                        }

                        if historical_diffs_approximately_equal(&vm.stack, historical_stacks) {
                            trace!("jump terminated.");
                            trace!(
                                "adding historical stack {} to jump frame {:?}",
                                &format!("{:#016x?}", vm.stack.hash()),
                                jump_frame
                            );

                            historical_stacks.push(vm.stack.clone());
                            return TraceOutcome::cutoff(TraceEnd::LoopCutoff);
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

                            historical_stacks.push(vm.stack.clone());
                        }
                    }
                    None => {
                        trace!("added new jump frame: {:?}", jump_frame);
                        handled_jumps.insert(jump_frame, vec![vm.stack.clone()]);
                    }
                }

                *branch_count += 1;
                details.end = TraceEnd::Branch;

                // Preserve the legacy exploration order: the counterfactual path first.
                let mut other = vm.clone();
                let other_taken = !jump_taken;
                let other_outcome = if other_taken {
                    match usize::try_from(last_instruction.inputs[0])
                        .ok()
                        .filter(|pc| Program::is_valid_jumpdest_in(&other.bytecode, *pc))
                    {
                        Some(pc) => {
                            other.instruction = pc as u128 + 1;
                            other.recursive_map(branch_count, handled_jumps, timeout_at)
                        }
                        None => TraceOutcome::cutoff(TraceEnd::InvalidDestination),
                    }
                } else {
                    other.instruction = last_instruction.instruction + 1;
                    other.recursive_map(branch_count, handled_jumps, timeout_at)
                };
                other_outcome.append_to(&mut vm_trace, &mut details, other_taken);
                vm.recursive_map(branch_count, handled_jumps, timeout_at).append_to(
                    &mut vm_trace,
                    &mut details,
                    jump_taken,
                );
                break;
            }

            if vm.exitcode != 255 || !vm.returndata.is_empty() {
                details.end = TraceEnd::Halt;
                break;
            }
        }

        TraceOutcome { trace: Some(vm_trace), details }
    }
}

#[cfg(test)]
mod tests {
    use super::{TraceDetails, TraceEnd, VM};
    use alloy::primitives::Address;
    use std::time::{Duration, Instant};

    fn vm(code: &[u8]) -> VM {
        VM::new(code, &[], Address::ZERO, Address::ZERO, Address::ZERO, 0, u128::MAX)
    }

    fn has_end(details: &TraceDetails, end: TraceEnd) -> bool {
        details.end == end || details.branches.iter().any(|branch| has_end(&branch.details, end))
    }

    fn shape(trace: &super::VMTrace) -> (u128, Vec<u8>, Vec<String>) {
        (
            trace.instruction,
            trace.operations.iter().map(|state| state.last_instruction.opcode).collect(),
            trace.children.iter().map(|child| format!("{:?}", shape(child))).collect(),
        )
    }

    #[test]
    fn detailed_trace_records_branch_outcomes_even_when_both_begin_with_jumpdest() {
        let code = [0x60, 1, 0x60, 7, 0x57, 0x5b, 0, 0x5b, 0];
        let (trace, details, _) =
            vm(&code).symbolic_exec_detailed(Instant::now() + Duration::from_secs(1)).unwrap();
        let (legacy, _) = vm(&code).symbolic_exec(Instant::now() + Duration::from_secs(1)).unwrap();
        assert_eq!(shape(&trace), shape(&legacy));
        assert_eq!(details.end, TraceEnd::Branch);
        assert_eq!(details.branches.len(), 2);
        for branch in details.branches {
            let child = &trace.children[branch.child_index.unwrap()];
            assert_eq!(child.operations[0].last_instruction.opcode, 0x5b);
            assert_eq!(child.instruction, if branch.taken { 8 } else { 6 });
        }
    }

    #[test]
    fn detailed_trace_reports_timeout_and_loop_cutoffs() {
        let (_, details, _) = vm(&[0]).symbolic_exec_detailed(Instant::now()).unwrap();
        assert_eq!(details.end, TraceEnd::Timeout);
        let (_, details, _) = vm(&[0x5b, 0x60, 1, 0x60, 0, 0x57])
            .symbolic_exec_detailed(Instant::now() + Duration::from_secs(1))
            .unwrap();
        assert!(has_end(&details, TraceEnd::LoopCutoff));
    }

    #[test]
    fn halted_jumpi_is_not_recorded_as_a_branch() {
        let mut vm = vm(&[0x57]);
        vm.gas_remaining = 0;
        let (_, details, branches) =
            vm.symbolic_exec_detailed(Instant::now() + Duration::from_secs(1)).unwrap();
        assert_eq!(vm.exitcode, 9);
        assert_eq!(details.end, TraceEnd::Halt);
        assert_eq!(branches, 0);
    }

    #[test]
    fn unrepresentable_counterfactual_destination_is_recorded_as_a_cutoff() {
        let mut code = vec![0x5f, 0x7f];
        code.extend([0xff; 32]);
        code.push(0x57);
        let (_, details, _) =
            vm(&code).symbolic_exec_detailed(Instant::now() + Duration::from_secs(1)).unwrap();
        let taken = details.branches.iter().find(|branch| branch.taken).unwrap();
        assert_eq!(taken.child_index, None);
        assert_eq!(taken.details.end, TraceEnd::InvalidDestination);
    }

    #[test]
    fn invalid_counterfactual_destinations_are_recorded_as_cutoffs() {
        for code in [
            vec![0x60, 0, 0x60, 0xff, 0x57, 0],
            vec![0x60, 0, 0x61, 0x5b, 0, 0x50, 0x60, 3, 0x57, 0],
        ] {
            let (trace, details, _) =
                vm(&code).symbolic_exec_detailed(Instant::now() + Duration::from_secs(1)).unwrap();
            let taken = details.branches.iter().find(|branch| branch.taken).unwrap();
            assert_eq!(taken.child_index, None);
            assert_eq!(taken.details.end, TraceEnd::InvalidDestination);
            assert_eq!(trace.children.len(), 1);
        }
    }

    #[test]
    fn incomplete_jumpi_inputs_are_reported_as_an_execution_error() {
        let (_, details, _) =
            vm(&[0x57]).symbolic_exec_detailed(Instant::now() + Duration::from_secs(1)).unwrap();
        assert_eq!(details.end, TraceEnd::StepError);
    }

    #[test]
    fn selector_entry_search_respects_deadline_during_a_loop() {
        let code = [0x5b, 0x60, 0, 0x56];
        let error = vm(&code)
            .symbolic_exec_selector(
                "deadbeef",
                u128::MAX,
                Instant::now() + Duration::from_millis(10),
            )
            .unwrap_err();
        assert!(error.to_string().contains("timed out"));
    }
}
