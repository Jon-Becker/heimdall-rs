//! Shrinking continuation context for context-sensitive abstract CFG analysis.
//!
//! Solidity lowers internal calls and returns to ordinary EVM jumps with continuation addresses on
//! the operand stack. This module identifies likely continuation pushes, distinguishes block-entry
//! states by their active calls, and removes completed calls from the context when a jump reaches a
//! matching continuation. The resulting context behaves like a bounded abstract call stack without
//! requiring function boundaries to be known in advance.

use std::collections::{BTreeSet, HashMap, VecDeque};

use alloy::primitives::U256;

#[cfg(feature = "smt")]
use super::smt::{SmtRefiner, SmtStats};
use super::{
    abstract_state::StateVersionArena,
    analysis::{
        assumed_state, branch_feasibility, execute_block, AbstractState, AbstractValue,
        AnalysisConfig, BlockExit, ExpressionArena,
    },
    program::{BlockId, BlockTerminator, EdgeKind, Program},
};

/// Default maximum private-call depth retained in an analysis context.
pub const DEFAULT_CONTEXT_DEPTH: usize = 20;

/// Default number of distinct contexts retained for one block before they are collapsed.
pub const DEFAULT_CONTEXTS_PER_BLOCK: usize = 64;

/// Default maximum block-state executions in one contextual analysis.
pub const DEFAULT_MAX_ANALYSIS_ITERATIONS: usize = 250_000;

/// A likely internal call and the blocks to which it may return.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CallFrame {
    caller: BlockId,
    continuations: BTreeSet<BlockId>,
}

impl CallFrame {
    /// Block that established this frame.
    pub fn caller(&self) -> BlockId {
        self.caller
    }

    /// Candidate return continuations pushed by the caller.
    pub fn continuations(&self) -> &BTreeSet<BlockId> {
        &self.continuations
    }
}

/// Bounded context attached to an abstract block-entry state.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AnalysisContext {
    public_entry: BlockId,
    private_calls: Vec<CallFrame>,
    truncated: bool,
}

impl AnalysisContext {
    /// Construct an empty private context for a public or transaction entry block.
    pub fn new(public_entry: BlockId) -> Self {
        Self { public_entry, private_calls: Vec::new(), truncated: false }
    }

    /// Sticky public entry under which this state is analyzed.
    pub fn public_entry(&self) -> BlockId {
        self.public_entry
    }

    /// Active likely private calls from outermost to innermost.
    pub fn private_calls(&self) -> &[CallFrame] {
        &self.private_calls
    }

    /// Whether older context was discarded due to a configured bound.
    pub fn is_truncated(&self) -> bool {
        self.truncated
    }

    fn collapsed(public_entry: BlockId) -> Self {
        Self { public_entry, private_calls: Vec::new(), truncated: true }
    }

    fn push(&mut self, frame: CallFrame, max_depth: usize) {
        if max_depth == 0 {
            self.private_calls.clear();
            self.truncated = true;
            return
        }
        self.private_calls.push(frame);
        if self.private_calls.len() > max_depth {
            self.private_calls.remove(0);
            self.truncated = true;
        }
    }

    fn cut_to_continuation(&mut self, target: BlockId) -> bool {
        let Some(matching_call) =
            self.private_calls.iter().rposition(|frame| frame.continuations.contains(&target))
        else {
            return false
        };
        self.private_calls.truncate(matching_call);
        true
    }
}

/// A basic block paired with the context under which it is analyzed.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ContextualPoint {
    /// Canonical basic block.
    pub block: BlockId,
    /// Calling context at block entry.
    pub context: AnalysisContext,
}

/// A context-sensitive CFG edge.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ContextualEdge {
    /// Source block and context.
    pub source: ContextualPoint,
    /// Destination block and transitioned context.
    pub target: ContextualPoint,
    /// Control-flow relationship.
    pub kind: ContextualEdgeKind,
}

