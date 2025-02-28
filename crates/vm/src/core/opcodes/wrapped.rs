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
    ///
    /// ```
    /// use heimdall_vm::core::opcodes::wrapped::{WrappedOpcode, WrappedInput};
    /// use alloy::primitives::U256;
    /// use heimdall_vm::ext::lexers::solidity::WrappedOpcode;
    ///
    /// // Create a PUSH1 0x01 operation
    /// let push1 = WrappedOpcode::new(0x60, vec![WrappedInput::Raw(U256::from(1))]);
    /// assert_eq!(push1.depth(), 1);  // Depth is 1 because it has only raw inputs
    ///
    /// // Create an ADD operation that takes the result of two PUSH operations
    /// let add = WrappedOpcode::new(0x01, vec![
    ///     WrappedInput::Opcode(push1.clone()),
    ///     WrappedInput::Opcode(push1.clone())
    /// ]);
    /// assert_eq!(add.depth(), 2);  // Depth is 2 because it contains operations with depth 1
    /// ```
    pub fn depth(&self) -> u32 {
        self.inputs.iter().map(|x| x.depth()).max().unwrap_or(0) + 1
    }
}

impl std::fmt::Display for WrappedOpcode {
    /// Formats the [`WrappedOpcode`] as a string.
    ///
    /// The format is: `OPCODENAME(input1, input2, ...)` where each input is
    /// formatted according to its own [`Display`] implementation.
    ///
    /// ```
    /// use heimdall_vm::core::opcodes::wrapped::{WrappedOpcode, WrappedInput};
    /// use heimdall_vm::ext::lexers::solidity::WrappedOpcode;
    /// use alloy::primitives::U256;
    ///
    /// let push1 = WrappedOpcode::new(0x60, vec![WrappedInput::Raw(U256::from(1))]);
    /// assert_eq!(push1.to_string(), "PUSH1(1)");
    ///
    /// let add = WrappedOpcode::new(0x01, vec![
    ///     WrappedInput::Opcode(push1.clone()),
    ///     WrappedInput::Opcode(push1.clone())
    /// ]);
    /// assert_eq!(add.to_string(), "ADD(PUSH1(1), PUSH1(1))");
    /// ```
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
    ///
    /// ```
    /// use heimdall_vm::core::opcodes::wrapped::WrappedInput;
    /// use heimdall_vm::ext::lexers::solidity::WrappedOpcode;
    /// use alloy::primitives::U256;
    ///
    /// // Raw inputs have depth 0
    /// let raw = WrappedInput::Raw(U256::from(42));
    /// assert_eq!(raw.depth(), 0);
    ///
    /// // Opcode inputs have the depth of the operation they contain
    /// let push1 = WrappedOpcode::new(0x60, vec![WrappedInput::Raw(U256::from(1))]);
    /// let op_input = WrappedInput::Opcode(push1);
    /// assert_eq!(op_input.depth(), 1);
    /// ```
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
    ///
    /// ```
    /// use heimdall_vm::core::opcodes::wrapped::WrappedInput;
    /// use heimdall_vm::ext::lexers::solidity::WrappedOpcode;
    /// use alloy::primitives::U256;
    ///
    /// let raw = WrappedInput::Raw(U256::from(42));
    /// assert_eq!(raw.to_string(), "42");
    ///
    /// let push1 = WrappedOpcode::new(0x60, vec![WrappedInput::Raw(U256::from(1))]);
    /// let op_input = WrappedInput::Opcode(push1);
    /// assert_eq!(op_input.to_string(), "PUSH1(1)");
    /// ```
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
    ///
    /// ```
    /// use heimdall_vm::core::opcodes::wrapped::WrappedInput;
    /// use alloy::primitives::U256;
    ///
    /// let u256_val = U256::from(42);
    /// let input: WrappedInput = u256_val.into();
    ///
    /// match input {
    ///     WrappedInput::Raw(val) => assert_eq!(val, U256::from(42)),
    ///     _ => panic!("Expected Raw variant"),
    /// }
    /// ```
    fn from(val: U256) -> Self {
        WrappedInput::Raw(val)
    }
}

impl From<WrappedOpcode> for WrappedInput {
    /// Converts a [`WrappedOpcode`] into a [`WrappedInput::Opcode`].
    ///
    /// This implementation allows for more ergonomic code when creating
    /// [`WrappedInput`]s from operations.
    ///
    /// ```
    /// use heimdall_vm::core::opcodes::wrapped::WrappedInput;
    /// use heimdall_vm::ext::lexers::solidity::WrappedOpcode;
    /// use alloy::primitives::U256;
    ///
    /// let push1 = WrappedOpcode::new(0x60, vec![WrappedInput::Raw(U256::from(1))]);
    /// let input: WrappedInput = push1.clone().into();
    ///
    /// match input {
    ///     WrappedInput::Opcode(op) => assert_eq!(op, push1),
    ///     _ => panic!("Expected Opcode variant"),
    /// }
    /// ```
    fn from(val: WrappedOpcode) -> Self {
        WrappedInput::Opcode(val)
    }
}
