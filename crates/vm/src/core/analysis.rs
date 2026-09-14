//! Context-free abstract interpretation over canonical EVM basic blocks.
//!
//! The analysis in this module is deliberately small: it tracks finite sets of stack constants,
//! joins states at block entries, and reaches a fixpoint with a worklist. It establishes the state
//! propagation substrate on which symbolic expressions, context sensitivity, and solver-backed
//! jump refinement can be layered without returning to recursive path enumeration.

use std::collections::{BTreeSet, HashMap, VecDeque};

pub use super::{
    stack::{AbstractStack, AbstractValue},
    state::AbstractState,
};

use alloy::primitives::U256;

use super::{
    opcodes::{self, OpCodeInfo},
    program::{BlockId, BlockTerminator, EdgeKind, Program},
};

/// Default maximum number of alternatives retained for one abstract value before widening.
pub const DEFAULT_MAX_VALUE_SET: usize = 8;

/// Kind of edge discovered by abstract interpretation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AbstractEdgeKind {
    /// Sequential control flow.
    Fallthrough,
    /// False side of a conditional jump.
    ConditionalFalse,
    /// True side of a conditional jump.
    ConditionalTrue,
    /// Unconditional jump.
    Jump,
}

/// A reachable edge in the abstract CFG.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AbstractEdge {
    /// Source basic block.
    pub source: BlockId,
    /// Destination basic block.
    pub target: BlockId,
    /// Edge semantics.
    pub kind: AbstractEdgeKind,
}

/// Configuration controlling finite-domain widening.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AnalysisConfig {
    /// Maximum alternatives retained in one [`AbstractValue::Known`] set.
    pub max_value_set: usize,
}

impl Default for AnalysisConfig {
    fn default() -> Self {
        Self { max_value_set: DEFAULT_MAX_VALUE_SET }
    }
}

/// Result of context-free worklist analysis.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AbstractCfg {
    /// Joined abstract state at every reachable block entry.
    pub entry_states: HashMap<BlockId, AbstractState>,
    /// Reachable, resolved control-flow edges.
    pub edges: BTreeSet<AbstractEdge>,
    /// Reachable jump blocks whose destination could not be finitely resolved.
    pub unresolved_jumps: BTreeSet<BlockId>,
    /// Reachable blocks that encounter a definite stack underflow.
    pub invalid_stack_blocks: BTreeSet<BlockId>,
    /// Reachable jumps for which at least one concrete target is not a valid JUMPDEST.
    pub invalid_jump_blocks: BTreeSet<BlockId>,
}

/// Analyze a program from its first block using the default finite-domain configuration.
pub fn analyze(program: &Program) -> AbstractCfg {
    analyze_with_config(program, AnalysisConfig::default())
}

/// Analyze a program from its first block with an explicit configuration.
pub fn analyze_with_config(program: &Program, config: AnalysisConfig) -> AbstractCfg {
    let Some(entry) = program.blocks.first().map(|block| block.id) else {
        return AbstractCfg::default()
    };
    analyze_from(program, entry, AbstractState::new(), config)
}

/// Analyze from an explicit block and abstract entry state.
///
/// This supports independent public-function analysis and later context-sensitive re-analysis.
/// The block identifier is program-local and must come from `program`.
pub fn analyze_from(
    program: &Program,
    entry: BlockId,
    initial_state: AbstractState,
    config: AnalysisConfig,
) -> AbstractCfg {
    if program.blocks.get(entry.index()).is_none_or(|block| block.id != entry) {
        return AbstractCfg::default()
    }

    let mut result = AbstractCfg::default();
    result.entry_states.insert(entry, initial_state);
    let mut worklist = VecDeque::from([entry]);

    while let Some(block_id) = worklist.pop_front() {
        let entry_state = result.entry_states[&block_id].clone();
        let Some(exit) = execute_block(program, block_id, entry_state) else {
            result.invalid_stack_blocks.insert(block_id);
            continue
        };

        for successor in successors(program, block_id, &exit, &mut result) {
            let edge =
                AbstractEdge { source: block_id, target: successor.target, kind: successor.kind };
            result.edges.insert(edge);
            let changed = match result.entry_states.get(&successor.target) {
                Some(previous) => {
                    let joined = previous.join(&successor.state, config.max_value_set);
                    if &joined == previous {
                        false
                    } else {
                        result.entry_states.insert(successor.target, joined);
                        true
                    }
                }
                None => {
                    result.entry_states.insert(successor.target, successor.state);
                    true
                }
            };
            if changed {
                worklist.push_back(successor.target);
            }
        }
    }

    result
}

#[derive(Clone, Debug)]
struct BlockExit {
    state: AbstractState,
    jump_target: Option<AbstractValue>,
    condition: Option<AbstractValue>,
}

#[derive(Clone, Debug)]
struct Successor {
    target: BlockId,
    kind: AbstractEdgeKind,
    state: AbstractState,
}

