use heimdall_vm::{
    core::opcodes::{OpCodeInfo, JUMPI},
    ext::exec::{TraceDetails, TraceEnd, VMTrace},
};

use super::ir::Expr;

/// Result of pruning statically decidable branches from a symbolic execution trace.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct PruneStats {
    pub branches: usize,
    pub paths: usize,
}

/// Folds a branch condition to a constant when its truthiness is statically decidable.
pub(crate) fn constant_truthiness(expr: Expr) -> Option<bool> {
    match expr.simplify() {
        Expr::Bool(value) => Some(value),
        Expr::Literal(value) => Some(!value.is_zero()),
        _ => None,
    }
}

/// Whether every path below this trace ends in a terminating instruction.
///
/// Symbolic execution deduplicates revisited jumps across sibling paths, so a subtree cut short at
/// a JUMPI may rely on a sibling for the rest of its body. Such a subtree is not self-contained.
fn is_complete(trace: &VMTrace, details: Option<&TraceDetails>) -> bool {
    if let Some(details) = details {
        return match details.end {
            TraceEnd::Branch => {
                trace.children.len() == 2 &&
                    details.branches.len() == 2 &&
                    details.branches.iter().all(|branch| {
                        branch.child_index.is_some_and(|index| {
                            trace
                                .children
                                .get(index)
                                .is_some_and(|child| is_complete(child, Some(&branch.details)))
                        })
                    })
            }
            TraceEnd::Halt => {
                trace.children.is_empty() &&
                    trace.operations.last().is_some_and(|state| {
                        OpCodeInfo::from(state.last_instruction.opcode).terminating()
                    })
            }
            _ => false,
        };
    }
    match trace.operations.last().map(|state| state.last_instruction.opcode) {
        Some(JUMPI) => {
            trace.children.len() == 2 && trace.children.iter().all(|child| is_complete(child, None))
        }
        Some(opcode) => trace.children.is_empty() && OpCodeInfo::from(opcode).terminating(),
        None => false,
    }
}

pub(crate) fn is_bare_reverting(trace: &VMTrace) -> bool {
    if trace.children.is_empty() {
        return trace.operations.last().is_some_and(|state| {
            state.last_instruction.opcode == heimdall_vm::core::opcodes::REVERT &&
                state.last_instruction.inputs.get(1).is_some_and(|size| size.is_zero())
        });
    }
    trace.children.iter().all(is_bare_reverting)
}

fn contains_opcode(trace: &VMTrace, opcode: u8) -> bool {
    trace.operations.iter().any(|state| state.last_instruction.opcode == opcode) ||
        trace.children.iter().any(|child| contains_opcode(child, opcode))
}

/// Remove the infeasible child of each JUMPI whose condition is provably constant.
///
/// Symbolic execution intentionally explores both children. This pass uses expression identity and
/// local folding to recover cases such as `msg.sender == msg.sender` without treating concrete
/// placeholder calldata values as constants. The infeasible child is only dropped when the feasible
/// child is complete, since a truncated feasible child may share its continuation with the sibling.
#[cfg(test)]
fn prune_constant_branches(trace: &mut VMTrace) -> PruneStats {
    prune_with_details(trace, None)
}

pub(crate) fn prune_constant_branches_with_details(
    trace: &mut VMTrace,
    details: &mut TraceDetails,
) -> PruneStats {
    prune_with_details(trace, Some(details))
}

