//! Successor resolution and branch feasibility for abstract control flow.

use alloy::primitives::U256;

use super::{transfer::BlockExit, AbstractCfg, AbstractEdgeKind, AbstractState, AbstractValue};
use crate::core::program::{BlockId, BlockTerminator, EdgeKind, Program};

#[derive(Clone, Debug)]
pub(super) struct Successor {
    pub(super) target: BlockId,
    pub(super) kind: AbstractEdgeKind,
    pub(super) state: AbstractState,
}

pub(super) fn successors(
    program: &Program,
    block_id: BlockId,
    exit: &BlockExit,
    result: &mut AbstractCfg,
) -> Vec<Successor> {
    let block = &program.blocks[block_id.index()];
    let (take_true, take_false) = branch_feasibility(exit.condition.as_ref());
    let mut successors = Vec::new();
    let mut add_successor = |target, kind| {
        successors.push(Successor { target, kind, state: exit.state.clone() });
    };

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
                            add_successor(target_block.id, kind);
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
                        add_successor(edge.target, kind);
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
            add_successor(edge.target, AbstractEdgeKind::ConditionalFalse);
        }
    } else if block.terminator == BlockTerminator::Fallthrough {
        if let Some(edge) =
            block.static_edges.iter().find(|edge| edge.kind == EdgeKind::Fallthrough)
        {
            add_successor(edge.target, AbstractEdgeKind::Fallthrough);
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
