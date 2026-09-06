//! Context-free abstract interpretation over canonical EVM basic blocks.
//!
//! The analysis in this module is deliberately small: it tracks finite sets of stack constants,
//! joins states at block entries, and reaches a fixpoint with a worklist. It establishes the state
//! propagation substrate on which symbolic expressions, context sensitivity, and solver-backed
//! jump refinement can be layered without returning to recursive path enumeration.

use std::{
    collections::{BTreeSet, HashMap, VecDeque},
    iter,
};

use alloy::primitives::U256;

#[cfg(feature = "smt")]
use super::smt::{SmtConfig, SmtRefiner, SmtStats};
pub use super::symbolic::{AbstractValue, ExprId, ExpressionArena, ExpressionNode};
use super::{
    abstract_state::{AbstractStateSpaces, StateVersionArena},
    facts::{condition_may_be, PathFacts},
    opcodes::{self, OpCodeInfo},
    program::{BlockId, BlockTerminator, EdgeKind, Program},
    symbolic::{operation_result, stateful_operation_result},
};

/// Default maximum number of alternatives retained for one abstract value before widening.
pub const DEFAULT_MAX_VALUE_SET: usize = 8;

/// Abstract EVM stack, stored from top to bottom.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AbstractStack {
    values: Vec<AbstractValue>,
    unknown_tail: bool,
}

impl AbstractStack {
    /// Construct an empty, exact stack.
    pub fn new() -> Self {
        Self::default()
    }

    /// Construct a stack from explicit values ordered from top to bottom.
    ///
    /// When `unknown_tail` is true, additional values may exist below the provided prefix.
    pub fn from_values(values: Vec<AbstractValue>, unknown_tail: bool) -> Self {
        Self { values, unknown_tail }
    }

    /// Explicit values from the top of the stack downward.
    pub fn values(&self) -> &[AbstractValue] {
        &self.values
    }

    /// Whether additional, untracked values may exist below the explicit values.
    pub fn has_unknown_tail(&self) -> bool {
        self.unknown_tail
    }

    fn push(&mut self, value: AbstractValue) {
        self.values.insert(0, value);
    }

    fn pop(&mut self) -> Option<AbstractValue> {
        if self.values.is_empty() {
            self.unknown_tail.then_some(AbstractValue::Unknown)
        } else {
            Some(self.values.remove(0))
        }
    }

    fn pop_n(&mut self, count: usize) -> Option<Vec<AbstractValue>> {
        (0..count).map(|_| self.pop()).collect()
    }

    fn peek(&self, index: usize) -> Option<AbstractValue> {
        self.values
            .get(index)
            .cloned()
            .or_else(|| self.unknown_tail.then_some(AbstractValue::Unknown))
    }

    fn swap(&mut self, index: usize) -> bool {
        if index >= self.values.len() {
            if !self.unknown_tail {
                return false
            }
            self.values
                .extend(iter::repeat_n(AbstractValue::Unknown, index + 1 - self.values.len()));
        }
        self.values.swap(0, index);
        true
    }

    fn join(&self, other: &Self, max_values: usize) -> Self {
        let common_depth = self.values.len().min(other.values.len());
        let values = (0..common_depth)
            .map(|index| self.values[index].join(&other.values[index], max_values))
            .collect();
        Self {
            values,
            unknown_tail: self.unknown_tail ||
                other.unknown_tail ||
                self.values.len() != other.values.len(),
        }
    }
}

/// Abstract state recorded at a basic-block entry.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AbstractState {
    /// Abstract operand stack.
    pub stack: AbstractStack,
    /// Predicates known to hold on this path.
    pub facts: PathFacts,
    /// Versioned memory, storage, and transient storage.
    pub state: AbstractStateSpaces,
}

impl AbstractState {
    /// Construct the empty initial EVM state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Construct a state with an explicit abstract stack.
    pub fn with_stack(stack: AbstractStack) -> Self {
        Self { stack, facts: PathFacts::new(), state: AbstractStateSpaces::default() }
    }

    pub(crate) fn join(
        &self,
        other: &Self,
        max_values: usize,
        state_versions: &mut StateVersionArena,
    ) -> Self {
        Self {
            stack: self.stack.join(&other.stack, max_values),
            facts: self.facts.join(&other.facts),
            state: self.state.join(&other.state, max_values, state_versions),
        }
    }
}

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
    #[cfg(feature = "smt")]
    /// Optional demand-driven SMT refinement limits.
    pub smt: Option<SmtConfig>,
}

