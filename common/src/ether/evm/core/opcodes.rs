use ethers::types::U256;
use std::fmt::{Display, Formatter, Result};

/// An [`Opcode`] represents an Ethereum Virtual Machine (EVM) opcode. \
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Opcode {
    pub code: u8,
    pub name: &'static str,
    pub mingas: u16,
    pub inputs: u16,
    pub outputs: u16,
}

impl Opcode {
    /// Creates a new [`Opcode`] with the given code.
    ///
    /// ```
    /// use heimdall_common::ether::evm::core::opcodes::Opcode;
    ///
    /// let opcode = Opcode::new(0x01);
    /// assert_eq!(opcode.code, 0x01);
    /// assert_eq!(opcode.name, "ADD");
    /// ```
    pub fn new(code: u8) -> Opcode {
        match code {
            0x00 => Opcode { code, name: "STOP", mingas: 0, inputs: 0, outputs: 0 },
            0x01 => Opcode { code, name: "ADD", mingas: 3, inputs: 2, outputs: 1 },
            0x02 => Opcode { code, name: "MUL", mingas: 5, inputs: 2, outputs: 1 },
            0x03 => Opcode { code, name: "SUB", mingas: 3, inputs: 2, outputs: 1 },
            0x04 => Opcode { code, name: "DIV", mingas: 5, inputs: 2, outputs: 1 },
            0x05 => Opcode { code, name: "SDIV", mingas: 5, inputs: 2, outputs: 1 },
            0x06 => Opcode { code, name: "MOD", mingas: 5, inputs: 2, outputs: 1 },
            0x07 => Opcode { code, name: "SMOD", mingas: 5, inputs: 2, outputs: 1 },
            0x08 => Opcode { code, name: "ADDMOD", mingas: 8, inputs: 3, outputs: 1 },
            0x09 => Opcode { code, name: "MULMOD", mingas: 8, inputs: 3, outputs: 1 },
            0x0a => Opcode { code, name: "EXP", mingas: 10, inputs: 2, outputs: 1 },
            0x0b => Opcode { code, name: "SIGNEXTEND", mingas: 5, inputs: 2, outputs: 1 },
            0x10 => Opcode { code, name: "LT", mingas: 3, inputs: 2, outputs: 1 },
            0x11 => Opcode { code, name: "GT", mingas: 3, inputs: 2, outputs: 1 },
            0x12 => Opcode { code, name: "SLT", mingas: 3, inputs: 2, outputs: 1 },
            0x13 => Opcode { code, name: "SGT", mingas: 3, inputs: 2, outputs: 1 },
            0x14 => Opcode { code, name: "EQ", mingas: 3, inputs: 2, outputs: 1 },
            0x15 => Opcode { code, name: "ISZERO", mingas: 3, inputs: 1, outputs: 1 },
            0x16 => Opcode { code, name: "AND", mingas: 3, inputs: 2, outputs: 1 },
            0x17 => Opcode { code, name: "OR", mingas: 3, inputs: 2, outputs: 1 },
            0x18 => Opcode { code, name: "XOR", mingas: 3, inputs: 2, outputs: 1 },
            0x19 => Opcode { code, name: "NOT", mingas: 3, inputs: 1, outputs: 1 },
            0x1a => Opcode { code, name: "BYTE", mingas: 3, inputs: 2, outputs: 1 },
            0x1b => Opcode { code, name: "SHL", mingas: 3, inputs: 2, outputs: 1 },
            0x1c => Opcode { code, name: "SHR", mingas: 3, inputs: 2, outputs: 1 },
            0x1d => Opcode { code, name: "SAR", mingas: 3, inputs: 2, outputs: 1 },
            0x20 => Opcode { code, name: "SHA3", mingas: 30, inputs: 2, outputs: 1 },
            0x30 => Opcode { code, name: "ADDRESS", mingas: 2, inputs: 0, outputs: 1 },
            0x31 => Opcode { code, name: "BALANCE", mingas: 100, inputs: 1, outputs: 1 },
            0x32 => Opcode { code, name: "ORIGIN", mingas: 2, inputs: 0, outputs: 1 },
            0x33 => Opcode { code, name: "CALLER", mingas: 2, inputs: 0, outputs: 1 },
            0x34 => Opcode { code, name: "CALLVALUE", mingas: 2, inputs: 0, outputs: 1 },
            0x35 => Opcode { code, name: "CALLDATALOAD", mingas: 3, inputs: 1, outputs: 1 },
            0x36 => Opcode { code, name: "CALLDATASIZE", mingas: 2, inputs: 0, outputs: 1 },
            0x37 => Opcode { code, name: "CALLDATACOPY", mingas: 3, inputs: 3, outputs: 0 },
            0x38 => Opcode { code, name: "CODESIZE", mingas: 2, inputs: 0, outputs: 1 },
            0x39 => Opcode { code, name: "CODECOPY", mingas: 3, inputs: 3, outputs: 0 },
            0x3a => Opcode { code, name: "GASPRICE", mingas: 2, inputs: 0, outputs: 1 },
            0x3b => Opcode { code, name: "EXTCODESIZE", mingas: 100, inputs: 1, outputs: 1 },
            0x3c => Opcode { code, name: "EXTCODECOPY", mingas: 100, inputs: 4, outputs: 0 },
            0x3d => Opcode { code, name: "RETURNDATASIZE", mingas: 2, inputs: 0, outputs: 1 },
            0x3e => Opcode { code, name: "RETURNDATACOPY", mingas: 3, inputs: 3, outputs: 0 },
            0x3f => Opcode { code, name: "EXTCODEHASH", mingas: 100, inputs: 1, outputs: 1 },
            0x40 => Opcode { code, name: "BLOCKHASH", mingas: 20, inputs: 1, outputs: 1 },
            0x41 => Opcode { code, name: "COINBASE", mingas: 2, inputs: 0, outputs: 1 },
            0x42 => Opcode { code, name: "TIMESTAMP", mingas: 2, inputs: 0, outputs: 1 },
            0x43 => Opcode { code, name: "NUMBER", mingas: 2, inputs: 0, outputs: 1 },
            0x44 => Opcode { code, name: "DIFFICULTY", mingas: 2, inputs: 0, outputs: 1 },
            0x45 => Opcode { code, name: "GASLIMIT", mingas: 2, inputs: 0, outputs: 1 },
            0x46 => Opcode { code, name: "CHAINID", mingas: 2, inputs: 0, outputs: 1 },
            0x47 => Opcode { code, name: "SELFBALANCE", mingas: 5, inputs: 0, outputs: 1 },
            0x48 => Opcode { code, name: "BASEFEE", mingas: 2, inputs: 0, outputs: 1 },
            0x50 => Opcode { code, name: "POP", mingas: 2, inputs: 1, outputs: 0 },
            0x51 => Opcode { code, name: "MLOAD", mingas: 3, inputs: 1, outputs: 1 },
            0x52 => Opcode { code, name: "MSTORE", mingas: 3, inputs: 2, outputs: 0 },
            0x53 => Opcode { code, name: "MSTORE8", mingas: 3, inputs: 2, outputs: 0 },
            0x54 => Opcode { code, name: "SLOAD", mingas: 0, inputs: 1, outputs: 1 },
            0x55 => Opcode { code, name: "SSTORE", mingas: 0, inputs: 2, outputs: 0 },
            0x56 => Opcode { code, name: "JUMP", mingas: 8, inputs: 1, outputs: 0 },
            0x57 => Opcode { code, name: "JUMPI", mingas: 10, inputs: 2, outputs: 0 },
            0x58 => Opcode { code, name: "PC", mingas: 2, inputs: 0, outputs: 1 },
            0x59 => Opcode { code, name: "MSIZE", mingas: 2, inputs: 0, outputs: 1 },
            0x5a => Opcode { code, name: "GAS", mingas: 2, inputs: 0, outputs: 1 },
            0x5b => Opcode { code, name: "JUMPDEST", mingas: 1, inputs: 0, outputs: 0 },
            0x5f => Opcode { code, name: "PUSH0", mingas: 3, inputs: 0, outputs: 1 },
            0x60 => Opcode { code, name: "PUSH1", mingas: 3, inputs: 0, outputs: 1 },
            0x61 => Opcode { code, name: "PUSH2", mingas: 3, inputs: 0, outputs: 1 },
            0x62 => Opcode { code, name: "PUSH3", mingas: 3, inputs: 0, outputs: 1 },
            0x63 => Opcode { code, name: "PUSH4", mingas: 3, inputs: 0, outputs: 1 },
            0x64 => Opcode { code, name: "PUSH5", mingas: 3, inputs: 0, outputs: 1 },
            0x65 => Opcode { code, name: "PUSH6", mingas: 3, inputs: 0, outputs: 1 },
            0x66 => Opcode { code, name: "PUSH7", mingas: 3, inputs: 0, outputs: 1 },
            0x67 => Opcode { code, name: "PUSH8", mingas: 3, inputs: 0, outputs: 1 },
            0x68 => Opcode { code, name: "PUSH9", mingas: 3, inputs: 0, outputs: 1 },
            0x69 => Opcode { code, name: "PUSH10", mingas: 3, inputs: 0, outputs: 1 },
            0x6a => Opcode { code, name: "PUSH11", mingas: 3, inputs: 0, outputs: 1 },
            0x6b => Opcode { code, name: "PUSH12", mingas: 3, inputs: 0, outputs: 1 },
            0x6c => Opcode { code, name: "PUSH13", mingas: 3, inputs: 0, outputs: 1 },
            0x6d => Opcode { code, name: "PUSH14", mingas: 3, inputs: 0, outputs: 1 },
            0x6e => Opcode { code, name: "PUSH15", mingas: 3, inputs: 0, outputs: 1 },
            0x6f => Opcode { code, name: "PUSH16", mingas: 3, inputs: 0, outputs: 1 },
            0x70 => Opcode { code, name: "PUSH17", mingas: 3, inputs: 0, outputs: 1 },
            0x71 => Opcode { code, name: "PUSH18", mingas: 3, inputs: 0, outputs: 1 },
            0x72 => Opcode { code, name: "PUSH19", mingas: 3, inputs: 0, outputs: 1 },
            0x73 => Opcode { code, name: "PUSH20", mingas: 3, inputs: 0, outputs: 1 },
            0x74 => Opcode { code, name: "PUSH21", mingas: 3, inputs: 0, outputs: 1 },
            0x75 => Opcode { code, name: "PUSH22", mingas: 3, inputs: 0, outputs: 1 },
            0x76 => Opcode { code, name: "PUSH23", mingas: 3, inputs: 0, outputs: 1 },
            0x77 => Opcode { code, name: "PUSH24", mingas: 3, inputs: 0, outputs: 1 },
            0x78 => Opcode { code, name: "PUSH25", mingas: 3, inputs: 0, outputs: 1 },
            0x79 => Opcode { code, name: "PUSH26", mingas: 3, inputs: 0, outputs: 1 },
            0x7a => Opcode { code, name: "PUSH27", mingas: 3, inputs: 0, outputs: 1 },
            0x7b => Opcode { code, name: "PUSH28", mingas: 3, inputs: 0, outputs: 1 },
            0x7c => Opcode { code, name: "PUSH29", mingas: 3, inputs: 0, outputs: 1 },
            0x7d => Opcode { code, name: "PUSH30", mingas: 3, inputs: 0, outputs: 1 },
            0x7e => Opcode { code, name: "PUSH31", mingas: 3, inputs: 0, outputs: 1 },
            0x7f => Opcode { code, name: "PUSH32", mingas: 3, inputs: 0, outputs: 1 },
            0x80 => Opcode { code, name: "DUP1", mingas: 3, inputs: 1, outputs: 2 },
            0x81 => Opcode { code, name: "DUP2", mingas: 3, inputs: 2, outputs: 3 },
            0x82 => Opcode { code, name: "DUP3", mingas: 3, inputs: 3, outputs: 4 },
            0x83 => Opcode { code, name: "DUP4", mingas: 3, inputs: 4, outputs: 5 },
            0x84 => Opcode { code, name: "DUP5", mingas: 3, inputs: 5, outputs: 6 },
            0x85 => Opcode { code, name: "DUP6", mingas: 3, inputs: 6, outputs: 7 },
            0x86 => Opcode { code, name: "DUP7", mingas: 3, inputs: 7, outputs: 8 },
            0x87 => Opcode { code, name: "DUP8", mingas: 3, inputs: 8, outputs: 9 },
            0x88 => Opcode { code, name: "DUP9", mingas: 3, inputs: 9, outputs: 10 },
            0x89 => Opcode { code, name: "DUP10", mingas: 3, inputs: 10, outputs: 11 },
            0x8a => Opcode { code, name: "DUP11", mingas: 3, inputs: 11, outputs: 12 },
            0x8b => Opcode { code, name: "DUP12", mingas: 3, inputs: 12, outputs: 13 },
            0x8c => Opcode { code, name: "DUP13", mingas: 3, inputs: 13, outputs: 14 },
            0x8d => Opcode { code, name: "DUP14", mingas: 3, inputs: 14, outputs: 15 },
            0x8e => Opcode { code, name: "DUP15", mingas: 3, inputs: 15, outputs: 16 },
            0x8f => Opcode { code, name: "DUP16", mingas: 3, inputs: 16, outputs: 17 },
            0x90 => Opcode { code, name: "SWAP1", mingas: 3, inputs: 2, outputs: 2 },
            0x91 => Opcode { code, name: "SWAP2", mingas: 3, inputs: 3, outputs: 3 },
            0x92 => Opcode { code, name: "SWAP3", mingas: 3, inputs: 4, outputs: 4 },
            0x93 => Opcode { code, name: "SWAP4", mingas: 3, inputs: 5, outputs: 5 },
            0x94 => Opcode { code, name: "SWAP5", mingas: 3, inputs: 6, outputs: 6 },
            0x95 => Opcode { code, name: "SWAP6", mingas: 3, inputs: 7, outputs: 7 },
            0x96 => Opcode { code, name: "SWAP7", mingas: 3, inputs: 8, outputs: 8 },
            0x97 => Opcode { code, name: "SWAP8", mingas: 3, inputs: 9, outputs: 9 },
            0x98 => Opcode { code, name: "SWAP9", mingas: 3, inputs: 10, outputs: 10 },
            0x99 => Opcode { code, name: "SWAP10", mingas: 3, inputs: 11, outputs: 11 },
            0x9a => Opcode { code, name: "SWAP11", mingas: 3, inputs: 12, outputs: 12 },
            0x9b => Opcode { code, name: "SWAP12", mingas: 3, inputs: 13, outputs: 13 },
            0x9c => Opcode { code, name: "SWAP13", mingas: 3, inputs: 14, outputs: 14 },
            0x9d => Opcode { code, name: "SWAP14", mingas: 3, inputs: 15, outputs: 15 },
            0x9e => Opcode { code, name: "SWAP15", mingas: 3, inputs: 16, outputs: 16 },
            0x9f => Opcode { code, name: "SWAP16", mingas: 3, inputs: 17, outputs: 17 },
            0xa0 => Opcode { code, name: "LOG0", mingas: 375, inputs: 2, outputs: 0 },
            0xa1 => Opcode { code, name: "LOG1", mingas: 375, inputs: 3, outputs: 0 },
            0xa2 => Opcode { code, name: "LOG2", mingas: 375, inputs: 4, outputs: 0 },
            0xa3 => Opcode { code, name: "LOG3", mingas: 375, inputs: 5, outputs: 0 },
            0xa4 => Opcode { code, name: "LOG4", mingas: 375, inputs: 6, outputs: 0 },
            0xf0 => Opcode { code, name: "CREATE", mingas: 32000, inputs: 3, outputs: 1 },
            0xf1 => Opcode { code, name: "CALL", mingas: 100, inputs: 7, outputs: 1 },
            0xf2 => Opcode { code, name: "CALLCODE", mingas: 100, inputs: 7, outputs: 1 },
            0xf3 => Opcode { code, name: "RETURN", mingas: 0, inputs: 2, outputs: 0 },
            0xf4 => Opcode { code, name: "DELEGATECALL", mingas: 100, inputs: 6, outputs: 1 },
            0xf5 => Opcode { code, name: "CREATE2", mingas: 32000, inputs: 4, outputs: 1 },
            0xfa => Opcode { code, name: "STATICCALL", mingas: 100, inputs: 6, outputs: 1 },
            0xfd => Opcode { code, name: "REVERT", mingas: 0, inputs: 2, outputs: 0 },
            0xfe => Opcode { code, name: "INVALID", mingas: 0, inputs: 0, outputs: 0 },
            0xff => Opcode { code, name: "SELFDESTRUCT", mingas: 5000, inputs: 1, outputs: 0 },
            _ => Opcode { code, name: "unknown", mingas: 0, inputs: 0, outputs: 0 },
        }
    }
}

