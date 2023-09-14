use crate::{
    ether::evm::core::stack::{Stack, StackFrame},
    io::logging::Logger,
};

/// Given two stacks A and B, return A - B, i.e. the items in A that are not in B.
/// This operation takes order into account, so if A = [1, 2, 3] and B = [1, 3, 2], then A - B =
/// [2]. This is referred to as the "stack diff"
pub fn stack_diff(a: &Stack, b: &Stack) -> Vec<StackFrame> {
    let mut diff = Vec::new();

    for (i, frame) in a.stack.iter().enumerate() {
        if b.stack.len() <= i {
            diff.push(frame.clone());
        } else if frame != &b.stack[i] {
            diff.push(frame.clone());
        }
    }

    diff
}

/// Check if the given stack contains too many of the same item.
/// If the stack contains more than 16 of the same item (with the same sources), it is considered a
/// loop.
pub fn stack_contains_too_many_of_the_same_item(stack: &Stack) -> bool {
    // get a new logger
    let level = std::env::var("RUST_LOG").unwrap_or_else(|_| "INFO".into());
    let (logger, _) = Logger::new(&level);

    if stack.size() > 16 && stack.stack.iter().any(|frame| {
        let solidified_frame_source = frame.operation.solidify();
        stack.stack.iter().filter(|f| f.operation.solidify() == solidified_frame_source).count() >=
            16
    }) {
        logger.debug_max(&format!(
            "jump matches loop-detection heuristic: 'stack_contains_too_many_of_the_same_item'"
        ));
        return true
    }

    false
}

/// Check if the stack contains any item with a source operation depth > 16. If so, it is considered
/// a loop. This check originates from the `stack too deep` error in Solidity due to the `DUP16` and
/// `SWAP16` operation limitations.
pub fn stack_item_source_depth_too_deep(stack: &Stack) -> bool {
    // get a new logger
    let level = std::env::var("RUST_LOG").unwrap_or_else(|_| "INFO".into());
    let (logger, _) = Logger::new(&level);

    if stack.stack.iter().any(|frame| frame.operation.depth() > 16) {
        logger.debug_max(&format!(
            "jump matches loop-detection heuristic: 'stack_item_source_depth_too_deep'"
        ));
        return true
    }

    false
}

/// Compare the similarity of the current stack and all previous stacks that have been encountered
/// for the given JUMPI. If the stacks are too similar, we consider this branch to be a loop.
pub fn stack_similarity_indicates_loop(stack: &Stack, historical_stacks: &Stack) -> bool {
    true
}
