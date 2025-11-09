use std::sync::Arc;

use alloy::primitives::U256;
use once_cell::sync::OnceCell;

use crate::core::opcodes::opcode_name;

/// A [`WrappedInput`] can contain either a raw [`U256`] value or a [`WrappedOpcode`].
///
/// This enum is used to represent inputs to EVM opcodes, allowing inputs to be
/// either constant values or the results of previous operations in the execution flow.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum WrappedInput {
    /// A raw value input (typically from a PUSH instruction)
    Raw(U256),
    /// An opcode result as input (indicating data dependency)
    ///
    /// Uses Arc for cheap cloning and shared ownership, preventing exponential
    /// memory growth in deeply nested operation trees.
    Opcode(Arc<WrappedOpcode>),
}

/// A [`WrappedOpcode`] is an EVM opcode with its inputs wrapped in a [`WrappedInput`].
///
/// This structure is used to represent opcodes and their arguments in a way
/// that can capture the relationships between operations, allowing for analysis
/// of execution flow and dependencies.
///
/// The structure uses Arc-wrapped inputs to enable O(1) cloning and shared memory,
/// preventing exponential slowdown in deeply nested expression trees common in
/// cryptographic operations like MULMOD.
#[derive(Debug)]
pub struct WrappedOpcode {
    /// The opcode value as a byte.
    ///
    /// This corresponds to the actual EVM opcode (e.g., 0x01 for ADD).
    pub opcode: u8,

    /// The inputs for this opcode, wrapped to preserve their source context.
    ///
    /// For example, an ADD opcode would typically have two inputs, which could be
    /// either raw values or the results of other operations.
    pub inputs: Vec<WrappedInput>,

    /// Cached depth value, computed once on first access.
    ///
    /// This prevents repeated O(n) tree traversals, making depth() O(1) after
    /// first access instead of O(2^depth).
    cached_depth: OnceCell<u32>,
}

impl WrappedOpcode {
    /// Creates a new WrappedOpcode with the given opcode and inputs.
    pub fn new(opcode: u8, inputs: Vec<WrappedInput>) -> Self {
        Self { opcode, inputs, cached_depth: OnceCell::new() }
    }

    /// Returns the maximum recursion depth of its inputs.
    ///
    /// The depth is calculated as the maximum depth of any input plus 1.
    /// A depth of 1 means the opcode has only raw inputs (or no inputs).
    /// Greater depths indicate a chain of operations.
    ///
    /// This method is memoized - the depth is computed once and cached for O(1)
    /// subsequent accesses.
    pub fn depth(&self) -> u32 {
        *self
            .cached_depth
            .get_or_init(|| self.inputs.iter().map(|x| x.depth()).max().unwrap_or(0) + 1)
    }
}

impl Clone for WrappedOpcode {
    fn clone(&self) -> Self {
        Self {
            opcode: self.opcode,
            inputs: self.inputs.clone(),
            // Don't clone the cached depth - let it be recomputed if needed
            cached_depth: OnceCell::new(),
        }
    }
}

impl PartialEq for WrappedOpcode {
    fn eq(&self, other: &Self) -> bool {
        self.opcode == other.opcode && self.inputs == other.inputs
    }
}

impl Eq for WrappedOpcode {}

impl std::hash::Hash for WrappedOpcode {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.opcode.hash(state);
        self.inputs.hash(state);
    }
}

impl Default for WrappedOpcode {
    fn default() -> Self {
        Self { opcode: 0, inputs: Vec::new(), cached_depth: OnceCell::new() }
    }
}

impl std::fmt::Display for WrappedOpcode {
    /// Formats the [`WrappedOpcode`] as a string.
    ///
    /// The format is: `OPCODENAME(input1, input2, ...)` where each input is
    /// formatted according to its own [`Display`] implementation.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}({})",
            opcode_name(self.opcode),
            self.inputs.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(", ")
        )
    }
}

impl WrappedInput {
    /// Returns the depth of the input.
    ///
    /// - 0 for a raw [`U256`] value
    /// - The depth of the contained [`WrappedOpcode`] for an opcode input
    ///
    /// This method is used to calculate the recursive depth of operations
    /// for analysis and optimization purposes.
    pub fn depth(&self) -> u32 {
        match self {
            WrappedInput::Raw(_) => 0,
            WrappedInput::Opcode(opcode) => opcode.depth(),
        }
    }
}

impl std::fmt::Display for WrappedInput {
    /// Formats the [`WrappedInput`] as a string.
    ///
    /// - For [`Raw`] inputs, displays the contained [`U256`] value.
    /// - For [`Opcode`] inputs, recursively formats the contained [`WrappedOpcode`].
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WrappedInput::Raw(u256) => write!(f, "{u256}"),
            WrappedInput::Opcode(opcode) => write!(f, "{opcode}"),
        }
    }
}

impl From<U256> for WrappedInput {
    /// Converts a [`U256`] value into a [`WrappedInput::Raw`].
    ///
    /// This implementation allows for more ergonomic code when creating
    /// [`WrappedInput`]s from raw values.
    fn from(val: U256) -> Self {
        WrappedInput::Raw(val)
    }
}

impl From<WrappedOpcode> for WrappedInput {
    /// Converts a [`WrappedOpcode`] into a [`WrappedInput::Opcode`].
    ///
    /// This implementation allows for more ergonomic code when creating
    /// [`WrappedInput`]s from operations. The opcode is automatically wrapped
    /// in an Arc for efficient sharing.
    fn from(val: WrappedOpcode) -> Self {
        WrappedInput::Opcode(Arc::new(val))
    }
}
