//! Recognize ABI heads and length-delimited tails from symbolic calldata expressions.

use alloy::primitives::U256;
use heimdall_vm::core::{
    opcodes::{self, WrappedInput, WrappedOpcode},
    vm::Instruction,
};

use crate::{core::types::SolidityType, interfaces::AnalyzedFunction};

fn constant(input: &WrappedInput) -> Option<U256> {
    match input {
        WrappedInput::Raw(value) => Some(*value),
        WrappedInput::Opcode(op) => constant_op(op),
    }
}

fn constant_op(op: &WrappedOpcode) -> Option<U256> {
    match op.opcode {
        opcodes::PUSH0 => Some(U256::ZERO),
        opcodes::PUSH1..=opcodes::PUSH32 => constant(op.inputs.first()?),
        opcodes::ADD => {
            Some(constant(op.inputs.first()?)?.wrapping_add(constant(op.inputs.get(1)?)?))
        }
        opcodes::SUB => {
            Some(constant(op.inputs.first()?)?.wrapping_sub(constant(op.inputs.get(1)?)?))
        }
        _ => None,
    }
}

/// Only aligned, statically known offsets beyond the selector are ABI head words.
pub(super) fn head_index(offset: &WrappedOpcode) -> Option<usize> {
    slot_index(constant_op(offset)?)
}

fn slot_index(offset: U256) -> Option<usize> {
    let offset = offset.checked_sub(U256::from(4))?;
    if offset % U256::from(32) != U256::ZERO {
        return None;
    }
    (offset / U256::from(32)).try_into().ok()
}

fn head_load(input: &WrappedInput) -> Option<usize> {
    let WrappedInput::Opcode(op) = input else { return None };
    if op.opcode != opcodes::CALLDATALOAD {
        return None;
    }
    slot_index(constant(op.inputs.first()?)?)
}

/// Flatten addition without substituting the executor's concrete (usually zero) values.
fn add_terms<'a>(input: &'a WrappedInput, offset: &mut U256, terms: &mut Vec<&'a WrappedInput>) {
    if let Some(value) = constant(input) {
        *offset = offset.wrapping_add(value);
    } else if let WrappedInput::Opcode(op) = input {
        if op.opcode == opcodes::ADD && op.inputs.len() == 2 {
            for input in &op.inputs {
                add_terms(input, offset, terms);
            }
        } else {
            terms.push(input);
        }
    }
}

fn tail_head(input: &WrappedInput, displacement: u64) -> Option<usize> {
    let mut offset = U256::ZERO;
    let mut terms = Vec::new();
    add_terms(input, &mut offset, &mut terms);
    if offset == U256::from(displacement) && terms.len() == 1 {
        head_load(terms[0])
    } else {
        None
    }
}

fn length_head(input: &WrappedInput) -> Option<usize> {
    let WrappedInput::Opcode(op) = input else { return None };
    if op.opcode != opcodes::CALLDATALOAD {
        return None;
    }
    tail_head(op.inputs.first()?, 4)
}

/// The decoder bounds check distinguishes byte lengths from word counts. Unused array element
/// types (and nested array offsets) are indistinguishable, so word arrays default to uint256[].
fn length_shape(input: &WrappedInput) -> Option<(usize, SolidityType)> {
    if let Some(index) = length_head(input) {
        return Some((index, SolidityType::Bytes));
    }
    let WrappedInput::Opcode(op) = input else { return None };
    let (lhs, rhs) = (op.inputs.first()?, op.inputs.get(1)?);
    let (length, word_sized) = match op.opcode {
        opcodes::SHL if constant(lhs) == Some(U256::from(5)) => (rhs, true),
        opcodes::MUL if constant(lhs) == Some(U256::from(32)) => (rhs, true),
        opcodes::MUL if constant(rhs) == Some(U256::from(32)) => (lhs, true),
        opcodes::MUL if constant(lhs) == Some(U256::from(1)) => (rhs, false),
        opcodes::MUL if constant(rhs) == Some(U256::from(1)) => (lhs, false),
        _ => return None,
    };
    let ty = if word_sized {
        SolidityType::Array { element: Box::new(SolidityType::Uint(256)), length: None }
    } else {
        SolidityType::Bytes
    };
    Some((length_head(length)?, ty))
}

