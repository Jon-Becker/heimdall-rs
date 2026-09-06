//! Context-sensitive stack SSA derived from canonical abstract CFG states.
//!
//! Abstract interpretation intentionally joins values to reach a fixpoint. This module retains the
//! predecessor relationship behind those joins as explicit phi nodes and maps instruction effects
//! onto stable SSA operands without flattening calling contexts.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use super::{
    analysis::{
        AbstractStack, AbstractValue, EffectStateRoots, InstructionEffect, InstructionEffectKind,
    },
    context::{ContextualCfg, ContextualPoint},
};

/// Stable identifier for one canonical SSA value.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SsaValueId(usize);

impl SsaValueId {
    /// Return this value's arena index.
    pub fn index(self) -> usize {
        self.0
    }
}

/// Definition of one canonical SSA value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SsaValue {
    /// Concrete, symbolic, mixed, or unknown abstract value.
    Abstract(AbstractValue),
    /// Entry-stack merge at a contextual block.
    Phi {
        /// Contextual block containing the merge.
        point: ContextualPoint,
        /// Zero-based stack slot from the top.
        slot: usize,
    },
}

/// Origin of a value entering a phi node.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SsaPredecessor {
    /// Explicit seed supplied to contextual analysis.
    Entry,
    /// Executed predecessor contextual block.
    Point(ContextualPoint),
}

/// One predecessor-qualified phi input.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SsaPhiInput {
    /// Control-flow predecessor.
    pub predecessor: SsaPredecessor,
    /// Value supplied by that predecessor.
    pub value: SsaValueId,
}

/// Explicit merge for one contextual entry-stack slot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SsaPhi {
    /// SSA result defined by this phi.
    pub result: SsaValueId,
    /// Stack slot defined by this phi.
    pub slot: usize,
    /// Inputs in deterministic predecessor order.
    pub inputs: Vec<SsaPhiInput>,
}

/// Effect record rewritten to stable SSA operands.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SsaEffect {
    /// Bytecode program counter.
    pub pc: usize,
    /// Raw EVM opcode.
    pub opcode: u8,
    /// Observable effect category.
    pub kind: InstructionEffectKind,
    /// SSA operands in EVM pop order.
    pub inputs: Vec<SsaValueId>,
    /// SSA outputs produced by the effect.
    pub outputs: Vec<SsaValueId>,
    /// Persistent roots before the effect.
    pub before: EffectStateRoots,
    /// Persistent roots after the effect.
    pub after: EffectStateRoots,
}

/// SSA view of one contextual canonical block.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SsaBlock {
    /// SSA values occupying the explicit entry stack, top first.
    pub entry_stack: Vec<SsaValueId>,
    /// Phi nodes required at this block entry.
    pub phis: Vec<SsaPhi>,
    /// Observable effects in instruction order.
    pub effects: Vec<SsaEffect>,
}

/// Context-sensitive SSA graph and value arena.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ContextualSsa {
    values: Vec<SsaValue>,
    /// Blocks keyed by full shrinking continuation context.
    pub blocks: BTreeMap<ContextualPoint, SsaBlock>,
}

impl ContextualSsa {
    /// Look up an SSA value definition.
    pub fn value(&self, id: SsaValueId) -> Option<&SsaValue> {
        self.values.get(id.0)
    }

    /// Number of SSA values retained by this graph.
    pub fn value_count(&self) -> usize {
        self.values.len()
    }
}

/// Build contextual stack SSA and effect operands from a completed canonical analysis.
pub fn build_contextual_ssa(cfg: &ContextualCfg) -> ContextualSsa {
    let mut ssa = ContextualSsa::default();
    let mut atoms = HashMap::new();
    let mut incoming_by_target = BTreeMap::<ContextualPoint, BTreeSet<ContextualPoint>>::new();
    for edge in &cfg.edges {
        incoming_by_target.entry(edge.target.clone()).or_default().insert(edge.source.clone());
    }
    let mut points = cfg.entry_states.keys().cloned().collect::<Vec<_>>();
    points.sort();

    for point in points {
        let state = &cfg.entry_states[&point];
        let mut block = SsaBlock::default();
        for slot in 0..state.stack.values().len() {
            let mut incoming = BTreeMap::new();
            if let Some(seed) =
                cfg.initial_states.get(&point).and_then(|seed| stack_value(&seed.stack, slot))
            {
                let value = intern_abstract(&mut ssa, &mut atoms, seed);
                incoming.insert(SsaPredecessor::Entry, value);
            }
            for predecessor in incoming_by_target.get(&point).into_iter().flatten() {
                let Some(value) = cfg
                    .exit_states
                    .get(predecessor)
                    .and_then(|exit| stack_value(&exit.state.stack, slot))
                else {
                    continue
                };
                let value = intern_abstract(&mut ssa, &mut atoms, value);
                incoming.insert(SsaPredecessor::Point(predecessor.clone()), value);
            }

            if incoming.is_empty() {
                let value =
                    intern_abstract(&mut ssa, &mut atoms, state.stack.values()[slot].clone());
                block.entry_stack.push(value);
                continue
            }
            let first = *incoming.values().next().expect("non-empty incoming values");
            if incoming.values().all(|value| *value == first) {
                block.entry_stack.push(first);
                continue
            }

            let result = SsaValueId(ssa.values.len());
            ssa.values.push(SsaValue::Phi { point: point.clone(), slot });
            block.entry_stack.push(result);
            block.phis.push(SsaPhi {
                result,
                slot,
                inputs: incoming
                    .into_iter()
                    .map(|(predecessor, value)| SsaPhiInput { predecessor, value })
                    .collect(),
            });
        }

        if let Some(exit) = cfg.exit_states.get(&point) {
            block.effects = exit
                .effects
                .iter()
                .map(|effect| lower_effect(effect, &mut ssa, &mut atoms))
                .collect();
        }
        ssa.blocks.insert(point, block);
    }
    ssa
}

