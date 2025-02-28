use alloy::primitives::U256;

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
    Opcode(WrappedOpcode),
}

/// A [`WrappedOpcode`] is an EVM opcode with its inputs wrapped in a [`WrappedInput`].
///
/// This structure is used to represent opcodes and their arguments in a way
/// that can capture the relationships between operations, allowing for analysis
/// of execution flow and dependencies.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Default)]
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
}

impl WrappedOpcode {
    /// Returns the maximum recursion depth of its inputs.
    ///
    /// The depth is calculated as the maximum depth of any input plus 1.
    /// A depth of 1 means the opcode has only raw inputs (or no inputs).
    /// Greater depths indicate a chain of operations.
    pub fn depth(&self) -> u32 {
        self.inputs.iter().map(|x| x.depth()).max().unwrap_or(0) + 1
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
    /// [`WrappedInput`]s from operations.
    fn from(val: WrappedOpcode) -> Self {
        WrappedInput::Opcode(val)
    }
}
