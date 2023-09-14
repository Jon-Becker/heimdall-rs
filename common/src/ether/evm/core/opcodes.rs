use ethers::types::U256;
use std::fmt::{Display, Formatter, Result};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Opcode {
    pub code: u8,
    pub name: &'static str,
    pub mingas: u16,
    pub inputs: u16,
    pub outputs: u16,
}

// Returns the opcode for the given hexcode, fetched from the hashmap.
pub fn opcode(code: u8) -> Opcode {
    match code {
        0x00 => Opcode { code: code, name: "STOP", mingas: 0, inputs: 0, outputs: 0 },
        0x01 => Opcode { code: code, name: "ADD", mingas: 3, inputs: 2, outputs: 1 },
        0x02 => Opcode { code: code, name: "MUL", mingas: 5, inputs: 2, outputs: 1 },
        0x03 => Opcode { code: code, name: "SUB", mingas: 3, inputs: 2, outputs: 1 },
        0x04 => Opcode { code: code, name: "DIV", mingas: 5, inputs: 2, outputs: 1 },
        0x05 => Opcode { code: code, name: "SDIV", mingas: 5, inputs: 2, outputs: 1 },
        0x06 => Opcode { code: code, name: "MOD", mingas: 5, inputs: 2, outputs: 1 },
        0x07 => Opcode { code: code, name: "SMOD", mingas: 5, inputs: 2, outputs: 1 },
        0x08 => Opcode { code: code, name: "ADDMOD", mingas: 8, inputs: 3, outputs: 1 },
        0x09 => Opcode { code: code, name: "MULMOD", mingas: 8, inputs: 3, outputs: 1 },
        0x0a => Opcode { code: code, name: "EXP", mingas: 10, inputs: 2, outputs: 1 },
        0x0b => Opcode { code: code, name: "SIGNEXTEND", mingas: 5, inputs: 2, outputs: 1 },
        0x10 => Opcode { code: code, name: "LT", mingas: 3, inputs: 2, outputs: 1 },
        0x11 => Opcode { code: code, name: "GT", mingas: 3, inputs: 2, outputs: 1 },
        0x12 => Opcode { code: code, name: "SLT", mingas: 3, inputs: 2, outputs: 1 },
        0x13 => Opcode { code: code, name: "SGT", mingas: 3, inputs: 2, outputs: 1 },
        0x14 => Opcode { code: code, name: "EQ", mingas: 3, inputs: 2, outputs: 1 },
        0x15 => Opcode { code: code, name: "ISZERO", mingas: 3, inputs: 1, outputs: 1 },
        0x16 => Opcode { code: code, name: "AND", mingas: 3, inputs: 2, outputs: 1 },
        0x17 => Opcode { code: code, name: "OR", mingas: 3, inputs: 2, outputs: 1 },
        0x18 => Opcode { code: code, name: "XOR", mingas: 3, inputs: 2, outputs: 1 },
        0x19 => Opcode { code: code, name: "NOT", mingas: 3, inputs: 1, outputs: 1 },
        0x1a => Opcode { code: code, name: "BYTE", mingas: 3, inputs: 2, outputs: 1 },
        0x1b => Opcode { code: code, name: "SHL", mingas: 3, inputs: 2, outputs: 1 },
        0x1c => Opcode { code: code, name: "SHR", mingas: 3, inputs: 2, outputs: 1 },
        0x1d => Opcode { code: code, name: "SAR", mingas: 3, inputs: 2, outputs: 1 },
        0x20 => Opcode { code: code, name: "SHA3", mingas: 30, inputs: 2, outputs: 1 },
        0x30 => Opcode { code: code, name: "ADDRESS", mingas: 2, inputs: 0, outputs: 1 },
        0x31 => Opcode { code: code, name: "BALANCE", mingas: 100, inputs: 1, outputs: 1 },
        0x32 => Opcode { code: code, name: "ORIGIN", mingas: 2, inputs: 0, outputs: 1 },
        0x33 => Opcode { code: code, name: "CALLER", mingas: 2, inputs: 0, outputs: 1 },
        0x34 => Opcode { code: code, name: "CALLVALUE", mingas: 2, inputs: 0, outputs: 1 },
        0x35 => Opcode { code: code, name: "CALLDATALOAD", mingas: 3, inputs: 1, outputs: 1 },
        0x36 => Opcode { code: code, name: "CALLDATASIZE", mingas: 2, inputs: 0, outputs: 1 },
        0x37 => Opcode { code: code, name: "CALLDATACOPY", mingas: 3, inputs: 3, outputs: 0 },
        0x38 => Opcode { code: code, name: "CODESIZE", mingas: 2, inputs: 0, outputs: 1 },
        0x39 => Opcode { code: code, name: "CODECOPY", mingas: 3, inputs: 3, outputs: 0 },
        0x3a => Opcode { code: code, name: "GASPRICE", mingas: 2, inputs: 0, outputs: 1 },
        0x3b => Opcode { code: code, name: "EXTCODESIZE", mingas: 100, inputs: 1, outputs: 1 },
        0x3c => Opcode { code: code, name: "EXTCODECOPY", mingas: 100, inputs: 4, outputs: 0 },
        0x3d => Opcode { code: code, name: "RETURNDATASIZE", mingas: 2, inputs: 0, outputs: 1 },
        0x3e => Opcode { code: code, name: "RETURNDATACOPY", mingas: 3, inputs: 3, outputs: 0 },
        0x3f => Opcode { code: code, name: "EXTCODEHASH", mingas: 100, inputs: 1, outputs: 1 },
        0x40 => Opcode { code: code, name: "BLOCKHASH", mingas: 20, inputs: 1, outputs: 1 },
        0x41 => Opcode { code: code, name: "COINBASE", mingas: 2, inputs: 0, outputs: 1 },
        0x42 => Opcode { code: code, name: "TIMESTAMP", mingas: 2, inputs: 0, outputs: 1 },
        0x43 => Opcode { code: code, name: "NUMBER", mingas: 2, inputs: 0, outputs: 1 },
        0x44 => Opcode { code: code, name: "DIFFICULTY", mingas: 2, inputs: 0, outputs: 1 },
        0x45 => Opcode { code: code, name: "GASLIMIT", mingas: 2, inputs: 0, outputs: 1 },
        0x46 => Opcode { code: code, name: "CHAINID", mingas: 2, inputs: 0, outputs: 1 },
        0x47 => Opcode { code: code, name: "SELFBALANCE", mingas: 5, inputs: 0, outputs: 1 },
        0x48 => Opcode { code: code, name: "BASEFEE", mingas: 2, inputs: 0, outputs: 1 },
        0x50 => Opcode { code: code, name: "POP", mingas: 2, inputs: 1, outputs: 0 },
        0x51 => Opcode { code: code, name: "MLOAD", mingas: 3, inputs: 1, outputs: 1 },
        0x52 => Opcode { code: code, name: "MSTORE", mingas: 3, inputs: 2, outputs: 0 },
        0x53 => Opcode { code: code, name: "MSTORE8", mingas: 3, inputs: 2, outputs: 0 },
        0x54 => Opcode { code: code, name: "SLOAD", mingas: 0, inputs: 1, outputs: 1 },
        0x55 => Opcode { code: code, name: "SSTORE", mingas: 0, inputs: 2, outputs: 0 },
        0x56 => Opcode { code: code, name: "JUMP", mingas: 8, inputs: 1, outputs: 0 },
        0x57 => Opcode { code: code, name: "JUMPI", mingas: 10, inputs: 2, outputs: 0 },
        0x58 => Opcode { code: code, name: "PC", mingas: 2, inputs: 0, outputs: 1 },
        0x59 => Opcode { code: code, name: "MSIZE", mingas: 2, inputs: 0, outputs: 1 },
        0x5a => Opcode { code: code, name: "GAS", mingas: 2, inputs: 0, outputs: 1 },
        0x5b => Opcode { code: code, name: "JUMPDEST", mingas: 1, inputs: 0, outputs: 0 },
        0x5f => Opcode { code: code, name: "PUSH0", mingas: 3, inputs: 0, outputs: 1 },
        0x60 => Opcode { code: code, name: "PUSH1", mingas: 3, inputs: 0, outputs: 1 },
        0x61 => Opcode { code: code, name: "PUSH2", mingas: 3, inputs: 0, outputs: 1 },
        0x62 => Opcode { code: code, name: "PUSH3", mingas: 3, inputs: 0, outputs: 1 },
        0x63 => Opcode { code: code, name: "PUSH4", mingas: 3, inputs: 0, outputs: 1 },
        0x64 => Opcode { code: code, name: "PUSH5", mingas: 3, inputs: 0, outputs: 1 },
        0x65 => Opcode { code: code, name: "PUSH6", mingas: 3, inputs: 0, outputs: 1 },
        0x66 => Opcode { code: code, name: "PUSH7", mingas: 3, inputs: 0, outputs: 1 },
        0x67 => Opcode { code: code, name: "PUSH8", mingas: 3, inputs: 0, outputs: 1 },
        0x68 => Opcode { code: code, name: "PUSH9", mingas: 3, inputs: 0, outputs: 1 },
        0x69 => Opcode { code: code, name: "PUSH10", mingas: 3, inputs: 0, outputs: 1 },
        0x6a => Opcode { code: code, name: "PUSH11", mingas: 3, inputs: 0, outputs: 1 },
        0x6b => Opcode { code: code, name: "PUSH12", mingas: 3, inputs: 0, outputs: 1 },
        0x6c => Opcode { code: code, name: "PUSH13", mingas: 3, inputs: 0, outputs: 1 },
        0x6d => Opcode { code: code, name: "PUSH14", mingas: 3, inputs: 0, outputs: 1 },
        0x6e => Opcode { code: code, name: "PUSH15", mingas: 3, inputs: 0, outputs: 1 },
        0x6f => Opcode { code: code, name: "PUSH16", mingas: 3, inputs: 0, outputs: 1 },
        0x70 => Opcode { code: code, name: "PUSH17", mingas: 3, inputs: 0, outputs: 1 },
        0x71 => Opcode { code: code, name: "PUSH18", mingas: 3, inputs: 0, outputs: 1 },
        0x72 => Opcode { code: code, name: "PUSH19", mingas: 3, inputs: 0, outputs: 1 },
        0x73 => Opcode { code: code, name: "PUSH20", mingas: 3, inputs: 0, outputs: 1 },
        0x74 => Opcode { code: code, name: "PUSH21", mingas: 3, inputs: 0, outputs: 1 },
        0x75 => Opcode { code: code, name: "PUSH22", mingas: 3, inputs: 0, outputs: 1 },
        0x76 => Opcode { code: code, name: "PUSH23", mingas: 3, inputs: 0, outputs: 1 },
        0x77 => Opcode { code: code, name: "PUSH24", mingas: 3, inputs: 0, outputs: 1 },
        0x78 => Opcode { code: code, name: "PUSH25", mingas: 3, inputs: 0, outputs: 1 },
        0x79 => Opcode { code: code, name: "PUSH26", mingas: 3, inputs: 0, outputs: 1 },
        0x7a => Opcode { code: code, name: "PUSH27", mingas: 3, inputs: 0, outputs: 1 },
        0x7b => Opcode { code: code, name: "PUSH28", mingas: 3, inputs: 0, outputs: 1 },
        0x7c => Opcode { code: code, name: "PUSH29", mingas: 3, inputs: 0, outputs: 1 },
        0x7d => Opcode { code: code, name: "PUSH30", mingas: 3, inputs: 0, outputs: 1 },
        0x7e => Opcode { code: code, name: "PUSH31", mingas: 3, inputs: 0, outputs: 1 },
        0x7f => Opcode { code: code, name: "PUSH32", mingas: 3, inputs: 0, outputs: 1 },
        0x80 => Opcode { code: code, name: "DUP1", mingas: 3, inputs: 1, outputs: 2 },
        0x81 => Opcode { code: code, name: "DUP2", mingas: 3, inputs: 2, outputs: 3 },
        0x82 => Opcode { code: code, name: "DUP3", mingas: 3, inputs: 3, outputs: 4 },
        0x83 => Opcode { code: code, name: "DUP4", mingas: 3, inputs: 4, outputs: 5 },
        0x84 => Opcode { code: code, name: "DUP5", mingas: 3, inputs: 5, outputs: 6 },
        0x85 => Opcode { code: code, name: "DUP6", mingas: 3, inputs: 6, outputs: 7 },
        0x86 => Opcode { code: code, name: "DUP7", mingas: 3, inputs: 7, outputs: 8 },
        0x87 => Opcode { code: code, name: "DUP8", mingas: 3, inputs: 8, outputs: 9 },
        0x88 => Opcode { code: code, name: "DUP9", mingas: 3, inputs: 9, outputs: 10 },
        0x89 => Opcode { code: code, name: "DUP10", mingas: 3, inputs: 10, outputs: 11 },
        0x8a => Opcode { code: code, name: "DUP11", mingas: 3, inputs: 11, outputs: 12 },
        0x8b => Opcode { code: code, name: "DUP12", mingas: 3, inputs: 12, outputs: 13 },
        0x8c => Opcode { code: code, name: "DUP13", mingas: 3, inputs: 13, outputs: 14 },
        0x8d => Opcode { code: code, name: "DUP14", mingas: 3, inputs: 14, outputs: 15 },
        0x8e => Opcode { code: code, name: "DUP15", mingas: 3, inputs: 15, outputs: 16 },
        0x8f => Opcode { code: code, name: "DUP16", mingas: 3, inputs: 16, outputs: 17 },
        0x90 => Opcode { code: code, name: "SWAP1", mingas: 3, inputs: 2, outputs: 2 },
        0x91 => Opcode { code: code, name: "SWAP2", mingas: 3, inputs: 3, outputs: 3 },
        0x92 => Opcode { code: code, name: "SWAP3", mingas: 3, inputs: 4, outputs: 4 },
        0x93 => Opcode { code: code, name: "SWAP4", mingas: 3, inputs: 5, outputs: 5 },
        0x94 => Opcode { code: code, name: "SWAP5", mingas: 3, inputs: 6, outputs: 6 },
        0x95 => Opcode { code: code, name: "SWAP6", mingas: 3, inputs: 7, outputs: 7 },
        0x96 => Opcode { code: code, name: "SWAP7", mingas: 3, inputs: 8, outputs: 8 },
        0x97 => Opcode { code: code, name: "SWAP8", mingas: 3, inputs: 9, outputs: 9 },
        0x98 => Opcode { code: code, name: "SWAP9", mingas: 3, inputs: 10, outputs: 10 },
        0x99 => Opcode { code: code, name: "SWAP10", mingas: 3, inputs: 11, outputs: 11 },
        0x9a => Opcode { code: code, name: "SWAP11", mingas: 3, inputs: 12, outputs: 12 },
        0x9b => Opcode { code: code, name: "SWAP12", mingas: 3, inputs: 13, outputs: 13 },
        0x9c => Opcode { code: code, name: "SWAP13", mingas: 3, inputs: 14, outputs: 14 },
        0x9d => Opcode { code: code, name: "SWAP14", mingas: 3, inputs: 15, outputs: 15 },
        0x9e => Opcode { code: code, name: "SWAP15", mingas: 3, inputs: 16, outputs: 16 },
        0x9f => Opcode { code: code, name: "SWAP16", mingas: 3, inputs: 17, outputs: 17 },
        0xa0 => Opcode { code: code, name: "LOG0", mingas: 375, inputs: 2, outputs: 0 },
        0xa1 => Opcode { code: code, name: "LOG1", mingas: 375, inputs: 3, outputs: 0 },
        0xa2 => Opcode { code: code, name: "LOG2", mingas: 375, inputs: 4, outputs: 0 },
        0xa3 => Opcode { code: code, name: "LOG3", mingas: 375, inputs: 5, outputs: 0 },
        0xa4 => Opcode { code: code, name: "LOG4", mingas: 375, inputs: 6, outputs: 0 },
        0xf0 => Opcode { code: code, name: "CREATE", mingas: 32000, inputs: 3, outputs: 1 },
        0xf1 => Opcode { code: code, name: "CALL", mingas: 100, inputs: 7, outputs: 1 },
        0xf2 => Opcode { code: code, name: "CALLCODE", mingas: 100, inputs: 7, outputs: 1 },
        0xf3 => Opcode { code: code, name: "RETURN", mingas: 0, inputs: 2, outputs: 0 },
        0xf4 => Opcode { code: code, name: "DELEGATECALL", mingas: 100, inputs: 6, outputs: 1 },
        0xf5 => Opcode { code: code, name: "CREATE2", mingas: 32000, inputs: 4, outputs: 1 },
        0xfa => Opcode { code: code, name: "STATICCALL", mingas: 100, inputs: 6, outputs: 1 },
        0xfd => Opcode { code: code, name: "REVERT", mingas: 0, inputs: 2, outputs: 0 },
        0xfe => Opcode { code: code, name: "INVALID", mingas: 0, inputs: 0, outputs: 0 },
        0xff => Opcode { code: code, name: "SELFDESTRUCT", mingas: 5000, inputs: 1, outputs: 0 },
        _ => Opcode { code: code, name: "unknown", mingas: 0, inputs: 0, outputs: 0 },
    }
}

// enum allows for Wrapped Opcodes to contain both raw U256 and Opcodes as inputs
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum WrappedInput {
    Raw(U256),
    Opcode(WrappedOpcode),
}

// represents an opcode with its direct inputs as WrappedInputs
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct WrappedOpcode {
    pub opcode: Opcode,
    pub inputs: Vec<WrappedInput>,
}

impl WrappedOpcode {
    pub fn depth(&self) -> u32 {
        self.inputs.iter().map(|x| x.depth()).max().unwrap_or(0) + 1
    }
}

impl WrappedInput {
    pub fn depth(&self) -> u32 {
        match self {
            WrappedInput::Raw(_) => 0,
            WrappedInput::Opcode(opcode) => opcode.depth(),
        }
    }
}

// implements pretty printing for WrappedOpcodes
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
