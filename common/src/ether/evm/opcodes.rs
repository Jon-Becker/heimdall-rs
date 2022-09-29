use std::fmt::{Display, Formatter, Result};

use ethers::types::U256;

#[derive(Clone, Debug, PartialEq)]
pub struct Opcode {
    pub name: String,
    pub mingas: u16,
    pub inputs: u16,
    pub outputs: u16,
}

// Returns the opcode for the given hexcode, fetched from the hashmap.
pub fn opcode(code: &str) -> Opcode {
    return match code {
        "00" => Opcode { name: String::from("STOP"), mingas: 0, inputs: 0, outputs: 0 },
        "01" => Opcode { name: String::from("ADD"), mingas: 3, inputs: 2, outputs: 1 },
        "02" => Opcode { name: String::from("MUL"), mingas: 5, inputs: 2, outputs: 1 },
        "03" => Opcode { name: String::from("SUB"), mingas: 3, inputs: 2, outputs: 1 },
        "04" => Opcode { name: String::from("DIV"), mingas: 5, inputs: 2, outputs: 1 },
        "05" => Opcode { name: String::from("SDIV"), mingas: 5, inputs: 2, outputs: 1 },
        "06" => Opcode { name: String::from("MOD"), mingas: 5, inputs: 2, outputs: 1 },
        "07" => Opcode { name: String::from("SMOD"), mingas: 5, inputs: 2, outputs: 1 },
        "08" => Opcode { name: String::from("ADDMOD"), mingas: 8, inputs: 3, outputs: 1 },
        "09" => Opcode { name: String::from("MULMOD"), mingas: 8, inputs: 3, outputs: 1 },
        "0a" => Opcode { name: String::from("EXP"), mingas: 10, inputs: 2, outputs: 1 },
        "0b" => Opcode { name: String::from("SIGNEXTEND"), mingas: 5, inputs: 2, outputs: 1 },
        "10" => Opcode { name: String::from("LT"), mingas: 3, inputs: 2, outputs: 1 },
        "11" => Opcode { name: String::from("GT"), mingas: 3, inputs: 2, outputs: 1 },
        "12" => Opcode { name: String::from("SLT"), mingas: 3, inputs: 2, outputs: 1 },
        "13" => Opcode { name: String::from("SGT"), mingas: 3, inputs: 2, outputs: 1 },
        "14" => Opcode { name: String::from("EQ"), mingas: 3, inputs: 2, outputs: 1 },
        "15" => Opcode { name: String::from("ISZERO"), mingas: 3, inputs: 1, outputs: 1 },
        "16" => Opcode { name: String::from("AND"), mingas: 3, inputs: 2, outputs: 1 },
        "17" => Opcode { name: String::from("OR"), mingas: 3, inputs: 2, outputs: 1 },
        "18" => Opcode { name: String::from("XOR"), mingas: 3, inputs: 2, outputs: 1 },
        "19" => Opcode { name: String::from("NOT"), mingas: 3, inputs: 1, outputs: 1 },
        "1a" => Opcode { name: String::from("BYTE"), mingas: 3, inputs: 2, outputs: 1 },
        "1b" => Opcode { name: String::from("SHL"), mingas: 3, inputs: 2, outputs: 1 },
        "1c" => Opcode { name: String::from("SHR"), mingas: 3, inputs: 2, outputs: 1 },
        "1d" => Opcode { name: String::from("SAR"), mingas: 3, inputs: 2, outputs: 1 },
        "20" => Opcode { name: String::from("SHA3"), mingas: 30, inputs: 2, outputs: 1 },
        "30" => Opcode { name: String::from("ADDRESS"), mingas: 2, inputs: 0, outputs: 1 },
        "31" => Opcode { name: String::from("BALANCE"), mingas: 100, inputs: 1, outputs: 1 },
        "32" => Opcode { name: String::from("ORIGIN"), mingas: 2, inputs: 0, outputs: 1 },
        "33" => Opcode { name: String::from("CALLER"), mingas: 2, inputs: 0, outputs: 1 },
        "34" => Opcode { name: String::from("CALLVALUE"), mingas: 2, inputs: 0, outputs: 1 },
        "35" => Opcode { name: String::from("CALLDATALOAD"), mingas: 3, inputs: 1, outputs: 1 },
        "36" => Opcode { name: String::from("CALLDATASIZE"), mingas: 2, inputs: 0, outputs: 1 },
        "37" => Opcode { name: String::from("CALLDATACOPY"), mingas: 3, inputs: 3, outputs: 0 },
        "38" => Opcode { name: String::from("CODESIZE"), mingas: 2, inputs: 0, outputs: 1 },
        "39" => Opcode { name: String::from("CODECOPY"), mingas: 3, inputs: 3, outputs: 0 },
        "3a" => Opcode { name: String::from("GASPRICE"), mingas: 2, inputs: 0, outputs: 1 },
        "3b" => Opcode { name: String::from("EXTCODESIZE"), mingas: 100, inputs: 1, outputs: 1 },
        "3c" => Opcode { name: String::from("EXTCODECOPY"), mingas: 100, inputs: 4, outputs: 0 },
        "3d" => Opcode { name: String::from("RETURNDATASIZE"), mingas: 2, inputs: 0, outputs: 1 },
        "3e" => Opcode { name: String::from("RETURNDATACOPY"), mingas: 3, inputs: 3, outputs: 0 },
        "3f" => Opcode { name: String::from("EXTCODEHASH"), mingas: 100, inputs: 1, outputs: 1 },
        "40" => Opcode { name: String::from("BLOCKHASH"), mingas: 20, inputs: 1, outputs: 1 },
        "41" => Opcode { name: String::from("COINBASE"), mingas: 2, inputs: 0, outputs: 1 },
        "42" => Opcode { name: String::from("TIMESTAMP"), mingas: 2, inputs: 0, outputs: 1 },
        "43" => Opcode { name: String::from("NUMBER"), mingas: 2, inputs: 0, outputs: 1 },
        "44" => Opcode { name: String::from("DIFFICULTY"), mingas: 2, inputs: 0, outputs: 1 },
        "45" => Opcode { name: String::from("GASLIMIT"), mingas: 2, inputs: 0, outputs: 1 },
        "46" => Opcode { name: String::from("CHAINID"), mingas: 2, inputs: 0, outputs: 1 },
        "47" => Opcode { name: String::from("SELFBALANCE"), mingas: 5, inputs: 0, outputs: 1 },
        "48" => Opcode { name: String::from("BASEFEE"), mingas: 2, inputs: 0, outputs: 1 },
        "50" => Opcode { name: String::from("POP"), mingas: 2, inputs: 1, outputs: 0 },
        "51" => Opcode { name: String::from("MLOAD"), mingas: 3, inputs: 1, outputs: 1 },
        "52" => Opcode { name: String::from("MSTORE"), mingas: 3, inputs: 2, outputs: 0 },
        "53" => Opcode { name: String::from("MSTORE8"), mingas: 3, inputs: 2, outputs: 0 },
        "54" => Opcode { name: String::from("SLOAD"), mingas: 100, inputs: 1, outputs: 1 },
        "55" => Opcode { name: String::from("SSTORE"), mingas: 100, inputs: 2, outputs: 0 },
        "56" => Opcode { name: String::from("JUMP"), mingas: 8, inputs: 1, outputs: 0 },
        "57" => Opcode { name: String::from("JUMPI"), mingas: 10, inputs: 2, outputs: 0 },
        "58" => Opcode { name: String::from("PC"), mingas: 2, inputs: 0, outputs: 1 },
        "59" => Opcode { name: String::from("MSIZE"), mingas: 2, inputs: 0, outputs: 1 },
        "5a" => Opcode { name: String::from("GAS"), mingas: 2, inputs: 0, outputs: 1 },
        "5b" => Opcode { name: String::from("JUMPDEST"), mingas: 1, inputs: 0, outputs: 0 },
        "60" => Opcode { name: String::from("PUSH1"), mingas: 3, inputs: 0, outputs: 1 },
        "61" => Opcode { name: String::from("PUSH2"), mingas: 3, inputs: 0, outputs: 1 },
        "62" => Opcode { name: String::from("PUSH3"), mingas: 3, inputs: 0, outputs: 1 },
        "63" => Opcode { name: String::from("PUSH4"), mingas: 3, inputs: 0, outputs: 1 },
        "64" => Opcode { name: String::from("PUSH5"), mingas: 3, inputs: 0, outputs: 1 },
        "65" => Opcode { name: String::from("PUSH6"), mingas: 3, inputs: 0, outputs: 1 },
        "66" => Opcode { name: String::from("PUSH7"), mingas: 3, inputs: 0, outputs: 1 },
        "67" => Opcode { name: String::from("PUSH8"), mingas: 3, inputs: 0, outputs: 1 },
        "68" => Opcode { name: String::from("PUSH9"), mingas: 3, inputs: 0, outputs: 1 },
        "69" => Opcode { name: String::from("PUSH10"), mingas: 3, inputs: 0, outputs: 1 },
        "6a" => Opcode { name: String::from("PUSH11"), mingas: 3, inputs: 0, outputs: 1 },
        "6b" => Opcode { name: String::from("PUSH12"), mingas: 3, inputs: 0, outputs: 1 },
        "6c" => Opcode { name: String::from("PUSH13"), mingas: 3, inputs: 0, outputs: 1 },
        "6d" => Opcode { name: String::from("PUSH14"), mingas: 3, inputs: 0, outputs: 1 },
        "6e" => Opcode { name: String::from("PUSH15"), mingas: 3, inputs: 0, outputs: 1 },
        "6f" => Opcode { name: String::from("PUSH16"), mingas: 3, inputs: 0, outputs: 1 },
        "70" => Opcode { name: String::from("PUSH17"), mingas: 3, inputs: 0, outputs: 1 },
        "71" => Opcode { name: String::from("PUSH18"), mingas: 3, inputs: 0, outputs: 1 },
        "72" => Opcode { name: String::from("PUSH19"), mingas: 3, inputs: 0, outputs: 1 },
        "73" => Opcode { name: String::from("PUSH20"), mingas: 3, inputs: 0, outputs: 1 },
        "74" => Opcode { name: String::from("PUSH21"), mingas: 3, inputs: 0, outputs: 1 },
        "75" => Opcode { name: String::from("PUSH22"), mingas: 3, inputs: 0, outputs: 1 },
        "76" => Opcode { name: String::from("PUSH23"), mingas: 3, inputs: 0, outputs: 1 },
        "77" => Opcode { name: String::from("PUSH24"), mingas: 3, inputs: 0, outputs: 1 },
        "78" => Opcode { name: String::from("PUSH25"), mingas: 3, inputs: 0, outputs: 1 },
        "79" => Opcode { name: String::from("PUSH26"), mingas: 3, inputs: 0, outputs: 1 },
        "7a" => Opcode { name: String::from("PUSH27"), mingas: 3, inputs: 0, outputs: 1 },
        "7b" => Opcode { name: String::from("PUSH28"), mingas: 3, inputs: 0, outputs: 1 },
        "7c" => Opcode { name: String::from("PUSH29"), mingas: 3, inputs: 0, outputs: 1 },
        "7d" => Opcode { name: String::from("PUSH30"), mingas: 3, inputs: 0, outputs: 1 },
        "7e" => Opcode { name: String::from("PUSH31"), mingas: 3, inputs: 0, outputs: 1 },
        "7f" => Opcode { name: String::from("PUSH32"), mingas: 3, inputs: 0, outputs: 1 },
        "80" => Opcode { name: String::from("DUP1"), mingas: 3, inputs: 1, outputs: 2 },
        "81" => Opcode { name: String::from("DUP2"), mingas: 3, inputs: 2, outputs: 3 },
        "82" => Opcode { name: String::from("DUP3"), mingas: 3, inputs: 3, outputs: 4 },
        "83" => Opcode { name: String::from("DUP4"), mingas: 3, inputs: 4, outputs: 5 },
        "84" => Opcode { name: String::from("DUP5"), mingas: 3, inputs: 5, outputs: 6 },
        "85" => Opcode { name: String::from("DUP6"), mingas: 3, inputs: 6, outputs: 7 },
        "86" => Opcode { name: String::from("DUP7"), mingas: 3, inputs: 7, outputs: 8 },
        "87" => Opcode { name: String::from("DUP8"), mingas: 3, inputs: 8, outputs: 9 },
        "88" => Opcode { name: String::from("DUP9"), mingas: 3, inputs: 9, outputs: 10 },
        "89" => Opcode { name: String::from("DUP10"), mingas: 3, inputs: 10, outputs: 11 },
        "8a" => Opcode { name: String::from("DUP11"), mingas: 3, inputs: 11, outputs: 12 },
        "8b" => Opcode { name: String::from("DUP12"), mingas: 3, inputs: 12, outputs: 13 },
        "8c" => Opcode { name: String::from("DUP13"), mingas: 3, inputs: 13, outputs: 14 },
        "8d" => Opcode { name: String::from("DUP14"), mingas: 3, inputs: 14, outputs: 15 },
        "8e" => Opcode { name: String::from("DUP15"), mingas: 3, inputs: 15, outputs: 16 },
        "8f" => Opcode { name: String::from("DUP16"), mingas: 3, inputs: 16, outputs: 17 },
        "90" => Opcode { name: String::from("SWAP1"), mingas: 3, inputs: 2, outputs: 2 },
        "91" => Opcode { name: String::from("SWAP2"), mingas: 3, inputs: 3, outputs: 3 },
        "92" => Opcode { name: String::from("SWAP3"), mingas: 3, inputs: 4, outputs: 4 },
        "93" => Opcode { name: String::from("SWAP4"), mingas: 3, inputs: 5, outputs: 5 },
        "94" => Opcode { name: String::from("SWAP5"), mingas: 3, inputs: 6, outputs: 6 },
        "95" => Opcode { name: String::from("SWAP6"), mingas: 3, inputs: 7, outputs: 7 },
        "96" => Opcode { name: String::from("SWAP7"), mingas: 3, inputs: 8, outputs: 8 },
        "97" => Opcode { name: String::from("SWAP8"), mingas: 3, inputs: 9, outputs: 9 },
        "98" => Opcode { name: String::from("SWAP9"), mingas: 3, inputs: 10, outputs: 10 },
        "99" => Opcode { name: String::from("SWAP10"), mingas: 3, inputs: 11, outputs: 11 },
        "9a" => Opcode { name: String::from("SWAP11"), mingas: 3, inputs: 12, outputs: 12 },
        "9b" => Opcode { name: String::from("SWAP12"), mingas: 3, inputs: 13, outputs: 13 },
        "9c" => Opcode { name: String::from("SWAP13"), mingas: 3, inputs: 14, outputs: 14 },
        "9d" => Opcode { name: String::from("SWAP14"), mingas: 3, inputs: 15, outputs: 15 },
        "9e" => Opcode { name: String::from("SWAP15"), mingas: 3, inputs: 16, outputs: 16 },
        "9f" => Opcode { name: String::from("SWAP16"), mingas: 3, inputs: 17, outputs: 17 },
        "a0" => Opcode { name: String::from("LOG0"), mingas: 375, inputs: 2, outputs: 0 },
        "a1" => Opcode { name: String::from("LOG1"), mingas: 750, inputs: 3, outputs: 0 },
        "a2" => Opcode { name: String::from("LOG2"), mingas: 1125, inputs: 4, outputs: 0 },
        "a3" => Opcode { name: String::from("LOG3"), mingas: 1500, inputs: 5, outputs: 0 },
        "a4" => Opcode { name: String::from("LOG4"), mingas: 1875, inputs: 6, outputs: 0 },
        "f0" => Opcode { name: String::from("CREATE"), mingas: 32000, inputs: 3, outputs: 1 },
        "f1" => Opcode { name: String::from("CALL"), mingas: 100, inputs: 7, outputs: 1 },
        "f2" => Opcode { name: String::from("CALLCODE"), mingas: 100, inputs: 7, outputs: 1 },
        "f3" => Opcode { name: String::from("RETURN"), mingas: 0, inputs: 2, outputs: 0 },
        "f4" => Opcode { name: String::from("DELEGATECALL"), mingas: 100, inputs: 6, outputs: 1 },
        "f5" => Opcode { name: String::from("CREATE2"), mingas: 32000, inputs: 4, outputs: 1 },
        "fa" => Opcode { name: String::from("STATICCALL"), mingas: 100, inputs: 6, outputs: 1 },
        "fd" => Opcode { name: String::from("REVERT"), mingas: 0, inputs: 2, outputs: 0 },
        "fe" => Opcode { name: String::from("INVALID"), mingas: 0, inputs: 0, outputs: 0 },
        "ff" => Opcode { name: String::from("SELFDESTRUCT"), mingas: 5000, inputs: 1, outputs: 0 },
           _ => Opcode { name: String::from("unknown"), mingas: 0, inputs: 0, outputs: 0, },
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
        write!(f, "{}({})", self.opcode.name, self.inputs.clone().into_iter().map(|x| x.to_string()).collect::<Vec<String>>().join(", "))
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