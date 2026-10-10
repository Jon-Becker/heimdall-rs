//! Opcode transfer semantics for abstract basic-block execution.

use alloy::primitives::U256;

use crate::core::{
    opcodes::{self, OpCodeInfo},
    program::{BlockId, BlockTerminator, Program},
    stack::AbstractValue,
    state::AbstractState,
};

#[derive(Clone, Debug)]
pub(super) struct BlockExit {
    pub(super) state: AbstractState,
    pub(super) jump_target: Option<AbstractValue>,
    pub(super) condition: Option<AbstractValue>,
}

pub(super) fn execute_block(
    program: &Program,
    block_id: BlockId,
    mut state: AbstractState,
) -> Option<BlockExit> {
    let block = &program.blocks[block_id.index()];
    let instructions = program.block_instructions(block_id);
    let mut jump_target = None;
    let mut condition = None;

    for instruction in instructions {
        match instruction.opcode {
            opcodes::JUMP => jump_target = state.stack.pop(),
            opcodes::JUMPI => {
                jump_target = state.stack.pop();
                condition = state.stack.pop();
                if jump_target.is_none() || condition.is_none() {
                    return None
                }
            }
            opcodes::PUSH0 => state.stack.push(AbstractValue::constant(U256::ZERO)),
            opcodes::PUSH1..=opcodes::PUSH32 => {
                state.stack.push(AbstractValue::constant(instruction.push_value()?));
            }
            opcodes::POP => {
                state.stack.pop()?;
            }
            opcodes::DUP1..=opcodes::DUP16 => {
                let index = (instruction.opcode - opcodes::DUP1) as usize;
                state.stack.push(state.stack.peek(index)?);
            }
            opcodes::SWAP1..=opcodes::SWAP16 => {
                let index = (instruction.opcode - opcodes::SWAP1 + 1) as usize;
                if !state.stack.swap(index) {
                    return None
                }
            }
            opcodes::PC => state.stack.push(AbstractValue::constant(U256::from(instruction.pc))),
            opcodes::CODESIZE => {
                state.stack.push(AbstractValue::constant(U256::from(program.bytecode.len())));
            }
            opcodes::JUMPDEST => {}
            opcode => {
                let info = OpCodeInfo::from(opcode);
                if !state.stack.pop_n(info.inputs() as usize) {
                    return None
                }
                for _ in 0..info.outputs() {
                    state.stack.push(AbstractValue::Unknown);
                }
            }
        }
    }

    if matches!(block.terminator, BlockTerminator::Jump | BlockTerminator::ConditionalJump) &&
        jump_target.is_none()
    {
        return None
    }

    Some(BlockExit { state, jump_target, condition })
}
