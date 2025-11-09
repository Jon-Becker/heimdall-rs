use hashbrown::HashMap;

use alloy::primitives::U256;
use heimdall_common::constants::{MEMORY_REGEX, STORAGE_REGEX};
use tracing::trace;

use crate::core::stack::{Stack, StackFrame};

use super::jump_frame::JumpFrame;

/// Given two stacks A and B, return A - B, i.e. the items in A that are not in B.
/// This operation takes order into account, so if A = [1, 2, 3] and B = [1, 3, 2], then A - B =
/// [2]. This is referred to as the "stack diff"
pub(super) fn stack_diff(a: &Stack, b: &Stack) -> Vec<StackFrame> {
    let mut diff = Vec::new();

    for (i, frame) in a.stack.iter().enumerate() {
        if b.stack.len() <= i || frame != &b.stack[i] {
            diff.push(frame.clone());
        }
    }

    diff
}

/// Check if the given stack contains too many items to feasibly
/// reach the bottom of the stack without being a loop.
pub(super) fn stack_contains_too_many_items(stack: &Stack) -> bool {
    if stack.size() > 320 {
        // 320 is an arbitrary number, i picked it randomly :D
        trace!("jump matches loop-detection heuristic: 'stack_contains_too_many_items'",);
        return true;
    }

    false
}

/// Check if the current jump frame has a stack depth less than the max stack depth of all previous
/// matching jumps. If yes, the stack is not growing and we likely have a loop.
pub(super) fn jump_stack_depth_less_than_max_stack_depth(
    current_jump_frame: &JumpFrame,
    handled_jumps: &HashMap<JumpFrame, Vec<Stack>>,
) -> bool {
    // (1) get all keys that match current_jump_frame.pc and current_jump_frame.jumpdest
    let matching_keys = handled_jumps
        .keys()
        .filter(|key| {
            key.pc == current_jump_frame.pc && key.jumpdest == current_jump_frame.jumpdest
        })
        .collect::<Vec<&JumpFrame>>();

    // (a) get the max stack_depth of all matching keys
    let max_stack_depth = matching_keys.iter().map(|key| key.stack_depth).max().unwrap_or(0);

    // (b) if the current stack depth is less than the max stack depth, we don't need to
    // continue.
    if current_jump_frame.stack_depth < max_stack_depth {
        trace!(
            "jump matches loop-detection heuristic: 'jump_stack_depth_less_than_max_stack_depth'"
        );
        trace!("jump terminated.");
        return true;
    }

    false
}

/// Check if the given stack contains too many of the same item.
/// If the stack contains more than 16 of the same item (with the same sources), it is considered a
/// loop.
pub(super) fn stack_contains_too_many_of_the_same_item(stack: &Stack) -> bool {
    if stack.size() > 16 && stack.stack.iter().any(|frame| {
        let solidified_frame_source = frame.operation.solidify();
        stack.stack.iter().filter(|f| f.operation.solidify() == solidified_frame_source).count() >=
            16
    }) {
        trace!("jump matches loop-detection heuristic: 'stack_contains_too_many_of_the_same_item'",);
        return true;
    }

    false
}

/// Check if the stack contains any item with a source operation depth > 16. If so, it is considered
/// a loop. This check originates from the `stack too deep` error in Solidity due to the `DUP16` and
/// `SWAP16` operation limitations.
pub(super) fn stack_item_source_depth_too_deep(stack: &Stack) -> bool {
    if stack.stack.iter().any(|frame| frame.operation.depth() > 16) {
        trace!("jump matches loop-detection heuristic: 'stack_item_source_depth_too_deep'");
        return true;
    }

    false
}

/// Compare the stack diff to the given jump condition and determine if the jump condition appears
/// to be the condition of a loop.
pub(super) fn jump_condition_appears_recursive(
    stack_diff: &[StackFrame],
    jump_condition: &str,
) -> bool {
    // check if the jump condition appears in the stack diff more than once, this is likely a loop
    if stack_diff
        .iter()
        .map(|frame| frame.operation.solidify())
        .any(|solidified| jump_condition.contains(&solidified))
    {
        trace!("jump matches loop-detection heuristic: 'jump_condition_appears_recursive'");
        return true;
    }

    false
}

/// Check if the jump condition contains a memory access that is modified within the stack diff.
pub(super) fn jump_condition_contains_mutated_memory_access(
    stack_diff: &[StackFrame],
    jump_condition: &str,
) -> bool {
    let mut memory_accesses = MEMORY_REGEX.find_iter(jump_condition);
    if stack_diff.iter().any(|frame| {
        memory_accesses.any(|_match| {
            if _match.is_err() {
                return false;
            }

            let memory_access = match _match {
                Ok(access) => access,
                Err(_) => return false,
            };

            let slice = &jump_condition[memory_access.start()..memory_access.end()];
            frame.operation.solidify().contains(slice)
        })
    }) {
        trace!("jump matches loop-detection heuristic: 'jump_condition_contains_mutated_memory_access'");
        return true;
    }

    false
}

