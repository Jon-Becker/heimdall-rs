//! Canonical decoding and basic-block recovery for EVM bytecode.
//!
//! This module provides a structural front end that is independent of concrete VM execution. It
//! decodes PUSH immediates once, identifies every basic-block boundary, validates direct jump
//! targets, and records the statically-known CFG edges. Dynamic jump edges are intentionally left
//! unresolved for a later abstract or symbolic analysis pass.

use std::{collections::HashMap, ops::Range};

use alloy::primitives::U256;

use super::{
    hardfork::HardFork,
    opcodes::{self, OpCodeInfo},
};

/// The stable identifier of a basic block within a [`Program`].
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct BlockId(usize);

impl BlockId {
    /// Return this block's zero-based index in [`Program::blocks`].
    pub fn index(self) -> usize {
        self.0
    }
}

/// A decoded EVM instruction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedInstruction {
    /// Zero-based byte offset of the opcode.
    pub pc: usize,
    /// Raw opcode byte.
    pub opcode: u8,
    /// Bytes following a PUSH opcode that belong to its immediate operand.
    pub immediate: Vec<u8>,
    /// Whether a PUSH operand was truncated by the end of the bytecode.
    pub truncated: bool,
}

impl DecodedInstruction {
    /// Number of bytecode bytes occupied by this instruction.
    pub fn size(&self) -> usize {
        1 + self.immediate.len()
    }

    /// Byte offset immediately after this instruction.
    pub fn next_pc(&self) -> usize {
        self.pc + self.size()
    }

    /// Returns the PUSH operand as a 256-bit value, or `None` for non-PUSH instructions.
    ///
    /// A truncated operand is right-padded with zero bytes, matching the EVM's code-reading
    /// semantics at the end of bytecode.
    pub fn push_value(&self) -> Option<U256> {
        let width = push_width(self.opcode)?;
        let mut bytes = [0u8; 32];
        bytes[32 - width..32 - width + self.immediate.len()].copy_from_slice(&self.immediate);
        Some(U256::from_be_bytes(bytes))
    }
}

/// The control-transfer behavior that terminates a basic block.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BlockTerminator {
    /// Execution continues into the next basic block.
    Fallthrough,
    /// An unconditional jump whose destination is read from the stack.
    Jump,
    /// A conditional jump with a statically-known fallthrough.
    ConditionalJump,
    /// An opcode that terminates execution.
    Halt,
    /// An unknown or hardfork-inactive opcode that terminates this execution path.
    Invalid,
}

/// The kind of a statically-known control-flow edge.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EdgeKind {
    /// Ordinary sequential control flow between adjacent basic blocks.
    Fallthrough,
    /// The condition of a JUMPI is false.
    ConditionalFalse,
    /// A JUMP or the true side of a JUMPI has a locally constant destination.
    Jump,
}

/// A statically-known edge between basic blocks.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BlockEdge {
    /// Destination block.
    pub target: BlockId,
    /// Control-flow relationship represented by this edge.
    pub kind: EdgeKind,
}

/// A maximal sequence of instructions with one entry and no internal control transfer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BasicBlock {
    /// Stable block identifier.
    pub id: BlockId,
    /// Zero-based byte offset of the first instruction.
    pub start_pc: usize,
    /// Byte offset immediately after the final instruction.
    pub end_pc: usize,
    /// Range into [`Program::instructions`] containing this block's instructions.
    pub instructions: Range<usize>,
    /// How control leaves this block.
    pub terminator: BlockTerminator,
    /// Edges that can be established without whole-program value-flow analysis.
    pub static_edges: Vec<BlockEdge>,
}

/// A canonically decoded EVM program and its execution-independent basic-block CFG skeleton.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Program {
    /// Original bytecode.
    pub bytecode: Vec<u8>,
    /// Decoded instructions in byte-offset order.
    pub instructions: Vec<DecodedInstruction>,
    /// Canonical basic blocks in byte-offset order.
    pub blocks: Vec<BasicBlock>,
    block_by_pc: HashMap<usize, BlockId>,
}

impl Program {
    /// Decode bytecode and recover canonical basic blocks for the selected hardfork.
    pub fn decode(bytecode: &[u8], hardfork: HardFork) -> Self {
        let instructions = decode_instructions(bytecode);
        let mut blocks = build_blocks(&instructions, hardfork);
        let block_by_pc = blocks.iter().map(|block| (block.start_pc, block.id)).collect();
        add_static_edges(&instructions, &mut blocks, &block_by_pc);

        Self { bytecode: bytecode.to_vec(), instructions, blocks, block_by_pc }
    }

