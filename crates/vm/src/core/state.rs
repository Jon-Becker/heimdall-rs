//! Abstract execution state for basic-block analysis.

use super::stack::AbstractStack;

/// Abstract state recorded at a basic-block entry.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AbstractState {
    /// Abstract operand stack.
    pub stack: AbstractStack,
}

impl AbstractState {
    /// Construct the empty initial EVM state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Construct a state with an explicit abstract stack.
    pub fn with_stack(stack: AbstractStack) -> Self {
        Self { stack }
    }

    pub(super) fn join(&self, other: &Self, max_values: usize) -> Self {
        Self { stack: self.stack.join(&other.stack, max_values) }
    }
}