/// Check if the jump condition contains a storage access that is modified within the stack diff.
pub(super) fn jump_condition_contains_mutated_storage_access(
    stack_diff: &[StackFrame],
    jump_condition: &str,
) -> bool {
    let mut storage_accesses = STORAGE_REGEX.find_iter(jump_condition);
    if stack_diff.iter().any(|frame| {
        storage_accesses.any(|_match| {
            if _match.is_err() {
                return false;
            }
            let storage_access = match _match {
                Ok(access) => access,
                Err(_) => return false,
            };
            let slice = &jump_condition[storage_access.start()..storage_access.end()];
            frame.operation.solidify().contains(slice)
        })
    }) {
        trace!("jump matches loop-detection heuristic: 'jump_condition_contains_mutated_storage_access'");
        return true;
    }

    false
}

/// check if all stack diffs for all historical stacks are exactly length 1, and the same
pub(super) fn historical_diffs_approximately_equal(
    stack: &Stack,
    historical_stacks: &[Stack],
) -> bool {
    // break if historical_stacks.len() < 4
    // this is an arbitrary number, i picked it randomly :D
    if historical_stacks.len() < 4 {
        return false;
    }

    // get the stack diffs for all historical stacks
    let mut stack_diffs = Vec::new();
    for historical_stack in historical_stacks {
        stack_diffs.push(
            stack_diff(stack, historical_stack)
                .iter()
                .map(|frame| frame.value)
                .collect::<Vec<U256>>(),
        );
    }

    // get stack length / 10, rounded up as threshold
    let threshold = (stack.size() as f64 / 10f64).ceil() as usize;

    // check if all stack diffs are similar
    if !stack_diffs.iter().all(|diff| diff.len() <= threshold) {
        return false;
    }

    // check if all stack diffs are the same
    if !stack_diffs
        .iter()
        .all(|diff| diff.first() == stack_diffs.first().unwrap_or(&vec![]).first())
    {
        return false;
    }

    trace!("jump matches loop-detection heuristic: 'jump_condition_historical_diffs_approximately_equal'");

    true
}

/// Check if any stack position shows a consistent pattern (increasing, decreasing, or alternating)
/// across more than 32 iterations. This indicates a loop counter or iterator.
pub(super) fn stack_position_shows_pattern(
    stack: &Stack,
    historical_stacks: &[Stack],
) -> bool {
    // Start checking after just 10 iterations to catch loops earlier
    if historical_stacks.len() < 10 {
        return false;
    }

    trace!("checking stack pattern with {} historical stacks", historical_stacks.len());

    // Determine the maximum stack size to check all positions
    let max_size = stack
        .size()
        .max(historical_stacks.iter().map(|s| s.size()).max().unwrap_or(0));

    // For each stack position, collect values across all historical stacks
    for position in 0..max_size {
        let mut values: Vec<U256> = Vec::new();

        // Collect values at this position from all historical stacks
        for hist_stack in historical_stacks {
            if let Some(frame) = hist_stack.stack.get(position) {
                values.push(frame.value);
            }
        }

        // Also include current stack value
        if let Some(frame) = stack.stack.get(position) {
            values.push(frame.value);
        }

        // Need at least 10 values to detect a meaningful pattern
        if values.len() < 10 {
            continue;
        }

        // Check for patterns
        if is_consistently_increasing(&values) {
            trace!(
                "jump matches loop-detection heuristic: 'stack_position_shows_pattern' \
                 (increasing at position {})",
                position
            );
            return true;
        }

        if is_consistently_decreasing(&values) {
            trace!(
                "jump matches loop-detection heuristic: 'stack_position_shows_pattern' \
                 (decreasing at position {})",
                position
            );
            return true;
        }

        if is_consistently_alternating(&values) {
            trace!(
                "jump matches loop-detection heuristic: 'stack_position_shows_pattern' \
                 (alternating at position {})",
                position
            );
            return true;
        }
    }

    false
}

/// Check if values show a strong increasing trend (>= 60% of pairs increase)
fn is_consistently_increasing(values: &[U256]) -> bool {
    if values.len() < 2 {
        return false;
    }

    let total_pairs = values.len() - 1;
    let increasing_pairs = values.windows(2).filter(|pair| pair[1] > pair[0]).count();

    // Require at least 60% of pairs to be increasing
    increasing_pairs as f64 / total_pairs as f64 >= 0.6
}

/// Check if values show a strong decreasing trend (>= 60% of pairs decrease)
fn is_consistently_decreasing(values: &[U256]) -> bool {
    if values.len() < 2 {
        return false;
    }

    let total_pairs = values.len() - 1;
    let decreasing_pairs = values.windows(2).filter(|pair| pair[1] < pair[0]).count();

    // Require at least 60% of pairs to be decreasing
    decreasing_pairs as f64 / total_pairs as f64 >= 0.6
}

/// Check if values show a strong alternating pattern (>= 60% of triples alternate)
fn is_consistently_alternating(values: &[U256]) -> bool {
    if values.len() < 3 {
        return false;
    }

    let total_triples = values.len() - 2;
    let alternating_triples = values.windows(3).filter(|triple| {
        (triple[1] > triple[0] && triple[2] < triple[1]) ||
            (triple[1] < triple[0] && triple[2] > triple[1])
    }).count();

    // Require at least 60% of triples to alternate
    alternating_triples as f64 / total_triples as f64 >= 0.6
}