    /// Find the basic block that starts at `pc`.
    pub fn block_at(&self, pc: usize) -> Option<&BasicBlock> {
        self.block_by_pc.get(&pc).and_then(|id| self.blocks.get(id.0))
    }

    /// Get the decoded instructions belonging to a block.
    pub fn block_instructions(&self, block: BlockId) -> &[DecodedInstruction] {
        let block = &self.blocks[block.0];
        &self.instructions[block.instructions.clone()]
    }

    /// Whether `pc` is a valid EVM jump destination.
    pub fn is_valid_jumpdest(&self, pc: usize) -> bool {
        self.block_at(pc).is_some_and(|block| {
            self.instructions[block.instructions.start].opcode == opcodes::JUMPDEST
        })
    }
}

fn push_width(opcode: u8) -> Option<usize> {
    if (opcodes::PUSH1..=opcodes::PUSH32).contains(&opcode) {
        Some((opcode - opcodes::PUSH0) as usize)
    } else {
        None
    }
}

fn decode_instructions(bytecode: &[u8]) -> Vec<DecodedInstruction> {
    let mut instructions = Vec::new();
    let mut pc = 0;

    while pc < bytecode.len() {
        let opcode = bytecode[pc];
        let width = push_width(opcode).unwrap_or(0);
        let available = width.min(bytecode.len().saturating_sub(pc + 1));
        let immediate = bytecode[pc + 1..pc + 1 + available].to_vec();
        instructions.push(DecodedInstruction {
            pc,
            opcode,
            truncated: available < width,
            immediate,
        });
        pc += 1 + available;
    }

    instructions
}

fn ends_block(instruction: &DecodedInstruction, hardfork: HardFork) -> bool {
    instruction.opcode == opcodes::JUMP ||
        instruction.opcode == opcodes::JUMPI ||
        OpCodeInfo::for_fork(instruction.opcode, hardfork)
            .is_none_or(|opcode| opcode.terminating())
}

fn terminator(instruction: &DecodedInstruction, hardfork: HardFork) -> BlockTerminator {
    match instruction.opcode {
        opcodes::JUMP => BlockTerminator::Jump,
        opcodes::JUMPI => BlockTerminator::ConditionalJump,
        opcode => match OpCodeInfo::for_fork(opcode, hardfork) {
            Some(info) if info.terminating() => BlockTerminator::Halt,
            Some(_) => BlockTerminator::Fallthrough,
            None => BlockTerminator::Invalid,
        },
    }
}

fn build_blocks(instructions: &[DecodedInstruction], hardfork: HardFork) -> Vec<BasicBlock> {
    if instructions.is_empty() {
        return Vec::new();
    }

    let mut blocks = Vec::new();
    let mut start = 0;

    for index in 0..instructions.len() {
        let current = &instructions[index];
        let next_starts_block =
            instructions.get(index + 1).is_some_and(|next| next.opcode == opcodes::JUMPDEST);
        if ends_block(current, hardfork) || next_starts_block || index + 1 == instructions.len() {
            let id = BlockId(blocks.len());
            blocks.push(BasicBlock {
                id,
                start_pc: instructions[start].pc,
                end_pc: current.next_pc(),
                instructions: start..index + 1,
                terminator: terminator(current, hardfork),
                static_edges: Vec::new(),
            });
            start = index + 1;
        }
    }

    blocks
}

fn direct_jump_target(
    instructions: &[DecodedInstruction],
    block: &BasicBlock,
    block_by_pc: &HashMap<usize, BlockId>,
) -> Option<BlockId> {
    if block.instructions.len() < 2 {
        return None;
    }
    let push = &instructions[block.instructions.end - 2];
    let target = usize::try_from(push.push_value()?).ok()?;
    let target_block = *block_by_pc.get(&target)?;
    let target_instruction =
        instructions.binary_search_by_key(&target, |instruction| instruction.pc).ok()?;
    (instructions[target_instruction].opcode == opcodes::JUMPDEST).then_some(target_block)
}