fn execute_block(
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

fn successors(
    program: &Program,
    block_id: BlockId,
    exit: &BlockExit,
    result: &mut AbstractCfg,
) -> Vec<Successor> {
    let block = &program.blocks[block_id.index()];
    let (take_true, take_false) = branch_feasibility(exit.condition.as_ref());
    let mut successors = Vec::new();

    if take_true &&
        matches!(block.terminator, BlockTerminator::Jump | BlockTerminator::ConditionalJump)
    {
        let kind = if block.terminator == BlockTerminator::ConditionalJump {
            AbstractEdgeKind::ConditionalTrue
        } else {
            AbstractEdgeKind::Jump
        };
        match exit.jump_target.as_ref() {
            Some(AbstractValue::Known(targets)) => {
                for target in targets {
                    match usize::try_from(*target).ok().and_then(|pc| program.block_at(pc)) {
                        Some(target_block) if program.is_valid_jumpdest(target_block.start_pc) => {
                            successors.push(Successor {
                                target: target_block.id,
                                kind,
                                state: exit.state.clone(),
                            });
                        }
                        _ => {
                            result.invalid_jump_blocks.insert(block_id);
                        }
                    }
                }
            }
            Some(AbstractValue::Unknown) | None => {
                // Retain locally-known direct edges from the structural frontend as a fallback.
                let mut resolved = false;
                for edge in &block.static_edges {
                    if edge.kind == EdgeKind::Jump {
                        resolved = true;
                        successors.push(Successor {
                            target: edge.target,
                            kind,
                            state: exit.state.clone(),
                        });
                    }
                }
                if !resolved {
                    result.unresolved_jumps.insert(block_id);
                }
            }
        }
    }

    if take_false && block.terminator == BlockTerminator::ConditionalJump {
        if let Some(edge) =
            block.static_edges.iter().find(|edge| edge.kind == EdgeKind::ConditionalFalse)
        {
            successors.push(Successor {
                target: edge.target,
                kind: AbstractEdgeKind::ConditionalFalse,
                state: exit.state.clone(),
            });
        }
    } else if block.terminator == BlockTerminator::Fallthrough {
        if let Some(edge) =
            block.static_edges.iter().find(|edge| edge.kind == EdgeKind::Fallthrough)
        {
            successors.push(Successor {
                target: edge.target,
                kind: AbstractEdgeKind::Fallthrough,
                state: exit.state.clone(),
            });
        }
    }

    successors
}

fn branch_feasibility(condition: Option<&AbstractValue>) -> (bool, bool) {
    match condition {
        None => (true, false),
        Some(AbstractValue::Unknown) => (true, true),
        Some(AbstractValue::Known(values)) => {
            (values.iter().any(|value| !value.is_zero()), values.contains(&U256::ZERO))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::hardfork::HardFork;

    fn program(bytecode: &[u8]) -> Program {
        Program::decode(bytecode, HardFork::Latest)
    }

    #[test]
    fn resolves_a_jump_through_stack_manipulation() {
        let program = program(&[
            opcodes::PUSH1,
            0x07,
            opcodes::DUP1,
            opcodes::POP,
            opcodes::JUMP,
            opcodes::STOP,
            opcodes::STOP,
            opcodes::JUMPDEST,
            opcodes::STOP,
        ]);
        let cfg = analyze(&program);

        assert!(cfg.edges.contains(&AbstractEdge {
            source: program.blocks[0].id,
            target: program.blocks[3].id,
            kind: AbstractEdgeKind::Jump,
        }));
        assert!(cfg.unresolved_jumps.is_empty());
    }

    #[test]
    fn joins_values_at_a_shared_block_entry() {
        let program = program(&[
            opcodes::CALLVALUE,
            opcodes::PUSH1,
            0x09,
            opcodes::JUMPI,
            opcodes::PUSH1,
            0x01,
            opcodes::PUSH1,
            0x0f,
            opcodes::JUMP,
            opcodes::JUMPDEST,
            opcodes::PUSH1,
            0x02,
            opcodes::PUSH1,
            0x0f,
            opcodes::JUMP,
            opcodes::JUMPDEST,
            opcodes::STOP,
        ]);
        let cfg = analyze(&program);
        let join = program.block_at(0x0f).expect("join block must exist");
        let values = cfg.entry_states[&join.id].stack.values()[0]
            .known_values()
            .expect("joined value should remain finite");

        assert_eq!(values, &BTreeSet::from([U256::from(1), U256::from(2)]));
    }

    #[test]
    fn prunes_a_statically_false_jump() {
        let program = program(&[
            opcodes::PUSH0,
            opcodes::PUSH1,
            0x05,
            opcodes::JUMPI,
            opcodes::STOP,
            opcodes::JUMPDEST,
            opcodes::STOP,
        ]);
        let cfg = analyze(&program);

        assert_eq!(cfg.edges.len(), 1);
        assert_eq!(
            cfg.edges.first().expect("fallthrough edge").kind,
            AbstractEdgeKind::ConditionalFalse
        );
        assert!(!cfg.entry_states.contains_key(&program.block_at(5).expect("target").id));
    }

    #[test]
    fn reaches_a_fixpoint_for_a_self_loop() {
        let program = program(&[opcodes::JUMPDEST, opcodes::PUSH0, opcodes::JUMP]);
        let cfg = analyze(&program);

        assert_eq!(cfg.entry_states.len(), 1);
        assert_eq!(cfg.edges.len(), 1);
        assert!(cfg.unresolved_jumps.is_empty());
    }

    #[test]
    fn supports_analysis_from_an_explicit_entry_state() {
        let program = program(&[opcodes::STOP, opcodes::JUMPDEST, opcodes::STOP]);
        let entry = program.block_at(1).expect("secondary entry").id;
        let initial = AbstractState::with_stack(AbstractStack::from_values(
            vec![AbstractValue::constant(U256::from(7))],
            true,
        ));
        let cfg = analyze_from(&program, entry, initial.clone(), AnalysisConfig::default());

        assert_eq!(cfg.entry_states.len(), 1);
        assert_eq!(cfg.entry_states[&entry], initial);
    }

    #[test]
    fn marks_unknown_dynamic_jumps_without_dropping_the_block() {
        let program = program(&[opcodes::CALLVALUE, opcodes::JUMP]);
        let cfg = analyze(&program);

        assert_eq!(cfg.entry_states.len(), 1);
        assert_eq!(cfg.unresolved_jumps, BTreeSet::from([program.blocks[0].id]));
        assert!(cfg.invalid_stack_blocks.is_empty());
    }
}