/// A WrappedInput can contain either a raw U256 value or a WrappedOpcode
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum WrappedInput {
    Raw(U256),
    Opcode(WrappedOpcode),
}

/// A WrappedOpcode is an Opcode with its inputs wrapped in a WrappedInput
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct WrappedOpcode {
    pub opcode: Opcode,
    pub inputs: Vec<WrappedInput>,
}

impl WrappedOpcode {
    /// Returns the depth of the opcode, i.e. the maximum recursion depth of its inputs
    ///
    /// ```
    /// use heimdall_common::ether::evm::core::opcodes::*;
    ///
    /// let opcode = WrappedOpcode::new(0x01, vec![WrappedInput::Raw(1.into()), WrappedInput::Raw(2.into())]);
    /// assert_eq!(opcode.depth(), 1);
    /// ```
    pub fn depth(&self) -> u32 {
        self.inputs.iter().map(|x| x.depth()).max().unwrap_or(0) + 1
    }
}

impl WrappedInput {
    /// Returns the depth of the input, i.e. 0 for a raw U256 and the depth of the opcode for a
    /// WrappedOpcode
    ///
    /// ```
    /// use heimdall_common::ether::evm::core::opcodes::*;
    ///
    /// let opcode = WrappedOpcode::new(0x01, vec![WrappedInput::Raw(1.into()), WrappedInput::Raw(2.into())]);
    /// assert_eq!(opcode.depth(), 1);
    ///
    /// let input = WrappedInput::Opcode(opcode);
    /// assert_eq!(input.depth(), 1);
    /// ```
    pub fn depth(&self) -> u32 {
        match self {
            WrappedInput::Raw(_) => 0,
            WrappedInput::Opcode(opcode) => opcode.depth(),
        }
    }
}