fn bounded_tail(input: &WrappedOpcode) -> Option<(usize, SolidityType)> {
    let input = WrappedInput::from(input.clone());
    let mut offset = U256::ZERO;
    let mut terms = Vec::new();
    add_terms(&input, &mut offset, &mut terms);
    // selector + length word + relative tail offset + length * element size
    if offset != U256::from(36) || terms.len() != 2 {
        return None;
    }
    for (head, length) in [(terms[0], terms[1]), (terms[1], terms[0])] {
        if let (Some(head), Some((index, ty))) = (head_load(head), length_shape(length)) {
            if head == index {
                return Some((index, ty));
            }
        }
    }
    None
}

pub(super) fn infer_dynamic_arguments(function: &mut AnalyzedFunction, instruction: &Instruction) {
    let ops = &instruction.input_operations;
    let inferred = match instruction.opcode {
        opcodes::LT | opcodes::GT | opcodes::SLT | opcodes::SGT if ops.len() == 2 => {
            if ops[0].opcode == opcodes::CALLDATASIZE {
                bounded_tail(&ops[1])
            } else if ops[1].opcode == opcodes::CALLDATASIZE {
                bounded_tail(&ops[0])
            } else {
                None
            }
        }
        opcodes::CALLDATACOPY if ops.len() == 3 => {
            let source = WrappedInput::from(ops[1].clone());
            let size = WrappedInput::from(ops[2].clone());
            tail_head(&source, 36)
                .and_then(|head| length_shape(&size).filter(|(index, _)| head == *index))
        }
        _ => None,
    };
    if let Some((index, ty)) = inferred {
        if let Some(frame) = function.arguments.get_mut(&index) {
            // Copy lengths can describe only part of an array. Decoder bounds carry
            // stronger evidence than a later copy of bytes from the same tail.
            if instruction.opcode != opcodes::CALLDATACOPY || frame.dynamic_type.is_none() {
                frame.dynamic_type = Some(ty);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interfaces::CalldataFrame;
    use hashbrown::HashSet;

    fn literal(value: u64) -> WrappedOpcode {
        WrappedOpcode::new(opcodes::PUSH32, vec![U256::from(value).into()])
    }

    fn op(opcode: u8, inputs: Vec<WrappedOpcode>) -> WrappedOpcode {
        WrappedOpcode::new(opcode, inputs.into_iter().map(Into::into).collect())
    }

    fn head(index: u64) -> WrappedOpcode {
        op(opcodes::CALLDATALOAD, vec![literal(4 + index * 32)])
    }

    fn length(index: u64) -> WrappedOpcode {
        op(opcodes::CALLDATALOAD, vec![op(opcodes::ADD, vec![head(index), literal(4)])])
    }

    fn function() -> AnalyzedFunction {
        let mut function = AnalyzedFunction::new("12345678", false);
        for index in 0..2 {
            function.arguments.insert(
                index,
                CalldataFrame {
                    arg_op: literal(4 + index as u64 * 32).to_string(),
                    mask_size: 32,
                    heuristics: HashSet::new(),
                    dynamic_type: None,
                },
            );
        }
        function
    }

    fn instruction(opcode: u8, input_operations: Vec<WrappedOpcode>) -> Instruction {
        Instruction {
            instruction: 0,
            opcode,
            inputs: vec![U256::ZERO; input_operations.len()],
            outputs: Vec::new(),
            input_operations,
            output_operations: Vec::new(),
        }
    }

    #[test]
    fn head_offsets_require_constant_aligned_words_beyond_the_selector() {
        assert_eq!(head_index(&literal(4)), Some(0));
        assert_eq!(head_index(&op(opcodes::ADD, vec![literal(64), literal(4)])), Some(2));
        for offset in [0, 1, 3, 5, 35] {
            assert_eq!(head_index(&literal(offset)), None);
        }
        assert_eq!(head_index(&op(opcodes::ADD, vec![literal(4), head(0)])), None);
        assert_eq!(head_index(&WrappedOpcode::new(opcodes::PUSH32, vec![U256::MAX.into()])), None);
    }

    #[test]
    fn bounds_checks_distinguish_byte_lengths_from_word_counts() {
        let array = SolidityType::parse("uint256[]");
        for (size, expected) in [
            (length(1), SolidityType::Bytes),
            (op(opcodes::MUL, vec![literal(1), length(1)]), SolidityType::Bytes),
            (op(opcodes::MUL, vec![length(1), literal(1)]), SolidityType::Bytes),
            (op(opcodes::SHL, vec![literal(5), length(1)]), array.clone()),
            (op(opcodes::MUL, vec![length(1), literal(32)]), array.clone()),
            (op(opcodes::MUL, vec![literal(32), length(1)]), array),
        ] {
            for reverse in [false, true] {
                let end = op(
                    opcodes::ADD,
                    vec![size.clone(), op(opcodes::ADD, vec![literal(36), head(1)])],
                );
                let calldata_size = op(opcodes::CALLDATASIZE, vec![]);
                let inputs =
                    if reverse { vec![calldata_size, end] } else { vec![end, calldata_size] };
                let mut function = function();
                infer_dynamic_arguments(&mut function, &instruction(opcodes::GT, inputs));
                assert_eq!(function.arguments[&1].dynamic_type, Some(expected.clone()));
                assert_eq!(function.arguments[&0].dynamic_type, None);
            }
        }
    }

    #[test]
    fn requires_matching_tail_length_and_calldata_bound() {
        for (offset, size, bound) in [
            (36, length(0), op(opcodes::CALLDATASIZE, vec![])),
            (4, length(1), op(opcodes::CALLDATASIZE, vec![])),
            (36, head(1), op(opcodes::CALLDATASIZE, vec![])),
            (36, length(1), literal(4096)),
        ] {
            let mut function = function();
            let end =
                op(opcodes::ADD, vec![head(1), op(opcodes::ADD, vec![literal(offset), size])]);
            infer_dynamic_arguments(&mut function, &instruction(opcodes::GT, vec![end, bound]));
            assert_eq!(function.arguments[&1].dynamic_type, None);
        }
    }

    #[test]
    fn calldata_copy_recovers_bytes_without_using_concrete_zero_size() {
        let mut function = function();
        let source = op(opcodes::ADD, vec![head(1), literal(36)]);
        infer_dynamic_arguments(
            &mut function,
            &instruction(opcodes::CALLDATACOPY, vec![literal(128), source.clone(), length(0)]),
        );
        assert_eq!(function.arguments[&1].dynamic_type, None);
        infer_dynamic_arguments(
            &mut function,
            &instruction(opcodes::CALLDATACOPY, vec![literal(128), source, length(1)]),
        );
        assert_eq!(
            function.arguments[&1].potential_types(),
            vec![SolidityType::Bytes, SolidityType::String]
        );
    }

    #[test]
    fn partial_copies_do_not_override_decoder_shapes() {
        for (bounded_size, copied_size, expected) in [
            (
                op(opcodes::SHL, vec![literal(5), length(1)]),
                length(1),
                SolidityType::parse("uint256[]"),
            ),
            (length(1), op(opcodes::MUL, vec![literal(32), length(1)]), SolidityType::Bytes),
        ] {
            let mut function = function();
            let end =
                op(opcodes::ADD, vec![head(1), op(opcodes::ADD, vec![literal(36), bounded_size])]);
            infer_dynamic_arguments(
                &mut function,
                &instruction(opcodes::GT, vec![end, op(opcodes::CALLDATASIZE, vec![])]),
            );
            infer_dynamic_arguments(
                &mut function,
                &instruction(
                    opcodes::CALLDATACOPY,
                    vec![literal(128), op(opcodes::ADD, vec![head(1), literal(36)]), copied_size],
                ),
            );
            assert_eq!(function.arguments[&1].dynamic_type, Some(expected));
        }
    }
}