fn prune_with_details(trace: &mut VMTrace, mut details: Option<&mut TraceDetails>) -> PruneStats {
    let mut stats = PruneStats::default();

    if let Some(state) = trace.operations.last() {
        let instruction = &state.last_instruction;
        if instruction.opcode == JUMPI {
            let truthiness = instruction
                .input_operations
                .get(1)
                .map(Expr::from_opcode)
                .and_then(constant_truthiness);
            if let Some(taken) = truthiness {
                let jump_destination = instruction
                    .inputs
                    .first()
                    .and_then(|destination| u128::try_from(*destination).ok())
                    .map(|destination| destination.saturating_add(1));
                let fallthrough = instruction.instruction.saturating_add(1);
                let expected = if taken { jump_destination } else { Some(fallthrough) };

                let feasible_is_complete = expected.is_some_and(|expected| {
                    let mut feasible = trace
                        .children
                        .iter()
                        .enumerate()
                        .filter(|(_, child)| child.instruction == expected)
                        .peekable();
                    feasible.peek().is_some() &&
                        feasible.all(|(index, child)| {
                            let child_details = details.as_deref().and_then(|details| {
                                details
                                    .branches
                                    .iter()
                                    .find(|branch| branch.child_index == Some(index))
                                    .map(|branch| branch.details.as_ref())
                            });
                            is_complete(child, child_details)
                        })
                });
                // Return paths also determine the recovered ABI. Preserve a nominally infeasible
                // child when it is the only path exposing a RETURN; concrete placeholder values
                // can otherwise make a live return look constant during symbolic execution.
                let drops_unique_return = expected.is_some_and(|expected| {
                    let feasible_has_return =
                        trace.children.iter().filter(|child| child.instruction == expected).any(
                            |child| contains_opcode(child, heimdall_vm::core::opcodes::RETURN),
                        );
                    let discarded_has_return =
                        trace.children.iter().filter(|child| child.instruction != expected).any(
                            |child| contains_opcode(child, heimdall_vm::core::opcodes::RETURN),
                        );
                    discarded_has_return && !feasible_has_return
                });

                if let (Some(expected), true, false) =
                    (expected, feasible_is_complete, drops_unique_return)
                {
                    let old_len = trace.children.len();
                    let retained: Vec<_> = trace
                        .children
                        .iter()
                        .enumerate()
                        .filter_map(|(index, child)| {
                            (child.instruction == expected).then_some(index)
                        })
                        .collect();
                    if let Some(details) = details.as_deref_mut() {
                        details.branches.retain_mut(|branch| match branch.child_index {
                            Some(old) => match retained.iter().position(|index| *index == old) {
                                Some(new) => {
                                    branch.child_index = Some(new);
                                    true
                                }
                                None => false,
                            },
                            None => true,
                        });
                    }
                    trace.children.retain(|child| child.instruction == expected);
                    let removed = old_len.saturating_sub(trace.children.len());
                    if removed > 0 {
                        stats.branches += 1;
                        stats.paths += removed;
                    }
                }
            }
        }
    }

    for (index, child) in trace.children.iter_mut().enumerate() {
        let child_details = details.as_deref_mut().and_then(|details| {
            details
                .branches
                .iter_mut()
                .find(|branch| branch.child_index == Some(index))
                .map(|branch| branch.details.as_mut())
        });
        let child_stats = prune_with_details(child, child_details);
        stats.branches += child_stats.branches;
        stats.paths += child_stats.paths;
    }
    stats
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use alloy::primitives::U256;
    use heimdall_vm::{
        core::{
            memory::Memory,
            opcodes::{self, WrappedInput, WrappedOpcode},
            stack::Stack,
            storage::Storage,
            vm::{Instruction, State},
        },
        ext::exec::{TraceBranch, TraceDetails, TraceEnd, VMTrace},
    };

    use super::*;

    fn caller() -> WrappedInput {
        WrappedInput::Opcode(Arc::new(WrappedOpcode::new(opcodes::CALLER, vec![])))
    }

    fn state(instruction: u128, opcode: u8, input_operations: Vec<WrappedOpcode>) -> State {
        State {
            last_instruction: Instruction {
                instruction,
                opcode,
                inputs: vec![U256::from(20), U256::from(1)],
                outputs: vec![],
                input_operations,
                output_operations: vec![],
            },
            gas_used: 0,
            gas_remaining: 0,
            stack: Stack::new(),
            memory: Memory::new(),
            storage: Storage::new(),
            events: vec![],
        }
    }

    fn leaf(instruction: u128, opcode: u8) -> VMTrace {
        VMTrace {
            instruction,
            gas_used: 0,
            operations: vec![state(instruction, opcode, vec![])],
            children: vec![],
        }
    }

    fn trace_with_condition(condition: WrappedOpcode, taken: VMTrace) -> VMTrace {
        VMTrace {
            instruction: 1,
            gas_used: 0,
            operations: vec![state(
                10,
                opcodes::JUMPI,
                vec![WrappedOpcode::new(opcodes::PUSH1, vec![U256::from(20).into()]), condition],
            )],
            children: vec![leaf(11, opcodes::STOP), taken],
        }
    }

    #[test]
    fn identifies_only_all_bare_revert_subtrees() {
        let mut bare_revert = leaf(2, opcodes::REVERT);
        bare_revert.operations[0].last_instruction.inputs[1] = U256::ZERO;
        let nested_reverts = VMTrace {
            instruction: 1,
            gas_used: 0,
            operations: vec![state(1, opcodes::JUMPI, vec![])],
            children: vec![bare_revert.clone(), bare_revert],
        };
        assert!(is_bare_reverting(&nested_reverts));

        let mixed = VMTrace {
            children: vec![leaf(2, opcodes::REVERT), leaf(3, opcodes::RETURN)],
            ..nested_reverts
        };
        assert!(!is_bare_reverting(&mixed));
    }

    #[test]
    fn keeps_only_taken_path_for_identical_equality() {
        let condition = WrappedOpcode::new(opcodes::EQ, vec![caller(), caller()]);
        let mut trace = trace_with_condition(condition, leaf(21, opcodes::STOP));
        let stats = prune_constant_branches(&mut trace);
        assert_eq!(stats, PruneStats { branches: 1, paths: 1 });
        assert_eq!(trace.children.len(), 1);
        assert_eq!(trace.children[0].instruction, 21);
    }

    #[test]
    fn pruning_keeps_branch_metadata_aligned_with_retained_children() {
        let condition = WrappedOpcode::new(opcodes::EQ, vec![caller(), caller()]);
        let mut trace = trace_with_condition(condition, leaf(21, opcodes::STOP));
        let leaf_details = || Box::new(TraceDetails { end: TraceEnd::Halt, branches: vec![] });
        let mut details = TraceDetails {
            end: TraceEnd::Branch,
            branches: vec![
                TraceBranch { taken: false, child_index: Some(0), details: leaf_details() },
                TraceBranch { taken: true, child_index: Some(1), details: leaf_details() },
            ],
        };
        let stats = prune_constant_branches_with_details(&mut trace, &mut details);
        assert_eq!(stats.paths, 1);
        assert_eq!(details.branches.len(), 1);
        assert!(details.branches[0].taken);
        assert_eq!(details.branches[0].child_index, Some(0));
        assert_eq!(trace.children[0].instruction, 21);
    }

    #[test]
    fn cutoff_details_prevent_a_trace_from_being_treated_as_complete() {
        let trace = leaf(1, opcodes::STOP);
        let details = TraceDetails { end: TraceEnd::Timeout, branches: vec![] };
        assert!(!is_complete(&trace, Some(&details)));
    }

    #[test]
    fn keeps_both_paths_when_taken_path_is_truncated() {
        // The executor deduplicates revisited jumps, so a taken path that stops at a JUMPI without
        // children relies on its sibling for the rest of the function body.
        let condition = WrappedOpcode::new(opcodes::EQ, vec![caller(), caller()]);
        let mut trace = trace_with_condition(condition, leaf(21, opcodes::JUMPI));
        let stats = prune_constant_branches(&mut trace);
        assert_eq!(stats, PruneStats::default());
        assert_eq!(trace.children.len(), 2);
    }

    #[test]
    fn keeps_unique_return_path_for_abi_recovery() {
        let condition = WrappedOpcode::new(opcodes::EQ, vec![caller(), caller()]);
        let mut trace = trace_with_condition(condition, leaf(21, opcodes::REVERT));
        trace.children[0] = leaf(11, opcodes::RETURN);
        let stats = prune_constant_branches(&mut trace);
        assert_eq!(stats, PruneStats::default());
        assert_eq!(trace.children.len(), 2);
    }

    #[test]
    fn keeps_both_paths_for_symbolic_condition() {
        let condition = WrappedOpcode::new(opcodes::CALLDATALOAD, vec![U256::from(4).into()]);
        let mut trace = trace_with_condition(condition, leaf(21, opcodes::STOP));
        let stats = prune_constant_branches(&mut trace);
        assert_eq!(stats, PruneStats::default());
        assert_eq!(trace.children.len(), 2);
    }
}