impl Default for AnalysisConfig {
    fn default() -> Self {
        Self {
            max_value_set: DEFAULT_MAX_VALUE_SET,
            #[cfg(feature = "smt")]
            smt: Some(SmtConfig::default()),
        }
    }
}

/// Result of context-free worklist analysis.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AbstractCfg {
    /// Hash-consed symbolic expressions referenced by abstract states.
    pub expressions: ExpressionArena,
    /// Persistent versions referenced by memory and storage expressions.
    pub state_versions: StateVersionArena,
    /// Joined abstract state at every reachable block entry.
    pub entry_states: HashMap<BlockId, AbstractState>,
    /// Most recent fixpoint exit state and control operands for each executed block.
    pub exit_states: HashMap<BlockId, BlockExit>,
    /// Reachable, resolved control-flow edges.
    pub edges: BTreeSet<AbstractEdge>,
    /// Reachable jump blocks whose destination could not be finitely resolved.
    pub unresolved_jumps: BTreeSet<BlockId>,
    /// Reachable blocks that encounter a definite stack underflow.
    pub invalid_stack_blocks: BTreeSet<BlockId>,
    /// Reachable jumps for which at least one concrete target is not a valid JUMPDEST.
    pub invalid_jump_blocks: BTreeSet<BlockId>,
    /// Conditional edge directions proven infeasible by constants, path facts, or SMT.
    pub pruned_branches: BTreeSet<(BlockId, bool)>,
    #[cfg(feature = "smt")]
    /// Aggregate demand-driven SMT activity.
    pub smt_stats: SmtStats,
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
    #[cfg(feature = "smt")]
    let mut smt = config.smt.map(SmtRefiner::new);
    let mut worklist = VecDeque::from([entry]);

    while let Some(block_id) = worklist.pop_front() {
        let entry_state = result.entry_states[&block_id].clone();
        let Some(exit) = execute_block(
            program,
            block_id,
            entry_state,
            &mut result.expressions,
            &mut result.state_versions,
            config.max_value_set,
        ) else {
            result.exit_states.remove(&block_id);
            result.invalid_stack_blocks.insert(block_id);
            continue
        };
        result.exit_states.insert(block_id, exit.clone());

        let successors = successors(
            program,
            block_id,
            &exit,
            &result.expressions,
            &mut result.unresolved_jumps,
            &mut result.invalid_jump_blocks,
            &mut result.pruned_branches,
            #[cfg(feature = "smt")]
            &mut smt,
        );
        for successor in successors {
            let edge =
                AbstractEdge { source: block_id, target: successor.target, kind: successor.kind };
            result.edges.insert(edge);
            let changed = match result.entry_states.get(&successor.target) {
                Some(previous) => {
                    let joined = previous.join(
                        &successor.state,
                        config.max_value_set,
                        &mut result.state_versions,
                    );
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

    #[cfg(feature = "smt")]
    if let Some(smt) = smt {
        result.smt_stats = smt.stats();
    }
    result
}

/// Abstract state and control operands after executing one canonical basic block.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BlockExit {
    /// State after the final instruction and after consuming jump operands.
    pub state: AbstractState,
    /// Abstract jump destination consumed by `JUMP` or `JUMPI`.
    pub jump_target: Option<AbstractValue>,
    /// Abstract branch condition consumed by `JUMPI`.
    pub condition: Option<AbstractValue>,
}

#[derive(Clone, Debug)]
struct Successor {
    target: BlockId,
    kind: AbstractEdgeKind,
    state: AbstractState,
}

pub(crate) fn execute_block(
    program: &Program,
    block_id: BlockId,
    mut state: AbstractState,
    expressions: &mut ExpressionArena,
    state_versions: &mut StateVersionArena,
    max_values: usize,
) -> Option<BlockExit> {
    let block = &program.blocks[block_id.index()];
    let instructions = program.block_instructions(block_id);
    let mut jump_target = None;
    let mut condition = None;

    for instruction in instructions {
        match instruction.opcode {
            opcodes::MLOAD => {
                let key = state.stack.pop()?;
                let value = state.state.memory.load(&key).cloned().unwrap_or_else(|| {
                    stateful_operation_result(
                        instruction,
                        vec![key],
                        state.state.memory.version,
                        expressions,
                    )
                });
                state.stack.push(value);
            }
            opcodes::MSTORE | opcodes::MSTORE8 => {
                let key = state.stack.pop()?;
                let value = state.stack.pop()?;
                state.state.memory.store(
                    key,
                    Some(value),
                    Some(if instruction.opcode == opcodes::MSTORE { 32 } else { 1 }),
                    state_versions,
                );
            }
            opcodes::SLOAD => {
                let key = state.stack.pop()?;
                let value = state.state.storage.load(&key).cloned().unwrap_or_else(|| {
                    stateful_operation_result(
                        instruction,
                        vec![key],
                        state.state.storage.version,
                        expressions,
                    )
                });
                state.stack.push(value);
            }
            opcodes::SSTORE => {
                let key = state.stack.pop()?;
                let value = state.stack.pop()?;
                state.state.storage.store(key, Some(value), Some(32), state_versions);
            }
            opcodes::TLOAD => {
                let key = state.stack.pop()?;
                let value =
                    state.state.transient_storage.load(&key).cloned().unwrap_or_else(|| {
                        stateful_operation_result(
                            instruction,
                            vec![key],
                            state.state.transient_storage.version,
                            expressions,
                        )
                    });
                state.stack.push(value);
            }
            opcodes::TSTORE => {
                let key = state.stack.pop()?;
                let value = state.stack.pop()?;
                state.state.transient_storage.store(key, Some(value), Some(32), state_versions);
            }
            opcodes::SHA3 => {
                let inputs = state.stack.pop_n(2)?;
                state.stack.push(stateful_operation_result(
                    instruction,
                    inputs,
                    state.state.memory.version,
                    expressions,
                ));
            }
            opcodes::CALL | opcodes::CALLCODE => {
                let inputs = state.stack.pop_n(7)?;
                let output_size = exact_usize(&inputs[6]);
                if output_size != Some(0) {
                    state.state.memory.havoc(inputs[5].clone(), output_size, state_versions);
                }
                state.stack.push(operation_result(instruction, inputs, 0, expressions, max_values));
            }
            opcodes::DELEGATECALL | opcodes::STATICCALL => {
                let inputs = state.stack.pop_n(6)?;
                let output_size = exact_usize(&inputs[5]);
                if output_size != Some(0) {
                    state.state.memory.havoc(inputs[4].clone(), output_size, state_versions);
                }
                state.stack.push(operation_result(instruction, inputs, 0, expressions, max_values));
            }
            opcodes::CALLDATACOPY |
            opcodes::CODECOPY |
            opcodes::RETURNDATACOPY |
            opcodes::MCOPY => {
                let inputs = state.stack.pop_n(3)?;
                state.state.memory.havoc(
                    inputs[0].clone(),
                    exact_usize(&inputs[2]),
                    state_versions,
                );
            }
            opcodes::EXTCODECOPY => {
                let inputs = state.stack.pop_n(4)?;
                state.state.memory.havoc(
                    inputs[1].clone(),
                    exact_usize(&inputs[3]),
                    state_versions,
                );
            }
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
                let inputs = state.stack.pop_n(info.inputs() as usize)?;
                for output in 0..info.outputs() {
                    state.stack.push(operation_result(
                        instruction,
                        inputs.clone(),
                        output,
                        expressions,
                        max_values,
                    ));
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

fn exact_usize(value: &AbstractValue) -> Option<usize> {
    value
        .known_values()
        .filter(|values| values.len() == 1)
        .and_then(|values| values.first())
        .and_then(|value| usize::try_from(*value).ok())
}

fn successors(
    program: &Program,
    block_id: BlockId,
    exit: &BlockExit,
    expressions: &ExpressionArena,
    unresolved_jumps: &mut BTreeSet<BlockId>,
    invalid_jump_blocks: &mut BTreeSet<BlockId>,
    pruned_branches: &mut BTreeSet<(BlockId, bool)>,
    #[cfg(feature = "smt")] smt: &mut Option<SmtRefiner>,
) -> Vec<Successor> {
    let block = &program.blocks[block_id.index()];
    #[allow(unused_mut)]
    let (mut take_true, mut take_false) =
        branch_feasibility(exit.condition.as_ref(), &exit.state.facts, expressions);
    #[cfg(feature = "smt")]
    if take_true && take_false {
        if let (Some(condition), Some(refiner)) = (exit.condition.as_ref(), smt.as_mut()) {
            take_true = refiner
                .branch_feasible(condition, true, &exit.state.facts, expressions)
                .unwrap_or(true);
            take_false = refiner
                .branch_feasible(condition, false, &exit.state.facts, expressions)
                .unwrap_or(true);
        }
    }
    if block.terminator == BlockTerminator::ConditionalJump {
        if !take_true {
            pruned_branches.insert((block_id, true));
        }
        if !take_false {
            pruned_branches.insert((block_id, false));
        }
    }
    let true_state = take_true.then(|| assumed_state(exit, true, expressions)).flatten();
    let false_state = take_false.then(|| assumed_state(exit, false, expressions)).flatten();
    let mut successors = Vec::new();

    if let Some(true_state) = true_state {
        if matches!(block.terminator, BlockTerminator::Jump | BlockTerminator::ConditionalJump) {
            let kind = if block.terminator == BlockTerminator::ConditionalJump {
                AbstractEdgeKind::ConditionalTrue
            } else {
                AbstractEdgeKind::Jump
            };
            match exit.jump_target.as_ref() {
                Some(AbstractValue::Known(targets)) => {
                    for target in targets {
                        match usize::try_from(*target).ok().and_then(|pc| program.block_at(pc)) {
                            Some(target_block)
                                if program.is_valid_jumpdest(target_block.start_pc) =>
                            {
                                successors.push(Successor {
                                    target: target_block.id,
                                    kind,
                                    state: true_state.clone(),
                                });
                            }
                            _ => {
                                invalid_jump_blocks.insert(block_id);
                            }
                        }
                    }
                }
                Some(AbstractValue::Symbolic { .. }) => {
                    let mut resolved = false;
                    #[cfg(feature = "smt")]
                    if let Some(refiner) = smt.as_mut() {
                        if let Some(models) = refiner.jump_targets(
                            exit.jump_target.as_ref().expect("matched symbolic target"),
                            &true_state.facts,
                            expressions,
                            program,
                        ) {
                            resolved = true;
                            if models.targets.is_empty() {
                                pruned_branches.insert((block_id, true));
                            }
                            for target in models.targets {
                                if let Some(target_block) =
                                    usize::try_from(target).ok().and_then(|pc| program.block_at(pc))
                                {
                                    successors.push(Successor {
                                        target: target_block.id,
                                        kind,
                                        state: true_state.clone(),
                                    });
                                }
                            }
                            if !models.complete {
                                unresolved_jumps.insert(block_id);
                            }
                        }
                    }
                    // Retain locally-known direct edges from the structural frontend as a fallback.
                    if !resolved {
                        for edge in &block.static_edges {
                            if edge.kind == EdgeKind::Jump {
                                resolved = true;
                                successors.push(Successor {
                                    target: edge.target,
                                    kind,
                                    state: true_state.clone(),
                                });
                            }
                        }
                    }
                    if !resolved {
                        unresolved_jumps.insert(block_id);
                    }
                }
                Some(AbstractValue::Unknown) | None => {
                    let mut resolved = false;
                    for edge in &block.static_edges {
                        if edge.kind == EdgeKind::Jump {
                            resolved = true;
                            successors.push(Successor {
                                target: edge.target,
                                kind,
                                state: true_state.clone(),
                            });
                        }
                    }
                    if !resolved {
                        unresolved_jumps.insert(block_id);
                    }
                }
            }
        }
    }

    if let Some(false_state) = false_state {
        if block.terminator == BlockTerminator::ConditionalJump {
            if let Some(edge) =
                block.static_edges.iter().find(|edge| edge.kind == EdgeKind::ConditionalFalse)
            {
                successors.push(Successor {
                    target: edge.target,
                    kind: AbstractEdgeKind::ConditionalFalse,
                    state: false_state,
                });
            }
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

pub(crate) fn branch_feasibility(
    condition: Option<&AbstractValue>,
    facts: &PathFacts,
    expressions: &ExpressionArena,
) -> (bool, bool) {
    match condition {
        None => (true, false),
        Some(condition) => (
            condition_may_be(condition, true, facts, expressions),
            condition_may_be(condition, false, facts, expressions),
        ),
    }
}

pub(crate) fn assumed_state(
    exit: &BlockExit,
    taken: bool,
    expressions: &ExpressionArena,
) -> Option<AbstractState> {
    let mut state = exit.state.clone();
    if let Some(condition) = &exit.condition {
        state.facts.assume(condition, taken, expressions).then_some(state)
    } else {
        Some(state)
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
        let entry = program.blocks[0].id;
        let exit = &cfg.exit_states[&entry];
        assert_eq!(
            exit.jump_target.as_ref().and_then(AbstractValue::known_values),
            Some(&BTreeSet::from([U256::from(5)]))
        );
        assert_eq!(
            exit.condition.as_ref().and_then(AbstractValue::known_values),
            Some(&BTreeSet::from([U256::ZERO]))
        );
        assert!(exit.state.stack.values().is_empty());
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
    fn widens_large_value_sets_to_unknown() {
        let left = AbstractValue::Known(BTreeSet::from([U256::from(1), U256::from(2)]));
        let right = AbstractValue::Known(BTreeSet::from([U256::from(3), U256::from(4)]));
        assert_eq!(left.join(&right, 3), AbstractValue::Unknown);
    }

    #[test]
    fn forwards_memory_words_through_versioned_state() {
        let program = program(&[
            opcodes::PUSH1,
            42,
            opcodes::PUSH1,
            0,
            opcodes::MSTORE,
            opcodes::PUSH1,
            0,
            opcodes::MLOAD,
            opcodes::PUSH1,
            12,
            opcodes::JUMP,
            opcodes::STOP,
            opcodes::JUMPDEST,
            opcodes::STOP,
        ]);
        let cfg = analyze(&program);
        let target = program.block_at(12).expect("jump target").id;

        assert_eq!(
            cfg.entry_states[&target].stack.values()[0].known_values(),
            Some(&BTreeSet::from([U256::from(42)]))
        );
    }

    #[test]
    fn external_call_invalidates_its_output_memory() {
        let program = program(&[
            opcodes::PUSH1,
            42,
            opcodes::PUSH0,
            opcodes::MSTORE,
            opcodes::PUSH1,
            32,
            opcodes::PUSH0,
            opcodes::PUSH0,
            opcodes::PUSH0,
            opcodes::PUSH0,
            opcodes::PUSH1,
            1,
            opcodes::PUSH0,
            opcodes::CALL,
            opcodes::POP,
            opcodes::PUSH0,
            opcodes::MLOAD,
            opcodes::STOP,
        ]);
        let cfg = analyze(&program);
        let exit = &cfg.exit_states[&program.blocks[0].id];
        let value = &exit.state.stack.values()[0];
        let expression = value
            .expressions()
            .and_then(|expressions| expressions.first())
            .and_then(|expression| cfg.expressions.get(*expression))
            .expect("memory read after call");

        assert_eq!(expression.opcode, opcodes::MLOAD);
        assert!(value.known_values().is_none());
    }

    #[test]
    fn storage_reads_reference_the_current_version() {
        let program = program(&[
            opcodes::PUSH0,
            opcodes::SLOAD,
            opcodes::POP,
            opcodes::PUSH1,
            1,
            opcodes::PUSH0,
            opcodes::SSTORE,
            opcodes::PUSH1,
            1,
            opcodes::SLOAD,
            opcodes::PUSH1,
            14,
            opcodes::JUMP,
            opcodes::STOP,
            opcodes::JUMPDEST,
            opcodes::STOP,
        ]);
        let cfg = analyze(&program);
        let target = program.block_at(14).expect("jump target").id;
        let expression = *cfg.entry_states[&target].stack.values()[0]
            .expressions()
            .expect("versioned storage read")
            .first()
            .expect("one expression");
        let node = cfg.expressions.get(expression).expect("expression node");

        assert_eq!(node.opcode, opcodes::SLOAD);
        assert_ne!(
            node.state_version,
            Some(cfg.state_versions.initial(crate::core::abstract_state::StateDomain::Storage))
        );
    }

    #[test]
    fn storage_writing_loop_reaches_a_version_fixpoint() {
        let program = program(&[
            opcodes::JUMPDEST,
            opcodes::PUSH1,
            1,
            opcodes::PUSH1,
            0,
            opcodes::SSTORE,
            opcodes::PUSH1,
            0,
            opcodes::JUMP,
        ]);
        let cfg = analyze(&program);

        assert_eq!(cfg.entry_states.len(), 1);
        assert_eq!(cfg.edges.len(), 1);
        assert!(cfg.state_versions.version_count() < 32);
    }

    #[test]
    fn carries_symbolic_expressions_across_block_edges() {
        let program = program(&[
            opcodes::CALLER,
            opcodes::PUSH2,
            0xff,
            0xff,
            opcodes::AND,
            opcodes::PUSH1,
            0x08,
            opcodes::JUMP,
            opcodes::JUMPDEST,
            opcodes::STOP,
        ]);
        let cfg = analyze(&program);
        let target = program.block_at(8).expect("jump target");
        let value = &cfg.entry_states[&target.id].stack.values()[0];
        let expressions = value.expressions().expect("symbolic masked caller");
        let expression = cfg
            .expressions
            .get(*expressions.first().expect("one expression"))
            .expect("interned expression");

        assert_eq!(expression.opcode, opcodes::AND);
        assert_eq!(cfg.expressions.len(), 2);
    }

    #[test]
    fn prunes_repeated_conditions_using_path_facts() {
        let program = program(&[
            opcodes::CALLDATASIZE,
            opcodes::PUSH1,
            36,
            opcodes::LT,
            opcodes::PUSH1,
            0x0f,
            opcodes::JUMPI,
            opcodes::CALLDATASIZE,
            opcodes::PUSH1,
            36,
            opcodes::LT,
            opcodes::PUSH1,
            0x0f,
            opcodes::JUMPI,
            opcodes::STOP,
            opcodes::JUMPDEST,
            opcodes::STOP,
        ]);
        let cfg = analyze(&program);
        let repeated = program.block_at(7).expect("repeated condition block").id;
        let outgoing = cfg.edges.iter().filter(|edge| edge.source == repeated).collect::<Vec<_>>();

        assert_eq!(outgoing.len(), 1);
        assert_eq!(outgoing[0].kind, AbstractEdgeKind::ConditionalFalse);
    }

    #[test]
    fn marks_unknown_dynamic_jumps_without_dropping_the_block() {
        let program = program(&[opcodes::CALLVALUE, opcodes::JUMP]);
        let cfg = analyze(&program);

        assert_eq!(cfg.entry_states.len(), 1);
        #[cfg(not(feature = "smt"))]
        assert_eq!(cfg.unresolved_jumps, BTreeSet::from([program.blocks[0].id]));
        #[cfg(feature = "smt")]
        assert!(cfg.pruned_branches.contains(&(program.blocks[0].id, true)));
        assert!(cfg.invalid_stack_blocks.is_empty());
    }

    #[cfg(feature = "smt")]
    #[test]
    fn smt_resolves_computed_dynamic_jump_targets() {
        let program = program(&[
            opcodes::PUSH0,
            opcodes::CALLDATALOAD,
            opcodes::PUSH1,
            1,
            opcodes::AND,
            opcodes::PUSH1,
            12,
            opcodes::ADD,
            opcodes::JUMP,
            opcodes::STOP,
            opcodes::STOP,
            opcodes::STOP,
            opcodes::JUMPDEST,
            opcodes::JUMPDEST,
            opcodes::STOP,
        ]);
        let cfg = analyze(&program);
        let targets = cfg
            .edges
            .iter()
            .filter(|edge| edge.source == program.blocks[0].id)
            .map(|edge| program.blocks[edge.target.index()].start_pc)
            .collect::<BTreeSet<_>>();

        assert_eq!(targets, BTreeSet::from([12, 13]));
        assert!(cfg.unresolved_jumps.is_empty());
        assert_eq!(cfg.smt_stats.resolved_targets, 2);
    }

    #[cfg(feature = "smt")]
    #[test]
    fn smt_prunes_nontrivial_modular_identity() {
        let program = program(&[
            opcodes::PUSH0,
            opcodes::CALLDATALOAD,
            opcodes::DUP1,
            opcodes::PUSH1,
            1,
            opcodes::ADD,
            opcodes::EQ,
            opcodes::PUSH1,
            0x0b,
            opcodes::JUMPI,
            opcodes::STOP,
            opcodes::JUMPDEST,
            opcodes::STOP,
        ]);
        let cfg = analyze(&program);

        assert!(cfg.pruned_branches.contains(&(program.blocks[0].id, true)));
        assert_eq!(cfg.smt_stats.infeasible_branches, 1);
        assert!(!cfg.edges.iter().any(|edge| edge.kind == AbstractEdgeKind::ConditionalTrue));
    }
}
