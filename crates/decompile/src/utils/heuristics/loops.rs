use futures::future::BoxFuture;
use hashbrown::HashSet;
use heimdall_vm::{core::vm::State, ext::exec::LoopInfo};
use tracing::trace;

use crate::{core::analyze::AnalyzerState, interfaces::AnalyzedFunction, Error};

/// Analyzer state extension for loop tracking
#[derive(Debug, Clone, Default)]
pub(crate) struct LoopAnalyzerState {
    /// Stack of active loops (for nested loops)
    pub active_loops: Vec<LoopInfo>,

    /// Set of PCs that are loop headers
    pub loop_headers: HashSet<u128>,

    /// Set of PCs that are loop exit points (JUMPI condition PCs)
    pub loop_exits: HashSet<u128>,

    /// Current nesting depth
    pub depth: usize,
}

/// Check if an operation is a loop header JUMPDEST
fn is_loop_header(state: &State, detected_loops: &[LoopInfo]) -> Option<LoopInfo> {
    let instruction = &state.last_instruction;

    // JUMPDEST opcode
    if instruction.opcode != 0x5b {
        return None;
    }

    let current_pc = instruction.instruction;

    for loop_info in detected_loops {
        if current_pc == loop_info.header_pc {
            return Some(loop_info.clone());
        }
    }

    None
}

/// Check if an operation is a loop condition JUMPI
fn is_loop_condition(state: &State, detected_loops: &[LoopInfo]) -> Option<LoopInfo> {
    let instruction = &state.last_instruction;

    // JUMPI opcode
    if instruction.opcode != 0x57 {
        return None;
    }

    let current_pc = instruction.instruction;

    for loop_info in detected_loops {
        if current_pc == loop_info.condition_pc {
            return Some(loop_info.clone());
        }
    }

    None
}

pub(crate) fn loop_heuristic<'a>(
    function: &'a mut AnalyzedFunction,
    state: &'a State,
    analyzer_state: &'a mut AnalyzerState,
    detected_loops: &'a [LoopInfo],
) -> BoxFuture<'a, Result<(), Error>> {
    Box::pin(async move {
        let instruction = &state.last_instruction;
        let current_pc = instruction.instruction;
        let opcode = instruction.opcode;

        // Log opcodes relevant to loops for tracing
        if opcode == 0x5b || opcode == 0x57 {
            trace!(
                "loop_heuristic: pc={}, opcode=0x{:02x} ({})",
                current_pc,
                opcode,
                if opcode == 0x5b { "JUMPDEST" } else { "JUMPI" }
            );
        }

        // Check if we're at the loop's JUMPI (condition check)
        // We emit the loop header here because the loop header JUMPDEST is skipped during trace creation
        if let Some(loop_info) = is_loop_condition(state, detected_loops) {
            // Check if we've already entered this loop (don't emit twice)
            let already_in_loop = analyzer_state
                .loop_state
                .active_loops
                .iter()
                .any(|l| l.condition_pc == loop_info.condition_pc);

            if !already_in_loop {
                // First time seeing this loop condition - emit the loop header
                trace!(
                    "emitting loop header at condition pc={}: {}",
                    current_pc,
                    loop_info.to_solidity()
                );
                function.logic.push(loop_info.to_solidity());

                // Track that we're in a loop
                analyzer_state.loop_state.active_loops.push(loop_info.clone());
                analyzer_state.loop_state.loop_headers.insert(loop_info.header_pc);
                analyzer_state.loop_state.loop_exits.insert(loop_info.condition_pc);
                analyzer_state.loop_state.depth += 1;
            } else {
                // We've seen this loop condition before - close the loop
                trace!("closing loop at condition pc={}", current_pc);
                function.logic.push("}".to_string());
                analyzer_state.loop_state.active_loops.retain(|l| l.condition_pc != loop_info.condition_pc);
                analyzer_state.loop_state.depth = analyzer_state.loop_state.depth.saturating_sub(1);
            }

            // Mark that we should skip this JUMPI in the solidity heuristic
            analyzer_state.skip_next_jumpi = true;

            return Ok(());
        }

        // Check if we're at a loop header (fallback, in case trace includes JUMPDEST)
        if let Some(loop_info) = is_loop_header(state, detected_loops) {
            trace!(
                "matched loop header at pc={}, emitting: {}",
                current_pc,
                loop_info.to_solidity()
            );

            // Check if we've already entered this loop
            let already_in_loop = analyzer_state
                .loop_state
                .active_loops
                .iter()
                .any(|l| l.header_pc == loop_info.header_pc);

            if !already_in_loop {
                function.logic.push(loop_info.to_solidity());
                analyzer_state.loop_state.active_loops.push(loop_info.clone());
                analyzer_state.loop_state.loop_headers.insert(loop_info.header_pc);
                analyzer_state.loop_state.loop_exits.insert(loop_info.condition_pc);
                analyzer_state.loop_state.depth += 1;
            }

            return Ok(());
        }

        Ok(())
    })
}

/// Check if an operation is part of loop overhead that should be suppressed
pub(crate) fn is_loop_overhead(state: &State, active_loops: &[LoopInfo]) -> bool {
    let instruction = &state.last_instruction;

    for loop_info in active_loops {
        // Suppress induction variable updates (they're in the for-loop header)
        if let Some(ref iv) = loop_info.induction_var {
            let solidified = instruction
                .input_operations
                .first()
                .map(|op| op.solidify())
                .unwrap_or_default();

            // Check if this is the increment/decrement of the induction var
            if solidified.contains(&iv.name)
                && (solidified.contains("+ 1")
                    || solidified.contains("+ 0x01")
                    || solidified.contains("- 1")
                    || solidified.contains("- 0x01"))
            {
                return true;
            }
        }
    }

    false
}

/// Filter out Solidity 0.8+ overflow check panic paths
pub(crate) fn is_overflow_panic(state: &State) -> bool {
    let instruction = &state.last_instruction;

    // REVERT opcode
    if instruction.opcode != 0xfd {
        return false;
    }

    // Check if we have memory data to inspect
    if instruction.inputs.len() < 2 {
        return false;
    }

    // Safely convert U256 to usize
    let offset: usize = instruction.inputs[0].try_into().unwrap_or(0);
    let size: usize = instruction.inputs[1].try_into().unwrap_or(0);

    if size < 4 {
        return false;
    }

    // Check if memory contains panic selector (0x4e487b71)
    let memory_data = state.memory.read(offset, size);
    if memory_data.len() >= 4 {
        // Panic(uint256) selector is 0x4e487b71
        if memory_data[0..4] == [0x4e, 0x48, 0x7b, 0x71] {
            // If we have the error code, check if it's arithmetic overflow (0x11)
            // or underflow (0x12)
            if memory_data.len() >= 36 {
                let error_code = memory_data[35];
                // 0x11 = overflow, 0x12 = underflow
                if error_code == 0x11 || error_code == 0x12 {
                    return true;
                }
            }
            // Even without the specific code, this is a panic
            return true;
        }
    }

    false
}