/// Kind of context-sensitive edge.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ContextualEdgeKind {
    /// Sequential control flow.
    Fallthrough,
    /// False side of a conditional jump.
    ConditionalFalse,
    /// True side of a conditional jump.
    ConditionalTrue,
    /// Unconditional jump.
    Jump,
}

/// Locally inferred internal-call continuation candidates.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ContinuationHints {
    by_caller: HashMap<BlockId, BTreeSet<BlockId>>,
}

impl ContinuationHints {
    /// Candidate continuations pushed by `caller`.
    pub fn for_caller(&self, caller: BlockId) -> Option<&BTreeSet<BlockId>> {
        self.by_caller.get(&caller)
    }

    /// Number of blocks classified as likely internal callers.
    pub fn len(&self) -> usize {
        self.by_caller.len()
    }

    /// Whether no likely internal callers were found.
    pub fn is_empty(&self) -> bool {
        self.by_caller.is_empty()
    }
}

/// Infer likely continuation pushes without executing the program.
///
/// A candidate caller is an unconditional-jump block with a locally known callee and an earlier
/// PUSH of a different valid JUMPDEST. False positives are safe for reachability: a frame affects
/// only context identity and is self-healed when one of its continuations is reached.
pub fn infer_continuations(program: &Program) -> ContinuationHints {
    let mut hints = ContinuationHints::default();

    for block in &program.blocks {
        if block.terminator != BlockTerminator::Jump {
            continue
        }
        let callees = block
            .static_edges
            .iter()
            .filter(|edge| edge.kind == EdgeKind::Jump)
            .map(|edge| edge.target)
            .collect::<BTreeSet<_>>();
        if callees.is_empty() {
            continue
        }

        let mut continuations = BTreeSet::new();
        for instruction in program.block_instructions(block.id).iter().rev().skip(2) {
            let Some(target) =
                instruction.push_value().and_then(|value| usize::try_from(value).ok())
            else {
                continue
            };
            let Some(candidate) = program.block_at(target) else { continue };
            if program.is_valid_jumpdest(target) && !callees.contains(&candidate.id) {
                continuations.insert(candidate.id);
            }
        }
        if !continuations.is_empty() {
            hints.by_caller.insert(block.id, continuations);
        }
    }

    hints
}

/// Configuration for shrinking-context abstract analysis.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ContextualAnalysisConfig {
    /// Underlying finite-value analysis configuration.
    pub values: AnalysisConfig,
    /// Maximum retained likely private-call depth.
    pub max_context_depth: usize,
    /// Maximum distinct contexts retained for one block before collapsing excess contexts.
    pub max_contexts_per_block: usize,
    /// Maximum block-state executions before returning a conservative partial result.
    pub max_iterations: usize,
}

impl Default for ContextualAnalysisConfig {
    fn default() -> Self {
        Self {
            values: AnalysisConfig::default(),
            max_context_depth: DEFAULT_CONTEXT_DEPTH,
            max_contexts_per_block: DEFAULT_CONTEXTS_PER_BLOCK,
            max_iterations: DEFAULT_MAX_ANALYSIS_ITERATIONS,
        }
    }
}

/// Result of shrinking-context worklist analysis.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ContextualCfg {
    /// Hash-consed symbolic expressions referenced by contextual states.
    pub expressions: ExpressionArena,
    /// Persistent versions referenced by memory and storage expressions.
    pub state_versions: StateVersionArena,
    /// Joined abstract states keyed by block and calling context.
    pub entry_states: HashMap<ContextualPoint, AbstractState>,
    /// Most recent fixpoint exit state and control operands for each executed contextual point.
    pub exit_states: HashMap<ContextualPoint, BlockExit>,
    /// Reachable context-sensitive edges.
    pub edges: BTreeSet<ContextualEdge>,
    /// Jump points whose destination could not be finitely resolved.
    pub unresolved_jumps: BTreeSet<ContextualPoint>,
    /// Points that encounter a definite stack underflow.
    pub invalid_stack_points: BTreeSet<ContextualPoint>,
    /// Jump points with at least one concrete target that is not a valid JUMPDEST.
    pub invalid_jump_points: BTreeSet<ContextualPoint>,
    /// Destination contexts collapsed by the per-block context budget.
    pub collapsed_contexts: BTreeSet<ContextualPoint>,
    /// Contextual conditional edge directions proven infeasible.
    pub pruned_branches: BTreeSet<(ContextualPoint, bool)>,
    /// Number of block-state executions completed by the worklist.
    pub analysis_iterations: usize,
    /// Queued points not executed because the iteration budget was exhausted.
    pub budget_exhausted_points: BTreeSet<ContextualPoint>,
    #[cfg(feature = "smt")]
    /// Aggregate demand-driven SMT activity.
    pub smt_stats: SmtStats,
}