impl Display for WrappedOpcode {
    fn fmt(&self, f: &mut Formatter) -> Result {
        write!(
            f,
            "{}({})",
            self.opcode.name,
            self.inputs.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(", ")
        )
    }
}

impl Display for WrappedInput {
    fn fmt(&self, f: &mut Formatter) -> Result {
        match self {
            WrappedInput::Raw(u256) => write!(f, "{u256}"),
            WrappedInput::Opcode(opcode) => write!(f, "{opcode}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use ethers::types::U256;

    use crate::ether::evm::core::opcodes::*;

    #[test]
    fn test_opcode() {
        let add_operation = Opcode::new(0x01);
        assert_eq!(add_operation.code, 0x01);
        assert_eq!(add_operation.name, "ADD");
    }

    #[test]
    fn test_get_unknown_opcode() {
        let unknown_opcode = Opcode::new(0xee);
        assert_eq!(unknown_opcode.name, "unknown");
    }

    #[test]
    fn test_wrapping_opcodes() {
        // wraps an ADD operation with 2 raw inputs
        let add_operation_wrapped = WrappedOpcode::new(
            0x01,
            vec![WrappedInput::Raw(U256::from(1u8)), WrappedInput::Raw(U256::from(2u8))],
        );
        println!("{}", add_operation_wrapped);

        // wraps a CALLDATALOAD operation
        let calldataload_wrapped =
            WrappedOpcode::new(0x35, vec![WrappedInput::Opcode(add_operation_wrapped)]);
        println!("{}", calldataload_wrapped);
    }
}
