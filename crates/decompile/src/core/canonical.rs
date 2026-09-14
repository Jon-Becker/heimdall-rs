//! Canonical control-flow analysis retained alongside legacy decompiler output.

use std::collections::BTreeMap;

use hashbrown::HashMap;
use heimdall_vm::core::{
    context::{analyze_contextual, ContextualCfg, ContextualPoint},
    hardfork::HardFork,
    program::{BlockId, Program},
    ssa::{build_contextual_ssa, ContextualSsa},
};

/// Existing selector discovery mapped onto canonical basic blocks.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalFunctionEntry {
    /// Bytecode program counter reported by selector discovery.
    pub entry_pc: u128,
    /// Canonical block beginning at `entry_pc`, when the mapping is valid.
    pub block: Option<BlockId>,
}

/// Canonical program and contextual analysis retained as one ID-consistent artifact.
///
/// Expression and state-version IDs in `cfg` are meaningful only within this artifact and must not
/// be combined with arenas from another analysis.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalAnalysis {
    /// Canonically decoded runtime bytecode.
    pub program: Program,
    /// Context-sensitive abstract CFG, including all uncertainty diagnostics.
    pub cfg: ContextualCfg,
    /// Contextual stack SSA and effect operands derived from `cfg`.
    pub ssa: ContextualSsa,
    /// Legacy-discovered selectors annotated with canonical entry blocks.
    pub function_entries: BTreeMap<String, CanonicalFunctionEntry>,
}

impl CanonicalAnalysis {
    /// Context-sensitive entry points associated with a discovered selector.
    pub fn points_for_selector(&self, selector: &str) -> impl Iterator<Item = &ContextualPoint> {
        let block = self.function_entries.get(selector).and_then(|entry| entry.block);
        self.cfg.entry_states.keys().filter(move |point| Some(point.block) == block)
    }
}

pub(crate) fn build_canonical_analysis(
    bytecode: &[u8],
    hardfork: HardFork,
    selectors: &HashMap<String, u128>,
) -> CanonicalAnalysis {
    let program = Program::decode(bytecode, hardfork);
    let cfg = analyze_contextual(&program);
    let ssa = build_contextual_ssa(&cfg);
    let function_entries = selectors
        .iter()
        .map(|(selector, &entry_pc)| {
            let block = usize::try_from(entry_pc)
                .ok()
                .and_then(|entry_pc| program.block_at(entry_pc))
                .map(|block| block.id);
            (selector.clone(), CanonicalFunctionEntry { entry_pc, block })
        })
        .collect();
    CanonicalAnalysis { program, cfg, ssa, function_entries }
}

#[cfg(test)]
mod tests {
    use heimdall_vm::core::opcodes;

    use super::*;

    #[test]
    fn maps_selectors_without_dropping_invalid_entries() {
        let bytecode = [opcodes::STOP, opcodes::JUMPDEST, opcodes::STOP];
        let selectors =
            HashMap::from([("0x11111111".to_owned(), 1), ("0x22222222".to_owned(), u128::MAX)]);
        let analysis = build_canonical_analysis(&bytecode, HardFork::Latest, &selectors);

        assert_eq!(
            analysis.function_entries["0x11111111"].block,
            analysis.program.block_at(1).map(|block| block.id)
        );
        assert_eq!(analysis.function_entries["0x22222222"].block, None);
        assert_eq!(analysis.points_for_selector("0x11111111").count(), 0);
    }

    #[test]
    fn retains_symbolic_and_versioned_analysis_arenas() {
        let bytecode = [
            opcodes::CALLER,
            opcodes::PUSH0,
            opcodes::SSTORE,
            opcodes::PUSH1,
            1,
            opcodes::SLOAD,
            opcodes::STOP,
        ];
        let analysis = build_canonical_analysis(&bytecode, HardFork::Latest, &HashMap::new());

        assert!(!analysis.cfg.expressions.is_empty());
        assert!(analysis.cfg.state_versions.version_count() > 3);
        assert_eq!(analysis.cfg.exit_states.len(), analysis.cfg.entry_states.len());
        assert_eq!(analysis.ssa.blocks.len(), analysis.cfg.entry_states.len());
        assert!(analysis
            .ssa
            .blocks
            .values()
            .flat_map(|block| &block.effects)
            .any(|effect| effect.kind ==
                heimdall_vm::core::analysis::InstructionEffectKind::StorageWrite));
    }
}