fn add_static_edges(
    instructions: &[DecodedInstruction],
    blocks: &mut [BasicBlock],
    block_by_pc: &HashMap<usize, BlockId>,
) {
    for index in 0..blocks.len() {
        let terminator = blocks[index].terminator;
        if matches!(terminator, BlockTerminator::Jump | BlockTerminator::ConditionalJump) {
            if let Some(target) = direct_jump_target(instructions, &blocks[index], block_by_pc) {
                blocks[index].static_edges.push(BlockEdge { target, kind: EdgeKind::Jump });
            }
        }

        if matches!(terminator, BlockTerminator::Fallthrough | BlockTerminator::ConditionalJump) {
            if let Some(next) = blocks.get(index + 1) {
                blocks[index].static_edges.push(BlockEdge {
                    target: next.id,
                    kind: if terminator == BlockTerminator::ConditionalJump {
                        EdgeKind::ConditionalFalse
                    } else {
                        EdgeKind::Fallthrough
                    },
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_push_data_without_treating_it_as_code() {
        let program =
            Program::decode(&[opcodes::PUSH2, 0x5b, 0x00, opcodes::STOP], HardFork::Latest);
        assert_eq!(program.instructions.len(), 2);
        assert_eq!(program.instructions[0].immediate, vec![0x5b, 0x00]);
        assert_eq!(program.instructions[1].pc, 3);
    }

    #[test]
    fn splits_at_jumpdest_and_control_transfer() {
        let bytecode = [
            opcodes::PUSH1,
            0x04,
            opcodes::JUMP,
            opcodes::STOP,
            opcodes::JUMPDEST,
            opcodes::PUSH0,
            opcodes::STOP,
        ];
        let program = Program::decode(&bytecode, HardFork::Latest);
        assert_eq!(
            program.blocks.iter().map(|block| block.start_pc).collect::<Vec<_>>(),
            vec![0, 3, 4]
        );
        assert_eq!(program.blocks[0].terminator, BlockTerminator::Jump);
        assert_eq!(program.blocks[1].terminator, BlockTerminator::Halt);
        assert!(program.is_valid_jumpdest(4));
        assert_eq!(
            program.blocks[0].static_edges,
            vec![BlockEdge { target: BlockId(2), kind: EdgeKind::Jump }]
        );
    }

    #[test]
    fn leaves_symbolic_conditional_target_unresolved() {
        let bytecode = [
            opcodes::PUSH1,
            0x06,
            opcodes::PUSH1,
            0x01,
            opcodes::JUMPI,
            opcodes::STOP,
            opcodes::JUMPDEST,
            opcodes::STOP,
        ];
        let program = Program::decode(&bytecode, HardFork::Latest);
        assert_eq!(program.blocks[0].terminator, BlockTerminator::ConditionalJump);
        assert_eq!(
            program.blocks[0].static_edges,
            vec![BlockEdge { target: BlockId(1), kind: EdgeKind::ConditionalFalse }]
        );
        // The destination is not immediately below JUMPI: its condition is, so local decoding
        // correctly leaves the true edge for value-flow analysis.
    }

    #[test]
    fn records_both_locally_known_conditional_edges() {
        let bytecode = [
            opcodes::PUSH1,
            0x01,
            opcodes::PUSH1,
            0x06,
            opcodes::JUMPI,
            opcodes::STOP,
            opcodes::JUMPDEST,
            opcodes::STOP,
        ];
        let program = Program::decode(&bytecode, HardFork::Latest);
        assert_eq!(
            program.blocks[0].static_edges,
            vec![
                BlockEdge { target: BlockId(2), kind: EdgeKind::Jump },
                BlockEdge { target: BlockId(1), kind: EdgeKind::ConditionalFalse },
            ]
        );
    }

    #[test]
    fn pads_a_truncated_push_operand_on_the_right() {
        let program = Program::decode(&[opcodes::PUSH2, 0x12], HardFork::Latest);
        let instruction = &program.instructions[0];
        assert!(instruction.truncated);
        assert_eq!(instruction.push_value(), Some(U256::from(0x1200)));
    }

    #[test]
    fn hardfork_inactive_opcode_ends_its_block() {
        let program =
            Program::decode(&[opcodes::PUSH0, opcodes::JUMPDEST, opcodes::STOP], HardFork::London);
        assert_eq!(program.blocks[0].terminator, BlockTerminator::Invalid);
        assert_eq!(program.blocks[1].start_pc, 1);
    }
}
