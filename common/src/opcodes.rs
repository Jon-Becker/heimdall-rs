use std::collections::HashMap;

#[derive(Clone, Debug)]
pub struct Opcode {
    pub name: String,
    pub hexcode: String,
    pub mingas: u16,
}

lazy_static! {

    // The opcode hashmap is defined here. It contains all the 
    // opcodes and their corresponding hexadecimal code, as well as 
    // the minimum gas cost for each opcode.
    static ref OPCODES: HashMap<&'static str, Opcode> = {
        let mut m = HashMap::new();
        m.insert("00", Opcode { name: String::from("STOP"), hexcode: String::from("0x00"), mingas: 0 });
        m.insert("01", Opcode { name: String::from("ADD"), hexcode: String::from("0x01"), mingas: 3 });
        m.insert("02", Opcode { name: String::from("MUL"), hexcode: String::from("0x02"), mingas: 5 });
        m.insert("03", Opcode { name: String::from("SUB"), hexcode: String::from("0x03"), mingas: 3 });
        m.insert("04", Opcode { name: String::from("DIV"), hexcode: String::from("0x04"), mingas: 5 });
        m.insert("05", Opcode { name: String::from("SDIV"), hexcode: String::from("0x05"), mingas: 5 });
        m.insert("06", Opcode { name: String::from("MOD"), hexcode: String::from("0x06"), mingas: 5 });
        m.insert("07", Opcode { name: String::from("SMOD"), hexcode: String::from("0x07"), mingas: 5 });
        m.insert("08", Opcode { name: String::from("ADDMOD"), hexcode: String::from("0x08"), mingas: 8 });
        m.insert("09", Opcode { name: String::from("MULMOD"), hexcode: String::from("0x09"), mingas: 8 });
        m.insert("0a", Opcode { name: String::from("EXP"), hexcode: String::from("0x0a"), mingas: 10 });
        m.insert("0b", Opcode { name: String::from("SIGNEXTEND"), hexcode: String::from("0x0b"), mingas: 5 });
        m.insert("10", Opcode { name: String::from("LT"), hexcode: String::from("0x10"), mingas: 3 });
        m.insert("11", Opcode { name: String::from("GT"), hexcode: String::from("0x11"), mingas: 3 });
        m.insert("12", Opcode { name: String::from("SLT"), hexcode: String::from("0x12"), mingas: 3 });
        m.insert("13", Opcode { name: String::from("SGT"), hexcode: String::from("0x13"), mingas: 3 });
        m.insert("14", Opcode { name: String::from("EQ"), hexcode: String::from("0x14"), mingas: 3 });
        m.insert("15", Opcode { name: String::from("ISZERO"), hexcode: String::from("0x15"), mingas: 3 });
        m.insert("16", Opcode { name: String::from("AND"), hexcode: String::from("0x16"), mingas: 3 });
        m.insert("17", Opcode { name: String::from("OR"), hexcode: String::from("0x17"), mingas: 3 });
        m.insert("18", Opcode { name: String::from("XOR"), hexcode: String::from("0x18"), mingas: 3 });
        m.insert("19", Opcode { name: String::from("NOT"), hexcode: String::from("0x19"), mingas: 3 });
        m.insert("1a", Opcode { name: String::from("BYTE"), hexcode: String::from("0x1a"), mingas: 3 });
        m.insert("1b", Opcode { name: String::from("SHL"), hexcode: String::from("0x2b"), mingas: 3 });
        m.insert("1c", Opcode { name: String::from("SHR"), hexcode: String::from("0x1c"), mingas: 3 });
        m.insert("1d", Opcode { name: String::from("SAR"), hexcode: String::from("0x1d"), mingas: 3 });
        m.insert("20", Opcode { name: String::from("SHA3"), hexcode: String::from("0x20"), mingas: 30 });
        m.insert("30", Opcode { name: String::from("ADDRESS"), hexcode: String::from("0x30"), mingas: 2 });
        m.insert("31", Opcode { name: String::from("BALANCE"), hexcode: String::from("0x31"), mingas: 100 });
        m.insert("32", Opcode { name: String::from("ORIGIN"), hexcode: String::from("0x32"), mingas: 2 });
        m.insert("33", Opcode { name: String::from("CALLER"), hexcode: String::from("0x33"), mingas: 2 });
        m.insert("34", Opcode { name: String::from("CALLVALUE"), hexcode: String::from("0x34"), mingas: 2 });
        m.insert("35", Opcode { name: String::from("CALLDATALOAD"), hexcode: String::from("0x35"), mingas: 3 });
        m.insert("36", Opcode { name: String::from("CALLDATASIZE"), hexcode: String::from("0x36"), mingas: 2 });
        m.insert("37", Opcode { name: String::from("CALLDATACOPY"), hexcode: String::from("0x37"), mingas: 3 });
        m.insert("38", Opcode { name: String::from("CODESIZE"), hexcode: String::from("0x38"), mingas: 2 });
        m.insert("39", Opcode { name: String::from("CODECOPY"), hexcode: String::from("0x39"), mingas: 3 });
        m.insert("3a", Opcode { name: String::from("GASPRICE"), hexcode: String::from("0x3a"), mingas: 2 });
        m.insert("3b", Opcode { name: String::from("EXTCODESIZE"), hexcode: String::from("0x3b"), mingas: 100 });
        m.insert("3c", Opcode { name: String::from("EXTCODECOPY"), hexcode: String::from("0x3c"), mingas: 100 });
        m.insert("3d", Opcode { name: String::from("RETURNDATASIZE"), hexcode: String::from("0x3d"), mingas: 2 });
        m.insert("3e", Opcode { name: String::from("RETURNDATACOPY"), hexcode: String::from("0x3e"), mingas: 3 });
        m.insert("3f", Opcode { name: String::from("EXTCODEHASH"), hexcode: String::from("0x3f"), mingas: 100 });
        m.insert("40", Opcode { name: String::from("BLOCKHASH"), hexcode: String::from("0x40"), mingas: 20 });
        m.insert("41", Opcode { name: String::from("COINBASE"), hexcode: String::from("0x41"), mingas: 2 });
        m.insert("42", Opcode { name: String::from("TIMESTAMP"), hexcode: String::from("0x42"), mingas: 2 });
        m.insert("43", Opcode { name: String::from("NUMBER"), hexcode: String::from("0x43"), mingas: 2 });
        m.insert("44", Opcode { name: String::from("DIFFICULTY"), hexcode: String::from("0x44"), mingas: 2 });
        m.insert("45", Opcode { name: String::from("GASLIMIT"), hexcode: String::from("0x45"), mingas: 2 });
        m.insert("46", Opcode { name: String::from("CHAINID"), hexcode: String::from("0x46"), mingas: 2 });
        m.insert("47", Opcode { name: String::from("SELFBALANCE"), hexcode: String::from("0x47"), mingas: 5 });
        m.insert("48", Opcode { name: String::from("BASEFEE"), hexcode: String::from("0x48"), mingas: 2 });
        m.insert("50", Opcode { name: String::from("POP"), hexcode: String::from("0x50"), mingas: 2 });
        m.insert("51", Opcode { name: String::from("MLOAD"), hexcode: String::from("0x51"), mingas: 3 });
        m.insert("52", Opcode { name: String::from("MSTORE"), hexcode: String::from("0x52"), mingas: 3 });
        m.insert("53", Opcode { name: String::from("MSTORE8"), hexcode: String::from("0x53"), mingas: 3 });
        m.insert("54", Opcode { name: String::from("SLOAD"), hexcode: String::from("0x54"), mingas: 100 });
        m.insert("55", Opcode { name: String::from("SSTORE"), hexcode: String::from("0x55"), mingas: 100 });
        m.insert("56", Opcode { name: String::from("JUMP"), hexcode: String::from("0x56"), mingas: 8 });
        m.insert("57", Opcode { name: String::from("JUMPI"), hexcode: String::from("0x57"), mingas: 10 });
        m.insert("58", Opcode { name: String::from("PC"), hexcode: String::from("0x58"), mingas: 2 });
        m.insert("59", Opcode { name: String::from("MSIZE"), hexcode: String::from("0x59"), mingas: 2 });
        m.insert("5a", Opcode { name: String::from("GAS"), hexcode: String::from("0x5a"), mingas: 2 });
        m.insert("5b", Opcode { name: String::from("JUMPDEST"), hexcode: String::from("0x5b"), mingas: 1 });
        m.insert("60", Opcode { name: String::from("PUSH1"), hexcode: String::from("0x60"), mingas: 3 });
        m.insert("61", Opcode { name: String::from("PUSH2"), hexcode: String::from("0x61"), mingas: 3 });
        m.insert("62", Opcode { name: String::from("PUSH3"), hexcode: String::from("0x62"), mingas: 3 });
        m.insert("63", Opcode { name: String::from("PUSH4"), hexcode: String::from("0x63"), mingas: 3 });
        m.insert("64", Opcode { name: String::from("PUSH5"), hexcode: String::from("0x64"), mingas: 3 });
        m.insert("65", Opcode { name: String::from("PUSH6"), hexcode: String::from("0x65"), mingas: 3 });
        m.insert("66", Opcode { name: String::from("PUSH7"), hexcode: String::from("0x66"), mingas: 3 });
        m.insert("67", Opcode { name: String::from("PUSH8"), hexcode: String::from("0x67"), mingas: 3 });
        m.insert("68", Opcode { name: String::from("PUSH9"), hexcode: String::from("0x68"), mingas: 3 });
        m.insert("69", Opcode { name: String::from("PUSH10"), hexcode: String::from("0x69"), mingas: 3 });
        m.insert("6a", Opcode { name: String::from("PUSH11"), hexcode: String::from("0x6a"), mingas: 3 });
        m.insert("6b", Opcode { name: String::from("PUSH12"), hexcode: String::from("0x6b"), mingas: 3 });
        m.insert("6c", Opcode { name: String::from("PUSH13"), hexcode: String::from("0x6c"), mingas: 3 });
        m.insert("6d", Opcode { name: String::from("PUSH14"), hexcode: String::from("0x6d"), mingas: 3 });
        m.insert("6e", Opcode { name: String::from("PUSH15"), hexcode: String::from("0x6e"), mingas: 3 });
        m.insert("6f", Opcode { name: String::from("PUSH16"), hexcode: String::from("0x6f"), mingas: 3 });
        m.insert("70", Opcode { name: String::from("PUSH17"), hexcode: String::from("0x70"), mingas: 3 });
        m.insert("71", Opcode { name: String::from("PUSH18"), hexcode: String::from("0x71"), mingas: 3 });
        m.insert("72", Opcode { name: String::from("PUSH19"), hexcode: String::from("0x72"), mingas: 3 });
        m.insert("73", Opcode { name: String::from("PUSH20"), hexcode: String::from("0x73"), mingas: 3 });
        m.insert("74", Opcode { name: String::from("PUSH21"), hexcode: String::from("0x74"), mingas: 3 });
        m.insert("75", Opcode { name: String::from("PUSH22"), hexcode: String::from("0x75"), mingas: 3 });
        m.insert("76", Opcode { name: String::from("PUSH23"), hexcode: String::from("0x76"), mingas: 3 });
        m.insert("77", Opcode { name: String::from("PUSH24"), hexcode: String::from("0x77"), mingas: 3 });
        m.insert("78", Opcode { name: String::from("PUSH25"), hexcode: String::from("0x78"), mingas: 3 });
        m.insert("79", Opcode { name: String::from("PUSH26"), hexcode: String::from("0x79"), mingas: 3 });
        m.insert("7a", Opcode { name: String::from("PUSH27"), hexcode: String::from("0x7a"), mingas: 3 });
        m.insert("7b", Opcode { name: String::from("PUSH28"), hexcode: String::from("0x7b"), mingas: 3 });
        m.insert("7c", Opcode { name: String::from("PUSH29"), hexcode: String::from("0x7c"), mingas: 3 });
        m.insert("7d", Opcode { name: String::from("PUSH30"), hexcode: String::from("0x7d"), mingas: 3 });
        m.insert("7e", Opcode { name: String::from("PUSH31"), hexcode: String::from("0x7e"), mingas: 3 });
        m.insert("7f", Opcode { name: String::from("PUSH32"), hexcode: String::from("0x7f"), mingas: 3 });
        m.insert("80", Opcode { name: String::from("DUP1"), hexcode: String::from("0x80"), mingas: 3 });
        m.insert("81", Opcode { name: String::from("DUP2"), hexcode: String::from("0x81"), mingas: 3 });
        m.insert("82", Opcode { name: String::from("DUP3"), hexcode: String::from("0x82"), mingas: 3 });
        m.insert("83", Opcode { name: String::from("DUP4"), hexcode: String::from("0x83"), mingas: 3 });
        m.insert("84", Opcode { name: String::from("DUP5"), hexcode: String::from("0x84"), mingas: 3 });
        m.insert("85", Opcode { name: String::from("DUP6"), hexcode: String::from("0x85"), mingas: 3 });
        m.insert("86", Opcode { name: String::from("DUP7"), hexcode: String::from("0x86"), mingas: 3 });
        m.insert("87", Opcode { name: String::from("DUP8"), hexcode: String::from("0x87"), mingas: 3 });
        m.insert("88", Opcode { name: String::from("DUP9"), hexcode: String::from("0x88"), mingas: 3 });
        m.insert("89", Opcode { name: String::from("DUP10"), hexcode: String::from("0x89"), mingas: 3 });
        m.insert("8a", Opcode { name: String::from("DUP11"), hexcode: String::from("0x8a"), mingas: 3 });
        m.insert("8b", Opcode { name: String::from("DUP12"), hexcode: String::from("0x8b"), mingas: 3 });
        m.insert("8c", Opcode { name: String::from("DUP13"), hexcode: String::from("0x8c"), mingas: 3 });
        m.insert("8d", Opcode { name: String::from("DUP14"), hexcode: String::from("0x8d"), mingas: 3 });
        m.insert("8e", Opcode { name: String::from("DUP15"), hexcode: String::from("0x8e"), mingas: 3 });
        m.insert("8f", Opcode { name: String::from("DUP16"), hexcode: String::from("0x8f"), mingas: 3 });
        m.insert("90", Opcode { name: String::from("SWAP1"), hexcode: String::from("0x90"), mingas: 3 });
        m.insert("91", Opcode { name: String::from("SWAP2"), hexcode: String::from("0x91"), mingas: 3 });
        m.insert("92", Opcode { name: String::from("SWAP3"), hexcode: String::from("0x92"), mingas: 3 });
        m.insert("93", Opcode { name: String::from("SWAP4"), hexcode: String::from("0x93"), mingas: 3 });
        m.insert("94", Opcode { name: String::from("SWAP5"), hexcode: String::from("0x94"), mingas: 3 });
        m.insert("95", Opcode { name: String::from("SWAP6"), hexcode: String::from("0x95"), mingas: 3 });
        m.insert("96", Opcode { name: String::from("SWAP7"), hexcode: String::from("0x96"), mingas: 3 });
        m.insert("97", Opcode { name: String::from("SWAP8"), hexcode: String::from("0x97"), mingas: 3 });
        m.insert("98", Opcode { name: String::from("SWAP9"), hexcode: String::from("0x98"), mingas: 3 });
        m.insert("99", Opcode { name: String::from("SWAP10"), hexcode: String::from("0x99"), mingas: 3 });
        m.insert("9a", Opcode { name: String::from("SWAP11"), hexcode: String::from("0x9a"), mingas: 3 });
        m.insert("9b", Opcode { name: String::from("SWAP12"), hexcode: String::from("0x9b"), mingas: 3 });
        m.insert("9c", Opcode { name: String::from("SWAP13"), hexcode: String::from("0x9c"), mingas: 3 });
        m.insert("9d", Opcode { name: String::from("SWAP14"), hexcode: String::from("0x9d"), mingas: 3 });
        m.insert("9e", Opcode { name: String::from("SWAP15"), hexcode: String::from("0x9e"), mingas: 3 });
        m.insert("9f", Opcode { name: String::from("SWAP16"), hexcode: String::from("0x9f"), mingas: 3 });
        m.insert("a0", Opcode { name: String::from("LOG0"), hexcode: String::from("0xa0"), mingas: 375 });
        m.insert("a1", Opcode { name: String::from("LOG1"), hexcode: String::from("0xa1"), mingas: 750 });
        m.insert("a2", Opcode { name: String::from("LOG2"), hexcode: String::from("0xa2"), mingas: 1125 });
        m.insert("a3", Opcode { name: String::from("LOG3"), hexcode: String::from("0xa3"), mingas: 1500 });
        m.insert("a4", Opcode { name: String::from("LOG4"), hexcode: String::from("0xa4"), mingas: 1875 });
        m.insert("f0", Opcode { name: String::from("CREATE"), hexcode: String::from("0xf0"), mingas: 32000 });
        m.insert("f1", Opcode { name: String::from("CALL"), hexcode: String::from("0xf1"), mingas: 100 });
        m.insert("f2", Opcode { name: String::from("CALLCODE"), hexcode: String::from("0xf2"), mingas: 100 });
        m.insert("f3", Opcode { name: String::from("RETURN"), hexcode: String::from("0xf3"), mingas: 0 });
        m.insert("f4", Opcode { name: String::from("DELEGATECALL"), hexcode: String::from("0xf4"), mingas: 100 });
        m.insert("f5", Opcode { name: String::from("CREATE2"), hexcode: String::from("0xf5"), mingas: 32000 });
        m.insert("f6", Opcode { name: String::from("STATICCALL"), hexcode: String::from("0xf6"), mingas: 100 });
        m.insert("f7", Opcode { name: String::from("REVERT"), hexcode: String::from("0xf7"), mingas: 0 });
        m.insert("fe", Opcode { name: String::from("INVALID"), hexcode: String::from("0xfe"), mingas: 0 });
        m.insert("fa", Opcode { name: String::from("SELFDESTRUCT"), hexcode: String::from("0xfa"), mingas: 5000 });
        m
    };
}

// Returns the opcode for the given hexcode, fetched from the hashmap.
pub fn opcode(code: &str) -> Option<&Opcode> {
    return OPCODES.get(code);
}