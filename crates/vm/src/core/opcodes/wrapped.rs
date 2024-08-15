use alloy::primitives::U256;

use crate::core::opcodes::OpCodeInfo;

/// A WrappedInput can contain either a raw U256 value or a WrappedOpcode
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum WrappedInput {
    /// A raw value input
    Raw(U256),
    /// An opcode input
    Opcode(WrappedOpcode),
}

/// A WrappedOpcode is an Opcode with its inputs wrapped in a WrappedInput
#[derive(Clone, Debug, PartialEq, Eq, Hash, Default)]
pub struct WrappedOpcode {
    pub opcode: u8,
    pub inputs: Vec<WrappedInput>,
}

impl WrappedOpcode {
    /// Returns the maximum recursion depth of its inputs
    pub fn depth(&self) -> u32 {
        self.inputs.iter().map(|x| x.depth()).max().unwrap_or(0) + 1
    }
}

impl std::fmt::Display for WrappedOpcode {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "{}({})",
            OpCodeInfo::from(self.opcode).name(),
            self.inputs.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(", ")
        )
    }
}

impl WrappedInput {
    /// Returns the depth of the input \
    ///
    /// i.e. 0 for a raw U256 and the maximum recursion depth for a WrappedOpcode
    pub fn depth(&self) -> u32 {
        match self {
            WrappedInput::Raw(_) => 0,
            WrappedInput::Opcode(opcode) => opcode.depth(),
        }
    }
}

impl std::fmt::Display for WrappedInput {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            WrappedInput::Raw(u256) => write!(f, "{u256}"),
            WrappedInput::Opcode(opcode) => write!(f, "{opcode}"),
        }
    }
}

impl From<U256> for WrappedInput {
    fn from(val: U256) -> Self {
        WrappedInput::Raw(val)
    }
}

impl From<WrappedOpcode> for WrappedInput {
    fn from(val: WrappedOpcode) -> Self {
        WrappedInput::Opcode(val)
    }
}