/// Analyze from bytecode entry with inferred continuation hints and default bounds.
pub fn analyze_contextual(program: &Program) -> ContextualCfg {
    analyze_contextual_with_config(program, ContextualAnalysisConfig::default())
}

/// Analyze from bytecode entry with inferred continuations and explicit resource bounds.
pub fn analyze_contextual_with_config(
    program: &Program,
    config: ContextualAnalysisConfig,
) -> ContextualCfg {
    let Some(entry) = program.blocks.first().map(|block| block.id) else {
        return ContextualCfg::default()
    };
    analyze_contextual_from(
        program,
        entry,
        AbstractState::new(),
        &infer_continuations(program),
        config,
    )
}

/// Analyze from an explicit public entry and abstract state under shrinking continuation context.
pub fn analyze_contextual_from(
    program: &Program,
    entry: BlockId,
    initial_state: AbstractState,
    hints: &ContinuationHints,
    config: ContextualAnalysisConfig,
) -> ContextualCfg {
    if program.blocks.get(entry.index()).is_none_or(|block| block.id != entry) {
        return ContextualCfg::default()
    }

    let entry_point = ContextualPoint { block: entry, context: AnalysisContext::new(entry) };
    let mut result = ContextualCfg::default();
    result.entry_states.insert(entry_point.clone(), initial_state);
    #[cfg(feature = "smt")]
    let mut smt = config.values.smt.map(SmtRefiner::new);
    let mut worklist = VecDeque::from([entry_point]);

    while let Some(point) = worklist.pop_front() {
        if result.analysis_iterations >= config.max_iterations {
            result.budget_exhausted_points.insert(point);
            result.budget_exhausted_points.extend(worklist);
            break
        }
        result.analysis_iterations += 1;
        let entry_state = result.entry_states[&point].clone();
        let Some(exit) = execute_block(
            program,
            point.block,
            entry_state,
            &mut result.expressions,
            &mut result.state_versions,
            config.values.max_value_set,
        ) else {
            result.exit_states.remove(&point);
            result.invalid_stack_points.insert(point);
            continue
        };
        result.exit_states.insert(point.clone(), exit.clone());

        let successors = successors(
            program,
            &point,
            &exit,
            hints,
            config,
            &result.expressions,
            &mut result.unresolved_jumps,
            &mut result.invalid_jump_points,
            &mut result.pruned_branches,
            #[cfg(feature = "smt")]
            &mut smt,
        );
        for successor in successors {
            let target = budget_context(program, successor.target, config, &mut result);
            result.edges.insert(ContextualEdge {
                source: point.clone(),
                target: target.clone(),
                kind: successor.kind,
            });
            let changed = match result.entry_states.get(&target) {
                Some(previous) => {
                    let joined = previous.join(
                        &successor.state,
                        config.values.max_value_set,
                        &mut result.state_versions,
                    );
                    if &joined == previous {
                        false
                    } else {
                        result.entry_states.insert(target.clone(), joined);
                        true
                    }
                }
                None => {
                    result.entry_states.insert(target.clone(), successor.state);
                    true
                }
            };
            if changed {
                worklist.push_back(target);
            }
        }
    }

    #[cfg(feature = "smt")]
    if let Some(smt) = smt {
        result.smt_stats = smt.stats();
    }
    result
}

