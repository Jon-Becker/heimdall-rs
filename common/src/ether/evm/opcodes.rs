use std::fmt::{Display, Formatter, Result};
use ethers::types::U256;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Opcode {
    pub name: &'static str,
    pub mingas: u16,
    pub inputs: u16,
    pub outputs: u16,
}

// Returns the opcode for the given hexcode, fetched from the hashmap.
pub fn opcode(code: &str) -> Opcode {
    return match code {
        "00" => Opcode { name: "STOP", mingas: 0, inputs: 0, outputs: 0 },
        "01" => Opcode { name: "ADD", mingas: 3, inputs: 2, outputs: 1 },
        "02" => Opcode { name: "MUL", mingas: 5, inputs: 2, outputs: 1 },
        "03" => Opcode { name: "SUB", mingas: 3, inputs: 2, outputs: 1 },
        "04" => Opcode { name: "DIV", mingas: 5, inputs: 2, outputs: 1 },
        "05" => Opcode { name: "SDIV", mingas: 5, inputs: 2, outputs: 1 },
        "06" => Opcode { name: "MOD", mingas: 5, inputs: 2, outputs: 1 },
        "07" => Opcode { name: "SMOD", mingas: 5, inputs: 2, outputs: 1 },
        "08" => Opcode { name: "ADDMOD", mingas: 8, inputs: 3, outputs: 1 },
        "09" => Opcode { name: "MULMOD", mingas: 8, inputs: 3, outputs: 1 },
        "0a" => Opcode { name: "EXP", mingas: 10, inputs: 2, outputs: 1 },
        "0b" => Opcode { name: "SIGNEXTEND", mingas: 5, inputs: 2, outputs: 1 },
        "10" => Opcode { name: "LT", mingas: 3, inputs: 2, outputs: 1 },
        "11" => Opcode { name: "GT", mingas: 3, inputs: 2, outputs: 1 },
        "12" => Opcode { name: "SLT", mingas: 3, inputs: 2, outputs: 1 },
        "13" => Opcode { name: "SGT", mingas: 3, inputs: 2, outputs: 1 },
        "14" => Opcode { name: "EQ", mingas: 3, inputs: 2, outputs: 1 },
        "15" => Opcode { name: "ISZERO", mingas: 3, inputs: 1, outputs: 1 },
        "16" => Opcode { name: "AND", mingas: 3, inputs: 2, outputs: 1 },
        "17" => Opcode { name: "OR", mingas: 3, inputs: 2, outputs: 1 },
        "18" => Opcode { name: "XOR", mingas: 3, inputs: 2, outputs: 1 },
        "19" => Opcode { name: "NOT", mingas: 3, inputs: 1, outputs: 1 },
        "1a" => Opcode { name: "BYTE", mingas: 3, inputs: 2, outputs: 1 },
        "1b" => Opcode { name: "SHL", mingas: 3, inputs: 2, outputs: 1 },
        "1c" => Opcode { name: "SHR", mingas: 3, inputs: 2, outputs: 1 },
        "1d" => Opcode { name: "SAR", mingas: 3, inputs: 2, outputs: 1 },
        "20" => Opcode { name: "SHA3", mingas: 30, inputs: 2, outputs: 1 },
        "30" => Opcode { name: "ADDRESS", mingas: 2, inputs: 0, outputs: 1 },
        "31" => Opcode { name: "BALANCE", mingas: 100, inputs: 1, outputs: 1 },
        "32" => Opcode { name: "ORIGIN", mingas: 2, inputs: 0, outputs: 1 },
        "33" => Opcode { name: "CALLER", mingas: 2, inputs: 0, outputs: 1 },
        "34" => Opcode { name: "CALLVALUE", mingas: 2, inputs: 0, outputs: 1 },
        "35" => Opcode { name: "CALLDATALOAD", mingas: 3, inputs: 1, outputs: 1 },
        "36" => Opcode { name: "CALLDATASIZE", mingas: 2, inputs: 0, outputs: 1 },
        "37" => Opcode { name: "CALLDATACOPY", mingas: 3, inputs: 3, outputs: 0 },
        "38" => Opcode { name: "CODESIZE", mingas: 2, inputs: 0, outputs: 1 },
        "39" => Opcode { name: "CODECOPY", mingas: 3, inputs: 3, outputs: 0 },
        "3a" => Opcode { name: "GASPRICE", mingas: 2, inputs: 0, outputs: 1 },
        "3b" => Opcode { name: "EXTCODESIZE", mingas: 100, inputs: 1, outputs: 1 },
        "3c" => Opcode { name: "EXTCODECOPY", mingas: 100, inputs: 4, outputs: 0 },
        "3d" => Opcode { name: "RETURNDATASIZE", mingas: 2, inputs: 0, outputs: 1 },
        "3e" => Opcode { name: "RETURNDATACOPY", mingas: 3, inputs: 3, outputs: 0 },
        "3f" => Opcode { name: "EXTCODEHASH", mingas: 100, inputs: 1, outputs: 1 },
        "40" => Opcode { name: "BLOCKHASH", mingas: 20, inputs: 1, outputs: 1 },
        "41" => Opcode { name: "COINBASE", mingas: 2, inputs: 0, outputs: 1 },
        "42" => Opcode { name: "TIMESTAMP", mingas: 2, inputs: 0, outputs: 1 },
        "43" => Opcode { name: "NUMBER", mingas: 2, inputs: 0, outputs: 1 },
        "44" => Opcode { name: "DIFFICULTY", mingas: 2, inputs: 0, outputs: 1 },
        "45" => Opcode { name: "GASLIMIT", mingas: 2, inputs: 0, outputs: 1 },
        "46" => Opcode { name: "CHAINID", mingas: 2, inputs: 0, outputs: 1 },
        "47" => Opcode { name: "SELFBALANCE", mingas: 5, inputs: 0, outputs: 1 },
        "48" => Opcode { name: "BASEFEE", mingas: 2, inputs: 0, outputs: 1 },
        "50" => Opcode { name: "POP", mingas: 2, inputs: 1, outputs: 0 },
        "51" => Opcode { name: "MLOAD", mingas: 3, inputs: 1, outputs: 1 },
        "52" => Opcode { name: "MSTORE", mingas: 3, inputs: 2, outputs: 0 },
        "53" => Opcode { name: "MSTORE8", mingas: 3, inputs: 2, outputs: 0 },
        "54" => Opcode { name: "SLOAD", mingas: 100, inputs: 1, outputs: 1 },
        "55" => Opcode { name: "SSTORE", mingas: 100, inputs: 2, outputs: 0 },
        "56" => Opcode { name: "JUMP", mingas: 8, inputs: 1, outputs: 0 },
        "57" => Opcode { name: "JUMPI", mingas: 10, inputs: 2, outputs: 0 },
        "58" => Opcode { name: "PC", mingas: 2, inputs: 0, outputs: 1 },
        "59" => Opcode { name: "MSIZE", mingas: 2, inputs: 0, outputs: 1 },
        "5a" => Opcode { name: "GAS", mingas: 2, inputs: 0, outputs: 1 },
        "5b" => Opcode { name: "JUMPDEST", mingas: 1, inputs: 0, outputs: 0 },
        "60" => Opcode { name: "PUSH1", mingas: 3, inputs: 0, outputs: 1 },
        "61" => Opcode { name: "PUSH2", mingas: 3, inputs: 0, outputs: 1 },
        "62" => Opcode { name: "PUSH3", mingas: 3, inputs: 0, outputs: 1 },
        "63" => Opcode { name: "PUSH4", mingas: 3, inputs: 0, outputs: 1 },
        "64" => Opcode { name: "PUSH5", mingas: 3, inputs: 0, outputs: 1 },
        "65" => Opcode { name: "PUSH6", mingas: 3, inputs: 0, outputs: 1 },
        "66" => Opcode { name: "PUSH7", mingas: 3, inputs: 0, outputs: 1 },
        "67" => Opcode { name: "PUSH8", mingas: 3, inputs: 0, outputs: 1 },
        "68" => Opcode { name: "PUSH9", mingas: 3, inputs: 0, outputs: 1 },
        "69" => Opcode { name: "PUSH10", mingas: 3, inputs: 0, outputs: 1 },
        "6a" => Opcode { name: "PUSH11", mingas: 3, inputs: 0, outputs: 1 },
        "6b" => Opcode { name: "PUSH12", mingas: 3, inputs: 0, outputs: 1 },
        "6c" => Opcode { name: "PUSH13", mingas: 3, inputs: 0, outputs: 1 },
        "6d" => Opcode { name: "PUSH14", mingas: 3, inputs: 0, outputs: 1 },
        "6e" => Opcode { name: "PUSH15", mingas: 3, inputs: 0, outputs: 1 },
        "6f" => Opcode { name: "PUSH16", mingas: 3, inputs: 0, outputs: 1 },
        "70" => Opcode { name: "PUSH17", mingas: 3, inputs: 0, outputs: 1 },
        "71" => Opcode { name: "PUSH18", mingas: 3, inputs: 0, outputs: 1 },
        "72" => Opcode { name: "PUSH19", mingas: 3, inputs: 0, outputs: 1 },
        "73" => Opcode { name: "PUSH20", mingas: 3, inputs: 0, outputs: 1 },
        "74" => Opcode { name: "PUSH21", mingas: 3, inputs: 0, outputs: 1 },
        "75" => Opcode { name: "PUSH22", mingas: 3, inputs: 0, outputs: 1 },
        "76" => Opcode { name: "PUSH23", mingas: 3, inputs: 0, outputs: 1 },
        "77" => Opcode { name: "PUSH24", mingas: 3, inputs: 0, outputs: 1 },
        "78" => Opcode { name: "PUSH25", mingas: 3, inputs: 0, outputs: 1 },
        "79" => Opcode { name: "PUSH26", mingas: 3, inputs: 0, outputs: 1 },
        "7a" => Opcode { name: "PUSH27", mingas: 3, inputs: 0, outputs: 1 },
        "7b" => Opcode { name: "PUSH28", mingas: 3, inputs: 0, outputs: 1 },
        "7c" => Opcode { name: "PUSH29", mingas: 3, inputs: 0, outputs: 1 },
        "7d" => Opcode { name: "PUSH30", mingas: 3, inputs: 0, outputs: 1 },
        "7e" => Opcode { name: "PUSH31", mingas: 3, inputs: 0, outputs: 1 },
        "7f" => Opcode { name: "PUSH32", mingas: 3, inputs: 0, outputs: 1 },
        "80" => Opcode { name: "DUP1", mingas: 3, inputs: 1, outputs: 2 },
        "81" => Opcode { name: "DUP2", mingas: 3, inputs: 2, outputs: 3 },
        "82" => Opcode { name: "DUP3", mingas: 3, inputs: 3, outputs: 4 },
        "83" => Opcode { name: "DUP4", mingas: 3, inputs: 4, outputs: 5 },
        "84" => Opcode { name: "DUP5", mingas: 3, inputs: 5, outputs: 6 },
        "85" => Opcode { name: "DUP6", mingas: 3, inputs: 6, outputs: 7 },
        "86" => Opcode { name: "DUP7", mingas: 3, inputs: 7, outputs: 8 },
        "87" => Opcode { name: "DUP8", mingas: 3, inputs: 8, outputs: 9 },
        "88" => Opcode { name: "DUP9", mingas: 3, inputs: 9, outputs: 10 },
        "89" => Opcode { name: "DUP10", mingas: 3, inputs: 10, outputs: 11 },
        "8a" => Opcode { name: "DUP11", mingas: 3, inputs: 11, outputs: 12 },
        "8b" => Opcode { name: "DUP12", mingas: 3, inputs: 12, outputs: 13 },
        "8c" => Opcode { name: "DUP13", mingas: 3, inputs: 13, outputs: 14 },
        "8d" => Opcode { name: "DUP14", mingas: 3, inputs: 14, outputs: 15 },
        "8e" => Opcode { name: "DUP15", mingas: 3, inputs: 15, outputs: 16 },
        "8f" => Opcode { name: "DUP16", mingas: 3, inputs: 16, outputs: 17 },
        "90" => Opcode { name: "SWAP1", mingas: 3, inputs: 2, outputs: 2 },
        "91" => Opcode { name: "SWAP2", mingas: 3, inputs: 3, outputs: 3 },
        "92" => Opcode { name: "SWAP3", mingas: 3, inputs: 4, outputs: 4 },
        "93" => Opcode { name: "SWAP4", mingas: 3, inputs: 5, outputs: 5 },
        "94" => Opcode { name: "SWAP5", mingas: 3, inputs: 6, outputs: 6 },
        "95" => Opcode { name: "SWAP6", mingas: 3, inputs: 7, outputs: 7 },
        "96" => Opcode { name: "SWAP7", mingas: 3, inputs: 8, outputs: 8 },
        "97" => Opcode { name: "SWAP8", mingas: 3, inputs: 9, outputs: 9 },
        "98" => Opcode { name: "SWAP9", mingas: 3, inputs: 10, outputs: 10 },
        "99" => Opcode { name: "SWAP10", mingas: 3, inputs: 11, outputs: 11 },
        "9a" => Opcode { name: "SWAP11", mingas: 3, inputs: 12, outputs: 12 },
        "9b" => Opcode { name: "SWAP12", mingas: 3, inputs: 13, outputs: 13 },
        "9c" => Opcode { name: "SWAP13", mingas: 3, inputs: 14, outputs: 14 },
        "9d" => Opcode { name: "SWAP14", mingas: 3, inputs: 15, outputs: 15 },
        "9e" => Opcode { name: "SWAP15", mingas: 3, inputs: 16, outputs: 16 },
        "9f" => Opcode { name: "SWAP16", mingas: 3, inputs: 17, outputs: 17 },
        "a0" => Opcode { name: "LOG0", mingas: 375, inputs: 2, outputs: 0 },
        "a1" => Opcode { name: "LOG1", mingas: 750, inputs: 3, outputs: 0 },
        "a2" => Opcode { name: "LOG2", mingas: 1125, inputs: 4, outputs: 0 },
        "a3" => Opcode { name: "LOG3", mingas: 1500, inputs: 5, outputs: 0 },
        "a4" => Opcode { name: "LOG4", mingas: 1875, inputs: 6, outputs: 0 },
        "f0" => Opcode { name: "CREATE", mingas: 32000, inputs: 3, outputs: 1 },
        "f1" => Opcode { name: "CALL", mingas: 100, inputs: 7, outputs: 1 },
        "f2" => Opcode { name: "CALLCODE", mingas: 100, inputs: 7, outputs: 1 },
        "f3" => Opcode { name: "RETURN", mingas: 0, inputs: 2, outputs: 0 },
        "f4" => Opcode { name: "DELEGATECALL", mingas: 100, inputs: 6, outputs: 1 },
        "f5" => Opcode { name: "CREATE2", mingas: 32000, inputs: 4, outputs: 1 },
        "fa" => Opcode { name: "STATICCALL", mingas: 100, inputs: 6, outputs: 1 },
        "fd" => Opcode { name: "REVERT", mingas: 0, inputs: 2, outputs: 0 },
        "fe" => Opcode { name: "INVALID", mingas: 0, inputs: 0, outputs: 0 },
        "ff" => Opcode { name: "SELFDESTRUCT", mingas: 5000, inputs: 1, outputs: 0 },
        _ => Opcode { name: "unknown", mingas: 0, inputs: 0, outputs: 0, },
    };
}

// enum allows for Wrapped Opcodes to contain both raw U256 and Opcodes as inputs
#[derive(Clone, Debug, PartialEq)]
pub enum WrappedInput {
    Raw(U256),
    Opcode(WrappedOpcode),
}

// represents an opcode with its direct inputs as WrappedInputs
#[derive(Clone, Debug, PartialEq)]
pub struct WrappedOpcode {
    pub opcode: Opcode,
    pub inputs: Vec<WrappedInput>,
}

// implements pretty printing for WrappedOpcodes
impl Display for WrappedOpcode {

    fn fmt(&self, f: &mut Formatter) -> Result {
        write!(f, "{}({})", self.opcode.name, self.inputs.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(", "))
    }

}

impl Display for WrappedInput {
    fn fmt(&self, f: &mut Formatter) -> Result {
        match self {
            WrappedInput::Raw(u256) => write!(f, "{}", u256),
            WrappedInput::Opcode(opcode) => write!(f, "{}", opcode),
        }
    }
}