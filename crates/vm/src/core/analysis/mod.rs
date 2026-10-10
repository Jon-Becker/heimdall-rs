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

use super::program::{BlockId, Program};

mod control_flow;
mod transfer;

use control_flow::successors;
use transfer::execute_block;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{hardfork::HardFork, opcodes};
    use alloy::primitives::U256;

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
