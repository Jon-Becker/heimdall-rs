use heimdall_vm::ext::exec::VMTrace;

use super::ir::Expr;

/// Result of pruning statically decidable branches from a symbolic execution trace.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct PruneStats {
    pub branches: usize,
    pub paths: usize,
}

fn constant_truthiness(expr: Expr) -> Option<bool> {
    match expr.simplify() {
        Expr::Bool(value) => Some(value),
        Expr::Literal(value) => Some(!value.is_zero()),
        _ => None,
    }
}

/// Remove the infeasible child of each JUMPI whose condition is provably constant.
///
/// Symbolic execution intentionally explores both children. This pass uses expression identity and
/// local folding to recover cases such as `msg.sender == msg.sender` without treating concrete
/// placeholder calldata values as constants.
pub(crate) fn prune_constant_branches(trace: &mut VMTrace) -> PruneStats {
    let mut stats = PruneStats::default();

    if let Some(state) = trace.operations.last() {
        let instruction = &state.last_instruction;
        if instruction.opcode == 0x57 {
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

                if let Some(expected) = expected {
                    let old_len = trace.children.len();
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

    for child in &mut trace.children {
        let child_stats = prune_constant_branches(child);
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
        ext::exec::VMTrace,
    };

    use super::*;

    fn caller() -> WrappedInput {
        WrappedInput::Opcode(Arc::new(WrappedOpcode::new(opcodes::CALLER, vec![])))
    }

    fn trace_with_condition(condition: WrappedOpcode) -> VMTrace {
        VMTrace {
            instruction: 1,
            gas_used: 0,
            operations: vec![State {
                last_instruction: Instruction {
                    instruction: 10,
                    opcode: opcodes::JUMPI,
                    inputs: vec![U256::from(20), U256::from(1)],
                    outputs: vec![],
                    input_operations: vec![
                        WrappedOpcode::new(opcodes::PUSH1, vec![U256::from(20).into()]),
                        condition,
                    ],
                    output_operations: vec![],
                },
                gas_used: 0,
                gas_remaining: 0,
                stack: Stack::new(),
                memory: Memory::new(),
                storage: Storage::new(),
                events: vec![],
            }],
            children: vec![
                VMTrace { instruction: 11, gas_used: 0, operations: vec![], children: vec![] },
                VMTrace { instruction: 21, gas_used: 0, operations: vec![], children: vec![] },
            ],
        }
    }

    #[test]
    fn keeps_only_taken_path_for_identical_equality() {
        let condition = WrappedOpcode::new(opcodes::EQ, vec![caller(), caller()]);
        let mut trace = trace_with_condition(condition);
        let stats = prune_constant_branches(&mut trace);
        assert_eq!(stats, PruneStats { branches: 1, paths: 1 });
        assert_eq!(trace.children.len(), 1);
        assert_eq!(trace.children[0].instruction, 21);
    }

    #[test]
    fn keeps_both_paths_for_symbolic_condition() {
        let condition = WrappedOpcode::new(opcodes::CALLDATALOAD, vec![U256::from(4).into()]);
        let mut trace = trace_with_condition(condition);
        let stats = prune_constant_branches(&mut trace);
        assert_eq!(stats, PruneStats::default());
        assert_eq!(trace.children.len(), 2);
    }
}