#[derive(Clone, Debug)]
struct ContextualSuccessor {
    target: ContextualPoint,
    kind: ContextualEdgeKind,
    state: AbstractState,
}

fn successors(
    program: &Program,
    point: &ContextualPoint,
    exit: &BlockExit,
    hints: &ContinuationHints,
    config: ContextualAnalysisConfig,
    expressions: &ExpressionArena,
    unresolved_jumps: &mut BTreeSet<ContextualPoint>,
    invalid_jump_points: &mut BTreeSet<ContextualPoint>,
    pruned_branches: &mut BTreeSet<(ContextualPoint, bool)>,
    #[cfg(feature = "smt")] smt: &mut Option<SmtRefiner>,
) -> Vec<ContextualSuccessor> {
    let block = &program.blocks[point.block.index()];
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
            pruned_branches.insert((point.clone(), true));
        }
        if !take_false {
            pruned_branches.insert((point.clone(), false));
        }
    }
    let true_state = take_true.then(|| assumed_state(exit, true, expressions)).flatten();
    let false_state = take_false.then(|| assumed_state(exit, false, expressions)).flatten();
    let mut successors = Vec::new();

    if let Some(true_state) = true_state {
        if matches!(block.terminator, BlockTerminator::Jump | BlockTerminator::ConditionalJump) {
            let kind = if block.terminator == BlockTerminator::ConditionalJump {
                ContextualEdgeKind::ConditionalTrue
            } else {
                ContextualEdgeKind::Jump
            };
            match exit.jump_target.as_ref() {
                Some(AbstractValue::Known(targets)) => {
                    for target in targets {
                        match valid_target(program, *target) {
                            Some(target) => successors.push(contextual_successor(
                                point,
                                target,
                                kind,
                                true_state.clone(),
                                hints,
                                config.max_context_depth,
                            )),
                            None => {
                                invalid_jump_points.insert(point.clone());
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
                                pruned_branches.insert((point.clone(), true));
                            }
                            for target in models.targets {
                                if let Some(target) = valid_target(program, target) {
                                    successors.push(contextual_successor(
                                        point,
                                        target,
                                        kind,
                                        true_state.clone(),
                                        hints,
                                        config.max_context_depth,
                                    ));
                                }
                            }
                            if !models.complete {
                                unresolved_jumps.insert(point.clone());
                            }
                        }
                    }
                    if !resolved {
                        for edge in &block.static_edges {
                            if edge.kind == EdgeKind::Jump {
                                resolved = true;
                                successors.push(contextual_successor(
                                    point,
                                    edge.target,
                                    kind,
                                    true_state.clone(),
                                    hints,
                                    config.max_context_depth,
                                ));
                            }
                        }
                    }
                    if !resolved {
                        unresolved_jumps.insert(point.clone());
                    }
                }
                Some(AbstractValue::Unknown) | None => {
                    let mut resolved = false;
                    for edge in &block.static_edges {
                        if edge.kind == EdgeKind::Jump {
                            resolved = true;
                            successors.push(contextual_successor(
                                point,
                                edge.target,
                                kind,
                                true_state.clone(),
                                hints,
                                config.max_context_depth,
                            ));
                        }
                    }
                    if !resolved {
                        unresolved_jumps.insert(point.clone());
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
                successors.push(contextual_successor(
                    point,
                    edge.target,
                    ContextualEdgeKind::ConditionalFalse,
                    false_state,
                    hints,
                    config.max_context_depth,
                ));
            }
        }
    } else if block.terminator == BlockTerminator::Fallthrough {
        if let Some(edge) =
            block.static_edges.iter().find(|edge| edge.kind == EdgeKind::Fallthrough)
        {
            successors.push(contextual_successor(
                point,
                edge.target,
                ContextualEdgeKind::Fallthrough,
                exit.state.clone(),
                hints,
                config.max_context_depth,
            ));
        }
    }

    successors
}

fn valid_target(program: &Program, target: U256) -> Option<BlockId> {
    let pc = usize::try_from(target).ok()?;
    program.is_valid_jumpdest(pc).then(|| program.block_at(pc).expect("validated block").id)
}

fn contextual_successor(
    source: &ContextualPoint,
    target: BlockId,
    kind: ContextualEdgeKind,
    state: AbstractState,
    hints: &ContinuationHints,
    max_depth: usize,
) -> ContextualSuccessor {
    let context = transition_context(&source.context, source.block, target, kind, hints, max_depth);
    ContextualSuccessor { target: ContextualPoint { block: target, context }, kind, state }
}

fn transition_context(
    current: &AnalysisContext,
    source: BlockId,
    target: BlockId,
    kind: ContextualEdgeKind,
    hints: &ContinuationHints,
    max_depth: usize,
) -> AnalysisContext {
    let mut next = current.clone();

    // A matching continuation is stronger evidence than a local call pattern. Cut the matching
    // call and every nested frame, mirroring returns on the dynamic EVM stack.
    if kind == ContextualEdgeKind::Jump && next.cut_to_continuation(target) {
        return next
    }

    if kind == ContextualEdgeKind::Jump {
        if let Some(continuations) = hints.for_caller(source) {
            next.push(
                CallFrame { caller: source, continuations: continuations.clone() },
                max_depth,
            );
        }
    }
    next
}

fn budget_context(
    program: &Program,
    mut point: ContextualPoint,
    config: ContextualAnalysisConfig,
    result: &mut ContextualCfg,
) -> ContextualPoint {
    let existing =
        result.entry_states.keys().filter(|existing| existing.block == point.block).count();
    if !result.entry_states.contains_key(&point) && existing >= config.max_contexts_per_block {
        result.collapsed_contexts.insert(point.clone());
        point.context = AnalysisContext::collapsed(point.context.public_entry);
    }

    debug_assert_eq!(program.blocks[point.block.index()].id, point.block);
    point
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{hardfork::HardFork, opcodes};

    fn program(bytecode: &[u8]) -> Program {
        Program::decode(bytecode, HardFork::Latest)
    }

    #[test]
    fn infers_a_call_continuation_and_shrinks_on_return() {
        let program = program(&[
            opcodes::PUSH1,
            0x07,
            opcodes::PUSH1,
            0x09,
            opcodes::JUMP,
            opcodes::STOP,
            opcodes::STOP,
            opcodes::JUMPDEST,
            opcodes::STOP,
            opcodes::JUMPDEST,
            opcodes::JUMP,
        ]);
        let hints = infer_continuations(&program);
        let caller = program.blocks[0].id;
        let continuation = program.block_at(7).expect("continuation").id;
        let callee = program.block_at(9).expect("callee").id;

        assert_eq!(hints.for_caller(caller), Some(&BTreeSet::from([continuation])));

        let cfg = analyze_contextual(&program);
        let callee_point =
            cfg.entry_states.keys().find(|point| point.block == callee).expect("callee context");
        assert_eq!(callee_point.context.private_calls().len(), 1);
        let continuation_point = cfg
            .entry_states
            .keys()
            .find(|point| point.block == continuation)
            .expect("returned continuation");
        assert!(continuation_point.context.private_calls().is_empty());
    }

    #[test]
    fn nested_returns_cut_only_through_the_matching_call() {
        let program = program(&[
            opcodes::JUMPDEST,
            opcodes::STOP,
            opcodes::JUMPDEST,
            opcodes::STOP,
            opcodes::JUMPDEST,
            opcodes::STOP,
            opcodes::JUMPDEST,
            opcodes::STOP,
        ]);
        let blocks = program.blocks.iter().map(|block| block.id).collect::<Vec<_>>();
        let mut hints = ContinuationHints::default();
        hints.by_caller.insert(blocks[0], BTreeSet::from([blocks[3]]));
        hints.by_caller.insert(blocks[1], BTreeSet::from([blocks[2]]));
        let root = AnalysisContext::new(blocks[0]);
        let outer =
            transition_context(&root, blocks[0], blocks[1], ContextualEdgeKind::Jump, &hints, 20);
        let inner =
            transition_context(&outer, blocks[1], blocks[0], ContextualEdgeKind::Jump, &hints, 20);

        let after_inner =
            transition_context(&inner, blocks[2], blocks[2], ContextualEdgeKind::Jump, &hints, 20);
        assert_eq!(after_inner.private_calls().len(), 1);
        let after_outer = transition_context(
            &after_inner,
            blocks[3],
            blocks[3],
            ContextualEdgeKind::Jump,
            &hints,
            20,
        );
        assert!(after_outer.private_calls().is_empty());
    }

    #[test]
    fn context_depth_is_bounded_and_records_truncation() {
        let program = program(&[
            opcodes::JUMPDEST,
            opcodes::STOP,
            opcodes::JUMPDEST,
            opcodes::STOP,
            opcodes::JUMPDEST,
            opcodes::STOP,
        ]);
        let blocks = program.blocks.iter().map(|block| block.id).collect::<Vec<_>>();
        let mut context = AnalysisContext::new(blocks[0]);
        context.push(CallFrame { caller: blocks[0], continuations: BTreeSet::new() }, 1);
        context.push(CallFrame { caller: blocks[1], continuations: BTreeSet::new() }, 1);

        assert_eq!(context.private_calls().len(), 1);
        assert_eq!(context.private_calls()[0].caller(), blocks[1]);
        assert!(context.is_truncated());
    }

    #[test]
    fn separates_two_calling_contexts_at_a_shared_callee() {
        let program = program(&[
            opcodes::CALLVALUE,
            opcodes::PUSH1,
            0x0c,
            opcodes::JUMPI,
            opcodes::PUSH1,
            0x14,
            opcodes::PUSH1,
            0x12,
            opcodes::JUMP,
            opcodes::STOP,
            opcodes::STOP,
            opcodes::STOP,
            opcodes::JUMPDEST,
            opcodes::PUSH1,
            0x16,
            opcodes::PUSH1,
            0x12,
            opcodes::JUMP,
            opcodes::JUMPDEST,
            opcodes::JUMP,
            opcodes::JUMPDEST,
            opcodes::STOP,
            opcodes::JUMPDEST,
            opcodes::STOP,
        ]);
        let callee = program.block_at(0x12).expect("shared callee").id;
        let cfg = analyze_contextual(&program);
        let contexts = cfg.entry_states.keys().filter(|point| point.block == callee).count();

        assert_eq!(contexts, 2);
    }

    #[test]
    fn reports_points_left_by_iteration_budget() {
        let program = program(&[opcodes::JUMPDEST, opcodes::STOP]);
        let entry = program.blocks[0].id;
        let cfg = analyze_contextual_from(
            &program,
            entry,
            AbstractState::new(),
            &ContinuationHints::default(),
            ContextualAnalysisConfig { max_iterations: 0, ..Default::default() },
        );

        assert_eq!(cfg.analysis_iterations, 0);
        assert_eq!(cfg.budget_exhausted_points.len(), 1);
        assert!(cfg.entry_states.contains_key(cfg.budget_exhausted_points.first().unwrap()));
    }

    #[test]
    fn zero_context_budget_collapses_excess_contexts() {
        let program = program(&[opcodes::JUMPDEST, opcodes::STOP]);
        let entry = program.blocks[0].id;
        let mut result = ContextualCfg::default();
        result.entry_states.insert(
            ContextualPoint { block: entry, context: AnalysisContext::new(entry) },
            AbstractState::new(),
        );
        let point = ContextualPoint {
            block: entry,
            context: AnalysisContext {
                public_entry: entry,
                private_calls: vec![CallFrame { caller: entry, continuations: BTreeSet::new() }],
                truncated: false,
            },
        };
        let budgeted = budget_context(
            &program,
            point,
            ContextualAnalysisConfig { max_contexts_per_block: 1, ..Default::default() },
            &mut result,
        );

        assert!(budgeted.context.is_truncated());
        assert!(budgeted.context.private_calls().is_empty());
        assert_eq!(result.collapsed_contexts.len(), 1);
    }
}