fn stack_value(stack: &AbstractStack, slot: usize) -> Option<AbstractValue> {
    stack
        .values()
        .get(slot)
        .cloned()
        .or_else(|| stack.has_unknown_tail().then_some(AbstractValue::Unknown))
}

fn intern_abstract(
    ssa: &mut ContextualSsa,
    atoms: &mut HashMap<AbstractValue, SsaValueId>,
    value: AbstractValue,
) -> SsaValueId {
    if let Some(id) = atoms.get(&value) {
        return *id
    }
    let id = SsaValueId(ssa.values.len());
    ssa.values.push(SsaValue::Abstract(value.clone()));
    atoms.insert(value, id);
    id
}

fn lower_effect(
    effect: &InstructionEffect,
    ssa: &mut ContextualSsa,
    atoms: &mut HashMap<AbstractValue, SsaValueId>,
) -> SsaEffect {
    SsaEffect {
        pc: effect.pc,
        opcode: effect.opcode,
        kind: effect.kind,
        inputs: effect
            .inputs
            .iter()
            .cloned()
            .map(|value| intern_abstract(ssa, atoms, value))
            .collect(),
        outputs: effect
            .outputs
            .iter()
            .cloned()
            .map(|value| intern_abstract(ssa, atoms, value))
            .collect(),
        before: effect.before,
        after: effect.after,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{context::analyze_contextual, hardfork::HardFork, opcodes, program::Program};

    fn program(bytecode: &[u8]) -> Program {
        Program::decode(bytecode, HardFork::Latest)
    }

    #[test]
    fn creates_predecessor_qualified_stack_phi_nodes() {
        let program = program(&[
            opcodes::CALLVALUE,
            opcodes::PUSH1,
            10,
            opcodes::JUMPI,
            opcodes::PUSH1,
            1,
            opcodes::PUSH1,
            16,
            opcodes::JUMP,
            opcodes::STOP,
            opcodes::JUMPDEST,
            opcodes::PUSH1,
            2,
            opcodes::PUSH1,
            16,
            opcodes::JUMP,
            opcodes::JUMPDEST,
            opcodes::STOP,
        ]);
        let cfg = analyze_contextual(&program);
        let ssa = build_contextual_ssa(&cfg);
        let join = program.block_at(16).unwrap().id;
        let block = ssa.blocks.iter().find(|(point, _)| point.block == join).unwrap().1;

        assert_eq!(block.phis.len(), 1);
        assert_eq!(block.phis[0].slot, 0);
        assert_eq!(block.phis[0].inputs.len(), 2);
        assert!(block.phis[0]
            .inputs
            .iter()
            .all(|input| matches!(input.predecessor, SsaPredecessor::Point(_))));
        assert!(matches!(ssa.value(block.phis[0].result), Some(SsaValue::Phi { .. })));
    }

    #[test]
    fn lowers_effect_operands_without_flattening_contexts() {
        let program = program(&[opcodes::PUSH1, 7, opcodes::PUSH0, opcodes::SSTORE, opcodes::STOP]);
        let cfg = analyze_contextual(&program);
        let ssa = build_contextual_ssa(&cfg);
        let block = ssa.blocks.values().next().unwrap();

        assert_eq!(block.effects.len(), 1);
        assert_eq!(block.effects[0].kind, InstructionEffectKind::StorageWrite);
        assert_eq!(block.effects[0].inputs.len(), 2);
        assert_ne!(block.effects[0].before.storage, block.effects[0].after.storage);
    }
}
