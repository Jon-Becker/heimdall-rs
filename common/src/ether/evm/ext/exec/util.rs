use ethers::types::U256;

use crate::{
    constants::{MEMORY_REGEX, STORAGE_REGEX},
    debug_max,
    ether::evm::core::stack::{Stack, StackFrame},
    utils::io::logging::Logger,
};

/// Given two stacks A and B, return A - B, i.e. the items in A that are not in B.
/// This operation takes order into account, so if A = [1, 2, 3] and B = [1, 3, 2], then A - B =
/// [2]. This is referred to as the "stack diff"
pub fn stack_diff(a: &Stack, b: &Stack) -> Vec<StackFrame> {
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
pub fn stack_contains_too_many_items(stack: &Stack) -> bool {
    if stack.size() > 320 {
        // 320 is an arbitrary number, i picked it randomly :D
        debug_max!("jump matches loop-detection heuristic: 'stack_contains_too_many_items'",);
        return true
    }

    false
}

/// Check if the given stack contains too many of the same item.
/// If the stack contains more than 16 of the same item (with the same sources), it is considered a
/// loop.
pub fn stack_contains_too_many_of_the_same_item(stack: &Stack) -> bool {
    if stack.size() > 16 && stack.stack.iter().any(|frame| {
        let solidified_frame_source = frame.operation.solidify();
        stack.stack.iter().filter(|f| f.operation.solidify() == solidified_frame_source).count() >=
            16
    }) {
        debug_max!(
            "jump matches loop-detection heuristic: 'stack_contains_too_many_of_the_same_item'",
        );
        return true
    }

    false
}

/// Check if the stack contains any item with a source operation depth > 16. If so, it is considered
/// a loop. This check originates from the `stack too deep` error in Solidity due to the `DUP16` and
/// `SWAP16` operation limitations.
pub fn stack_item_source_depth_too_deep(stack: &Stack) -> bool {
    if stack.stack.iter().any(|frame| frame.operation.depth() > 16) {
        // get a new logger
        let logger = Logger::default();

        logger
            .debug_max("jump matches loop-detection heuristic: 'stack_item_source_depth_too_deep'");
        return true
    }

    false
}

/// Compare the stack diff to the given jump condition and determine if the jump condition appears
/// to be the condition of a loop.
pub fn jump_condition_appears_recursive(stack_diff: &[StackFrame], jump_condition: &str) -> bool {
    // check if the jump condition appears in the stack diff more than once, this is likely a loop
    if stack_diff
        .iter()
        .map(|frame| frame.operation.solidify())
        .any(|solidified| jump_condition.contains(&solidified))
    {
        // get a new logger
        let logger = Logger::default();

        logger
            .debug_max("jump matches loop-detection heuristic: 'jump_condition_appears_recursive'");
        return true
    }

    false
}

/// Check if the jump condition contains a memory access that is modified within the stack diff.
pub fn jump_condition_contains_mutated_memory_access(
    stack_diff: &[StackFrame],
    jump_condition: &str,
) -> bool {
    let mut memory_accesses = MEMORY_REGEX.find_iter(jump_condition);
    if stack_diff.iter().any(|frame| {
        memory_accesses.any(|_match| {
            if _match.is_err() {
                return false
            }
            let memory_access = _match.unwrap();
            let slice = &jump_condition[memory_access.start()..memory_access.end()];
            frame.operation.solidify().contains(slice)
        })
    }) {
        debug_max!("jump matches loop-detection heuristic: 'jump_condition_contains_mutated_memory_access'");
        return true
    }

    false
}

/// Check if the jump condition contains a storage access that is modified within the stack diff.
pub fn jump_condition_contains_mutated_storage_access(
    stack_diff: &[StackFrame],
    jump_condition: &str,
) -> bool {
    let mut storage_accesses = STORAGE_REGEX.find_iter(jump_condition);
    if stack_diff.iter().any(|frame| {
        storage_accesses.any(|_match| {
            if _match.is_err() {
                return false
            }
            let storage_access = _match.unwrap();
            let slice = &jump_condition[storage_access.start()..storage_access.end()];
            frame.operation.solidify().contains(slice)
        })
    }) {
        debug_max!("jump matches loop-detection heuristic: 'jump_condition_contains_mutated_storage_access'");
        return true
    }

    false
}

/// check if all stack diffs for all historical stacks are exactly length 1, and the same
pub fn historical_diffs_approximately_equal(stack: &Stack, historical_stacks: &[Stack]) -> bool {
    // break if historical_stacks.len() < 4
    // this is an arbitrary number, i picked it randomly :D
    if historical_stacks.len() < 4 {
        return false
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
        return false
    }

    // check if all stack diffs are the same
    if !stack_diffs.iter().all(|diff| diff[0] == stack_diffs[0][0]) {
        return false
    }

    debug_max!("jump matches loop-detection heuristic: 'jump_condition_historical_diffs_approximately_equal'");

    true
}
