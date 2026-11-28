use alloy::primitives::U256;
use hashbrown::HashSet;

use crate::core::opcodes::{WrappedInput, WrappedOpcode, ADD, CALLDATALOAD, CALLDATASIZE};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(super) struct JumpFrame {
    pub pc: u128,
    pub jumpdest: U256,
    pub stack_depth: usize,
    pub jump_taken: bool,
    pub calldata_heads: Vec<U256>,
}

impl JumpFrame {
    pub(super) fn new(
        pc: u128,
        jumpdest: U256,
        stack_depth: usize,
        jump_taken: bool,
        condition: Option<&WrappedOpcode>,
    ) -> Self {
        Self {
            pc,
            jumpdest,
            stack_depth,
            jump_taken,
            calldata_heads: condition.map(calldata_heads).unwrap_or_default(),
        }
    }
}

/// Solidity reuses ABI decoder helpers for different arguments. Their bounds checks are not
/// loop iterations when the calldata head supplying the tail pointer has changed. Keep that
/// identity stable as element indices or nested tail offsets change within a single argument.
fn calldata_heads(condition: &WrappedOpcode) -> Vec<U256> {
    let mut heads = Vec::new();
    let mut pending = vec![(condition, false)];
    let mut visited = HashSet::new();
    let mut checks_calldata_size = false;
    while let Some((operation, indirect)) = pending.pop() {
        if !visited.insert((operation as *const WrappedOpcode, indirect)) {
            continue;
        }
        checks_calldata_size |= operation.opcode == CALLDATASIZE;
        if operation.opcode == ADD && operation.inputs.len() == 2 {
            for (base, offset) in [(0, 1), (1, 0)] {
                if constant(&operation.inputs[base]) != Some(U256::from(4)) {
                    continue;
                }
                if let WrappedInput::Opcode(offset) = &operation.inputs[offset] {
                    if offset.opcode == CALLDATALOAD {
                        if let Some(head) = offset.inputs.first().and_then(constant) {
                            if head >= U256::from(4) &&
                                (head - U256::from(4)) % U256::from(32) == U256::ZERO
                            {
                                heads.push((head, indirect));
                            }
                        }
                    }
                }
            }
        }
        pending.extend(operation.inputs.iter().filter_map(|input| match input {
            WrappedInput::Opcode(input) => {
                Some((input.as_ref(), indirect || operation.opcode == CALLDATALOAD))
            }
            WrappedInput::Raw(_) => None,
        }));
    }
    // An addition to a scalar argument is not sufficient evidence of a tail pointer.
    let mut heads: Vec<_> = heads
        .into_iter()
        .filter_map(|(head, indirect)| (checks_calldata_size || indirect).then_some(head))
        .collect();
    heads.sort_unstable();
    heads.dedup();
    heads
}

fn constant(input: &WrappedInput) -> Option<U256> {
    match input {
        WrappedInput::Raw(value) => Some(*value),
        WrappedInput::Opcode(operation) if (0x60..=0x7f).contains(&operation.opcode) => {
            match operation.inputs.as_slice() {
                [WrappedInput::Raw(value)] => Some(*value),
                _ => None,
            }
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::opcodes::{CALLDATASIZE, GT, LT, PUSH1, PUSH32};

    fn literal(value: u64) -> WrappedInput {
        WrappedOpcode::new(PUSH32, vec![U256::from(value).into()]).into()
    }

    fn tail(head: u64) -> WrappedOpcode {
        WrappedOpcode::new(
            ADD,
            vec![literal(4), WrappedOpcode::new(CALLDATALOAD, vec![literal(head)]).into()],
        )
    }

    fn bounds(head: u64, index: u64) -> WrappedOpcode {
        WrappedOpcode::new(
            GT,
            vec![
                WrappedOpcode::new(ADD, vec![tail(head).into(), literal(index)]).into(),
                WrappedOpcode::new(CALLDATASIZE, vec![]).into(),
            ],
        )
    }

    fn frame(condition: &WrappedOpcode) -> JumpFrame {
        JumpFrame::new(100, U256::from(200), 8, false, Some(condition))
    }

    #[test]
    fn decoder_guards_for_distinct_arguments_have_separate_histories() {
        assert_ne!(frame(&bounds(36, 31)), frame(&bounds(68, 31)));
    }

    #[test]
    fn changing_element_offsets_keeps_the_same_loop_history() {
        assert_eq!(frame(&bounds(36, 32)), frame(&bounds(36, 64)));
        let nested = WrappedOpcode::new(
            GT,
            vec![WrappedOpcode::new(CALLDATALOAD, vec![tail(36).into()]).into(), literal(u64::MAX)],
        );
        assert_eq!(frame(&bounds(36, 32)), frame(&nested));
    }

    #[test]
    fn ordinary_calldata_loads_do_not_partition_loop_history() {
        let condition = |head| {
            WrappedOpcode::new(
                LT,
                vec![WrappedOpcode::new(CALLDATALOAD, vec![literal(head)]).into(), literal(100)],
            )
        };
        assert_eq!(frame(&condition(36)), frame(&condition(68)));
        assert!(frame(&tail(36)).calldata_heads.is_empty());
        assert!(frame(&tail(68)).calldata_heads.is_empty());
        assert!(frame(&bounds(0, 31)).calldata_heads.is_empty());
        assert!(frame(&bounds(5, 31)).calldata_heads.is_empty());
    }

    #[test]
    fn head_identity_ignores_push_width_and_add_order() {
        let pointer = WrappedOpcode::new(
            ADD,
            vec![
                WrappedOpcode::new(
                    CALLDATALOAD,
                    vec![WrappedOpcode::new(PUSH1, vec![U256::from(36).into()]).into()],
                )
                .into(),
                U256::from(4).into(),
            ],
        );
        let load = WrappedOpcode::new(CALLDATALOAD, vec![pointer.into()]);
        assert_eq!(frame(&bounds(36, 31)), frame(&load));
    }
}
