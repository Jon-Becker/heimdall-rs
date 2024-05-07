use std::str::FromStr;

use ethers::types::U256;

use crate::{
    constants::{MEMLEN_REGEX, WORD_REGEX},
    ether::evm::core::opcodes::*,
    utils::strings::encode_hex_reduced,
};

pub fn is_ext_call_precompile(precompile_address: U256) -> bool {
    let address: usize = match precompile_address.try_into() {
        Ok(x) => x,
        Err(_) => usize::MAX,
    };

    matches!(address, 1..=3)
}

impl WrappedOpcode {
    /// Returns a WrappedOpcode's solidity representation.
    pub fn solidify(&self) -> String {
        let mut solidified_wrapped_opcode = String::new();

        match self.opcode.name {
            "ADD" => {
                solidified_wrapped_opcode.push_str(
                    format!("{} + {}", self.inputs[0]._solidify(), self.inputs[1]._solidify())
                        .as_str(),
                );
            }
            "MUL" => {
                solidified_wrapped_opcode.push_str(
                    format!("{} * {}", self.inputs[0]._solidify(), self.inputs[1]._solidify())
                        .as_str(),
                );
            }
            "SUB" => {
                solidified_wrapped_opcode.push_str(
                    format!("{} - {}", self.inputs[0]._solidify(), self.inputs[1]._solidify())
                        .as_str(),
                );
            }
            "DIV" => {
                solidified_wrapped_opcode.push_str(
                    format!("{} / {}", self.inputs[0]._solidify(), self.inputs[1]._solidify())
                        .as_str(),
                );
            }
            "SDIV" => {
                solidified_wrapped_opcode.push_str(
                    format!("{} / {}", self.inputs[0]._solidify(), self.inputs[1]._solidify())
                        .as_str(),
                );
            }
            "MOD" => {
                solidified_wrapped_opcode.push_str(
                    format!("{} % {}", self.inputs[0]._solidify(), self.inputs[1]._solidify())
                        .as_str(),
                );
            }
            "SMOD" => {
                solidified_wrapped_opcode.push_str(
                    format!("{} % {}", self.inputs[0]._solidify(), self.inputs[1]._solidify())
                        .as_str(),
                );
            }
            "ADDMOD" => {
                solidified_wrapped_opcode.push_str(
                    format!(
                        "{} + {} % {}",
                        self.inputs[0]._solidify(),
                        self.inputs[1]._solidify(),
                        self.inputs[2]._solidify()
                    )
                    .as_str(),
                );
            }
            "MULMOD" => {
                solidified_wrapped_opcode.push_str(
                    format!(
                        "({} * {}) % {}",
                        self.inputs[0]._solidify(),
                        self.inputs[1]._solidify(),
                        self.inputs[2]._solidify()
                    )
                    .as_str(),
                );
            }
            "EXP" => {
                solidified_wrapped_opcode.push_str(
                    format!("{} ** {}", self.inputs[0]._solidify(), self.inputs[1]._solidify())
                        .as_str(),
                );
            }
            "LT" => {
                solidified_wrapped_opcode.push_str(
                    format!("{} < {}", self.inputs[0]._solidify(), self.inputs[1]._solidify())
                        .as_str(),
                );
            }
            "GT" => {
                solidified_wrapped_opcode.push_str(
                    format!("{} > {}", self.inputs[0]._solidify(), self.inputs[1]._solidify())
                        .as_str(),
                );
            }
            "SLT" => {
                solidified_wrapped_opcode.push_str(
                    format!("{} < {}", self.inputs[0]._solidify(), self.inputs[1]._solidify())
                        .as_str(),
                );
            }
            "SGT" => {
                solidified_wrapped_opcode.push_str(
                    format!("{} > {}", self.inputs[0]._solidify(), self.inputs[1]._solidify())
                        .as_str(),
                );
            }
            "EQ" => {
                solidified_wrapped_opcode.push_str(
                    format!("{} == {}", self.inputs[0]._solidify(), self.inputs[1]._solidify())
                        .as_str(),
                );
            }
            "ISZERO" => {
                let solidified_input = self.inputs[0]._solidify();

                match solidified_input.contains(' ') {
                    true => {
                        solidified_wrapped_opcode
                            .push_str(format!("!({})", self.inputs[0]._solidify()).as_str());
                    }
                    false => {
                        solidified_wrapped_opcode
                            .push_str(format!("!{}", self.inputs[0]._solidify()).as_str());
                    }
                }
            }
            "AND" => {
                solidified_wrapped_opcode.push_str(
                    format!("({}) & ({})", self.inputs[0]._solidify(), self.inputs[1]._solidify())
                        .as_str(),
                );
            }
            "OR" => {
                solidified_wrapped_opcode.push_str(
                    format!("{} | {}", self.inputs[0]._solidify(), self.inputs[1]._solidify())
                        .as_str(),
                );
            }
            "XOR" => {
                solidified_wrapped_opcode.push_str(
                    format!("{} ^ {}", self.inputs[0]._solidify(), self.inputs[1]._solidify())
                        .as_str(),
                );
            }
            "NOT" => {
                solidified_wrapped_opcode
                    .push_str(format!("~({})", self.inputs[0]._solidify()).as_str());
            }
            "SHL" => {
                solidified_wrapped_opcode.push_str(
                    format!("{} << {}", self.inputs[1]._solidify(), self.inputs[0]._solidify())
                        .as_str(),
                );
            }
            "SHR" => {
                solidified_wrapped_opcode.push_str(
                    format!("{} >> {}", self.inputs[1]._solidify(), self.inputs[0]._solidify())
                        .as_str(),
                );
            }
            "SAR" => {
                solidified_wrapped_opcode.push_str(
                    format!("{} >> {}", self.inputs[1]._solidify(), self.inputs[0]._solidify())
                        .as_str(),
                );
            }
            "BYTE" => {
                solidified_wrapped_opcode.push_str(self.inputs[1]._solidify().as_str());
            }
            "SHA3" => {
                solidified_wrapped_opcode
                    .push_str(&format!("keccak256(memory[{}])", self.inputs[0]._solidify()));
            }
            "ADDRESS" => {
                solidified_wrapped_opcode.push_str("address(this)");
            }
            "BALANCE" => {
                solidified_wrapped_opcode
                    .push_str(format!("address({}).balance", self.inputs[0]._solidify()).as_str());
            }
            "ORIGIN" => {
                solidified_wrapped_opcode.push_str("tx.origin");
            }
            "CALLER" => {
                solidified_wrapped_opcode.push_str("msg.sender");
            }
            "CALLVALUE" => {
                solidified_wrapped_opcode.push_str("msg.value");
            }
            "CALLDATALOAD" => {
                let solidified_slot = self.inputs[0]._solidify();

                // are dealing with a slot that is a constant, we can just use the slot directly
                if WORD_REGEX.is_match(&solidified_slot).unwrap_or(false) {
                    // convert to usize
                    match usize::from_str_radix(&solidified_slot.replacen("0x", "", 1), 16) {
                        Ok(slot) => {
                            solidified_wrapped_opcode
                                .push_str(format!("arg{}", (slot - 4) / 32).as_str());
                        }
                        Err(_) => {
                            if solidified_slot.contains("0x04 + ") ||
                                solidified_slot.contains("+ 0x04")
                            {
                                solidified_wrapped_opcode.push_str(
                                    solidified_slot
                                        .replace("0x04 + ", "")
                                        .replace("+ 0x04", "")
                                        .as_str(),
                                );
                            } else {
                                solidified_wrapped_opcode
                                    .push_str(format!("msg.data[{solidified_slot}]").as_str());
                            }
                        }
                    };
                } else {
                    solidified_wrapped_opcode
                        .push_str(format!("msg.data[{solidified_slot}]").as_str());
                }
            }
            "CALLDATASIZE" => {
                solidified_wrapped_opcode.push_str("msg.data.length");
            }
            "CODESIZE" => {
                solidified_wrapped_opcode.push_str("this.code.length");
            }
            "EXTCODESIZE" => {
                solidified_wrapped_opcode.push_str(
                    format!("address({}).code.length", self.inputs[0]._solidify()).as_str(),
                );
            }
            "EXTCODEHASH" => {
                solidified_wrapped_opcode
                    .push_str(format!("address({}).codehash", self.inputs[0]._solidify()).as_str());
            }
            "BLOCKHASH" => {
                solidified_wrapped_opcode
                    .push_str(format!("blockhash({})", self.inputs[0]._solidify()).as_str());
            }
            "COINBASE" => {
                solidified_wrapped_opcode.push_str("block.coinbase");
            }
            "TIMESTAMP" => {
                solidified_wrapped_opcode.push_str("block.timestamp");
            }
            "NUMBER" => {
                solidified_wrapped_opcode.push_str("block.number");
            }
            "DIFFICULTY" => {
                solidified_wrapped_opcode.push_str("block.difficulty");
            }
            "GASLIMIT" => {
                solidified_wrapped_opcode.push_str("block.gaslimit");
            }
            "CHAINID" => {
                solidified_wrapped_opcode.push_str("block.chainid");
            }
            "SELFBALANCE" => {
                solidified_wrapped_opcode.push_str("address(this).balance");
            }
            "BASEFEE" => {
                solidified_wrapped_opcode.push_str("block.basefee");
            }
            "GAS" => {
                solidified_wrapped_opcode.push_str("gasleft()");
            }
            "GASPRICE" => {
                solidified_wrapped_opcode.push_str("tx.gasprice");
            }
            "SLOAD" => {
                solidified_wrapped_opcode
                    .push_str(format!("storage[{}]", self.inputs[0]._solidify()).as_str());
            }
            "TLOAD" => {
                solidified_wrapped_opcode
                    .push_str(format!("transient[{}]", self.inputs[0]._solidify()).as_str());
            }
            "MLOAD" => {
                let memloc = self.inputs[0]._solidify();
                if memloc.contains("memory") {
                    match MEMLEN_REGEX.find(&format!("memory[{memloc}]")).unwrap_or(None) {
                        Some(_) => {
                            solidified_wrapped_opcode.push_str(format!("{memloc}.length").as_str());
                        }
                        None => {
                            solidified_wrapped_opcode
                                .push_str(format!("memory[{memloc}]").as_str());
                        }
                    }
                } else {
                    solidified_wrapped_opcode.push_str(format!("memory[{memloc}]").as_str());
                }
            }
            "MSIZE" => {
                solidified_wrapped_opcode.push_str("memory.length");
            }
            "CALL" => {
                match U256::from_str(&self.inputs[1]._solidify()) {
                    Ok(addr) => {
                        if is_ext_call_precompile(addr) {
                            solidified_wrapped_opcode
                                .push_str(&format!("memory[{}]", self.inputs[5]._solidify()));
                        } else {
                            solidified_wrapped_opcode.push_str("success");
                        }
                    }
                    Err(_) => {
                        solidified_wrapped_opcode.push_str("success");
                    }
                };
            }
            "CALLCODE" => {
                match U256::from_str(&self.inputs[1]._solidify()) {
                    Ok(addr) => {
                        if is_ext_call_precompile(addr) {
                            solidified_wrapped_opcode
                                .push_str(&format!("memory[{}]", self.inputs[5]._solidify()));
                        } else {
                            solidified_wrapped_opcode.push_str("success");
                        }
                    }
                    Err(_) => {
                        solidified_wrapped_opcode.push_str("success");
                    }
                };
            }
            "DELEGATECALL" => {
                match U256::from_str(&self.inputs[1]._solidify()) {
                    Ok(addr) => {
                        if is_ext_call_precompile(addr) {
                            solidified_wrapped_opcode
                                .push_str(&format!("memory[{}]", self.inputs[5]._solidify()));
                        } else {
                            solidified_wrapped_opcode.push_str("success");
                        }
                    }
                    Err(_) => {
                        solidified_wrapped_opcode.push_str("success");
                    }
                };
            }
            "STATICCALL" => {
                match U256::from_str(&self.inputs[1]._solidify()) {
                    Ok(addr) => {
                        if is_ext_call_precompile(addr) {
                            solidified_wrapped_opcode
                                .push_str(&format!("memory[{}]", self.inputs[5]._solidify()));
                        } else {
                            solidified_wrapped_opcode.push_str("success");
                        }
                    }
                    Err(_) => {
                        solidified_wrapped_opcode.push_str("success");
                    }
                };
            }
            "RETURNDATASIZE" => {
                solidified_wrapped_opcode.push_str("ret0.length");
            }
            "PUSH0" => {
                solidified_wrapped_opcode.push('0');
            }
            opcode => {
                if opcode.starts_with("PUSH") {
                    solidified_wrapped_opcode.push_str(self.inputs[0]._solidify().as_str());
                } else {
                    solidified_wrapped_opcode.push_str(opcode.to_string().as_str());
                }
            }
        }

        solidified_wrapped_opcode
    }

    /// creates a new WrappedOpcode from a set of raw inputs
    pub fn new(opcode_int: u8, inputs: Vec<WrappedInput>) -> WrappedOpcode {
        WrappedOpcode { opcode: Opcode::new(opcode_int), inputs }
    }
}

impl Default for WrappedOpcode {
    fn default() -> Self {
        WrappedOpcode {
            opcode: Opcode { code: 0, name: "unknown", mingas: 0, inputs: 0, outputs: 0 },
            inputs: Vec::new(),
        }
    }
}

impl WrappedInput {
    /// Returns a WrappedInput's solidity representation.
    pub fn _solidify(&self) -> String {
        let mut solidified_wrapped_input = String::new();

        match self {
            WrappedInput::Raw(u256) => {
                solidified_wrapped_input.push_str(&encode_hex_reduced(*u256));
            }
            WrappedInput::Opcode(opcode) => {
                let solidified_opcode = opcode.solidify();

                if solidified_opcode.contains(' ') {
                    solidified_wrapped_input.push_str(format!("({solidified_opcode})").as_str());
                } else {
                    solidified_wrapped_input.push_str(solidified_opcode.as_str());
                }
            }
        }

        solidified_wrapped_input
    }
}

#[cfg(test)]
mod tests {
    use crate::ether::{
        evm::core::opcodes::{Opcode, WrappedInput, WrappedOpcode},
        lexers::solidity::is_ext_call_precompile,
    };
    use ethers::types::U256;

    #[test]
    fn test_is_ext_call_precompile() {
        assert!(is_ext_call_precompile(U256::from(1)));
        assert!(is_ext_call_precompile(U256::from(2)));
        assert!(is_ext_call_precompile(U256::from(3)));
        assert!(!is_ext_call_precompile(U256::from(4)));
        assert!(!is_ext_call_precompile(U256::MAX));
    }

    #[test]
    fn test_wrapped_opcode_solidify_add() {
        let opcode = Opcode { code: 0x01, name: "ADD", mingas: 1, inputs: 2, outputs: 1 };
        let inputs = vec![WrappedInput::Raw(U256::from(1u8)), WrappedInput::Raw(U256::from(2u8))];
        let wrapped_opcode = WrappedOpcode { opcode, inputs };

        assert_eq!(wrapped_opcode.solidify(), "0x01 + 0x02");
    }

    #[test]
    fn test_wrapped_opcode_solidify_mul() {
        let opcode = Opcode { code: 0x02, name: "MUL", mingas: 1, inputs: 2, outputs: 1 };
        let inputs = vec![WrappedInput::Raw(U256::from(2u8)), WrappedInput::Raw(U256::from(3u8))];
        let wrapped_opcode = WrappedOpcode { opcode, inputs };

        assert_eq!(wrapped_opcode.solidify(), "0x02 * 0x03");
    }

    #[test]
    fn test_wrapped_opcode_solidify_sub() {
        let opcode = Opcode { code: 0x03, name: "SUB", mingas: 1, inputs: 2, outputs: 1 };
        let inputs = vec![WrappedInput::Raw(U256::from(5u8)), WrappedInput::Raw(U256::from(3u8))];
        let wrapped_opcode = WrappedOpcode { opcode, inputs };

        assert_eq!(wrapped_opcode.solidify(), "0x05 - 0x03");
    }

    #[test]
    fn test_wrapped_opcode_solidify_div() {
        let opcode = Opcode { code: 0x04, name: "DIV", mingas: 1, inputs: 2, outputs: 1 };
        let inputs = vec![WrappedInput::Raw(U256::from(10u8)), WrappedInput::Raw(U256::from(2u8))];
        let wrapped_opcode = WrappedOpcode { opcode, inputs };

        assert_eq!(wrapped_opcode.solidify(), "0x0a / 0x02");
    }

    #[test]
    fn test_wrapped_opcode_solidify_sdiv() {
        let opcode = Opcode { code: 0x05, name: "SDIV", mingas: 1, inputs: 2, outputs: 1 };
        let inputs = vec![WrappedInput::Raw(U256::from(10u8)), WrappedInput::Raw(U256::from(2u8))];
        let wrapped_opcode = WrappedOpcode { opcode, inputs };

        assert_eq!(wrapped_opcode.solidify(), "0x0a / 0x02");
    }

    #[test]
    fn test_wrapped_opcode_solidify_mod() {
        let opcode = Opcode { code: 0x06, name: "MOD", mingas: 1, inputs: 2, outputs: 1 };
        let inputs = vec![WrappedInput::Raw(U256::from(10u8)), WrappedInput::Raw(U256::from(3u8))];
        let wrapped_opcode = WrappedOpcode { opcode, inputs };

        assert_eq!(wrapped_opcode.solidify(), "0x0a % 0x03");
    }

    #[test]
    fn test_wrapped_opcode_solidify_smod() {
        let opcode = Opcode { code: 0x07, name: "SMOD", mingas: 1, inputs: 2, outputs: 1 };
        let inputs = vec![WrappedInput::Raw(U256::from(10u8)), WrappedInput::Raw(U256::from(3u8))];
        let wrapped_opcode = WrappedOpcode { opcode, inputs };

        assert_eq!(wrapped_opcode.solidify(), "0x0a % 0x03");
    }

    #[test]
    fn test_wrapped_opcode_solidify_addmod() {
        let opcode = Opcode { code: 0x08, name: "ADDMOD", mingas: 1, inputs: 3, outputs: 1 };
        let inputs = vec![
            WrappedInput::Raw(U256::from(3u8)),
            WrappedInput::Raw(U256::from(4u8)),
            WrappedInput::Raw(U256::from(5u8)),
        ];
        let wrapped_opcode = WrappedOpcode { opcode, inputs };

        assert_eq!(wrapped_opcode.solidify(), "0x03 + 0x04 % 0x05");
    }

    #[test]
    fn test_wrapped_opcode_solidify_mulmod() {
        let opcode = Opcode { code: 0x09, name: "MULMOD", mingas: 1, inputs: 3, outputs: 1 };
        let inputs = vec![
            WrappedInput::Raw(U256::from(3u8)),
            WrappedInput::Raw(U256::from(4u8)),
            WrappedInput::Raw(U256::from(5u8)),
        ];
        let wrapped_opcode = WrappedOpcode { opcode, inputs };

        assert_eq!(wrapped_opcode.solidify(), "(0x03 * 0x04) % 0x05");
    }

    #[test]
    fn test_wrapped_opcode_solidify_exp() {
        let opcode = Opcode { code: 0x0a, name: "EXP", mingas: 1, inputs: 2, outputs: 1 };
        let inputs = vec![WrappedInput::Raw(U256::from(2u8)), WrappedInput::Raw(U256::from(3u8))];
        let wrapped_opcode = WrappedOpcode { opcode, inputs };

        assert_eq!(wrapped_opcode.solidify(), "0x02 ** 0x03");
    }

    #[test]
    fn test_wrapped_opcode_solidify_lt() {
        let opcode = Opcode { code: 0x10, name: "LT", mingas: 1, inputs: 2, outputs: 1 };
        let inputs = vec![WrappedInput::Raw(U256::from(2u8)), WrappedInput::Raw(U256::from(3u8))];
        let wrapped_opcode = WrappedOpcode { opcode, inputs };

        assert_eq!(wrapped_opcode.solidify(), "0x02 < 0x03");
    }

    #[test]
    fn test_wrapped_opcode_solidify_gt() {
        let opcode = Opcode { code: 0x11, name: "GT", mingas: 1, inputs: 2, outputs: 1 };
        let inputs = vec![WrappedInput::Raw(U256::from(5u8)), WrappedInput::Raw(U256::from(3u8))];
        let wrapped_opcode = WrappedOpcode { opcode, inputs };

        assert_eq!(wrapped_opcode.solidify(), "0x05 > 0x03");
    }

    #[test]
    fn test_wrapped_opcode_solidify_slt() {
        let opcode = Opcode { code: 0x12, name: "SLT", mingas: 1, inputs: 2, outputs: 1 };
        let inputs = vec![WrappedInput::Raw(U256::from(2u8)), WrappedInput::Raw(U256::from(3u8))];
        let wrapped_opcode = WrappedOpcode { opcode, inputs };

        assert_eq!(wrapped_opcode.solidify(), "0x02 < 0x03");
    }

    #[test]
    fn test_wrapped_opcode_solidify_sgt() {
        let opcode = Opcode { code: 0x13, name: "SGT", mingas: 1, inputs: 2, outputs: 1 };
        let inputs = vec![WrappedInput::Raw(U256::from(5u8)), WrappedInput::Raw(U256::from(3u8))];
        let wrapped_opcode = WrappedOpcode { opcode, inputs };

        assert_eq!(wrapped_opcode.solidify(), "0x05 > 0x03");
    }

    #[test]
    fn test_wrapped_opcode_solidify_eq() {
        let opcode = Opcode { code: 0x14, name: "EQ", mingas: 1, inputs: 2, outputs: 1 };
        let inputs = vec![WrappedInput::Raw(U256::from(5u8)), WrappedInput::Raw(U256::from(5u8))];
        let wrapped_opcode = WrappedOpcode { opcode, inputs };

        assert_eq!(wrapped_opcode.solidify(), "0x05 == 0x05");
    }

    #[test]
    fn test_wrapped_opcode_solidify_iszero() {
        let opcode = Opcode { code: 0x15, name: "ISZERO", mingas: 1, inputs: 1, outputs: 1 };
        let inputs = vec![WrappedInput::Raw(U256::from(0u8))];
        let wrapped_opcode = WrappedOpcode { opcode, inputs };

        assert_eq!(wrapped_opcode.solidify(), "!0");
    }

    #[test]
    fn test_wrapped_opcode_solidify_and() {
        let opcode = Opcode { code: 0x16, name: "AND", mingas: 1, inputs: 2, outputs: 1 };
        let inputs =
            vec![WrappedInput::Raw(U256::from(0b1010u8)), WrappedInput::Raw(U256::from(0b1100u8))];
        let wrapped_opcode = WrappedOpcode { opcode, inputs };

        assert_eq!(wrapped_opcode.solidify(), "(0x0a) & (0x0c)");
    }

    #[test]
    fn test_wrapped_opcode_solidify_or() {
        let opcode = Opcode { code: 0x17, name: "OR", mingas: 1, inputs: 2, outputs: 1 };
        let inputs =
            vec![WrappedInput::Raw(U256::from(0b1010u8)), WrappedInput::Raw(U256::from(0b1100u8))];
        let wrapped_opcode = WrappedOpcode { opcode, inputs };

        assert_eq!(wrapped_opcode.solidify(), "0x0a | 0x0c");
    }

    #[test]
    fn test_wrapped_opcode_solidify_xor() {
        let opcode = Opcode { code: 0x18, name: "XOR", mingas: 1, inputs: 2, outputs: 1 };
        let inputs =
            vec![WrappedInput::Raw(U256::from(0b1010u8)), WrappedInput::Raw(U256::from(0b1100u8))];
        let wrapped_opcode = WrappedOpcode { opcode, inputs };

        assert_eq!(wrapped_opcode.solidify(), "0x0a ^ 0x0c");
    }

    #[test]
    fn test_wrapped_opcode_solidify_not() {
        let opcode = Opcode { code: 0x19, name: "NOT", mingas: 1, inputs: 1, outputs: 1 };
        let inputs = vec![WrappedInput::Raw(U256::from(0b1010u8))];
        let wrapped_opcode = WrappedOpcode { opcode, inputs };

        assert_eq!(wrapped_opcode.solidify(), "~(0x0a)");
    }

    #[test]
    fn test_wrapped_opcode_solidify_shl() {
        let opcode = Opcode { code: 0x1a, name: "SHL", mingas: 1, inputs: 2, outputs: 1 };
        let inputs = vec![WrappedInput::Raw(U256::from(3u8)), WrappedInput::Raw(U256::from(1u8))];
        let wrapped_opcode = WrappedOpcode { opcode, inputs };

        assert_eq!(wrapped_opcode.solidify(), "0x01 << 0x03");
    }

    #[test]
    fn test_wrapped_opcode_solidify_shr() {
        let opcode = Opcode { code: 0x1b, name: "SHR", mingas: 1, inputs: 2, outputs: 1 };
        let inputs = vec![WrappedInput::Raw(U256::from(6u8)), WrappedInput::Raw(U256::from(1u8))];
        let wrapped_opcode = WrappedOpcode { opcode, inputs };

        assert_eq!(wrapped_opcode.solidify(), "0x01 >> 0x06");
    }

    #[test]
    fn test_wrapped_opcode_solidify_sar() {
        let opcode = Opcode { code: 0x1c, name: "SAR", mingas: 1, inputs: 2, outputs: 1 };
        let inputs = vec![WrappedInput::Raw(U256::from(6u8)), WrappedInput::Raw(U256::from(1u8))];
        let wrapped_opcode = WrappedOpcode { opcode, inputs };

        assert_eq!(wrapped_opcode.solidify(), "0x01 >> 0x06");
    }

    #[test]
    fn test_wrapped_opcode_solidify_byte() {
        let opcode = Opcode { code: 0x1d, name: "BYTE", mingas: 1, inputs: 2, outputs: 1 };
        let inputs =
            vec![WrappedInput::Raw(U256::from(3u8)), WrappedInput::Raw(U256::from(0x12345678u32))];
        let wrapped_opcode = WrappedOpcode { opcode, inputs };

        assert_eq!(wrapped_opcode.solidify(), "0x12345678");
    }

    #[test]
    fn test_wrapped_opcode_solidify_sha3() {
        let opcode = Opcode { code: 0x20, name: "SHA3", mingas: 1, inputs: 1, outputs: 1 };
        let inputs = vec![WrappedInput::Raw(U256::from(0u8))];
        let wrapped_opcode = WrappedOpcode { opcode, inputs };

        assert_eq!(wrapped_opcode.solidify(), "keccak256(memory[0])");
    }

    #[test]
    fn test_wrapped_opcode_solidify_address() {
        let opcode = Opcode { code: 0x30, name: "ADDRESS", mingas: 1, inputs: 0, outputs: 1 };
        let inputs = vec![];
        let wrapped_opcode = WrappedOpcode { opcode, inputs };

        assert_eq!(wrapped_opcode.solidify(), "address(this)");
    }

    #[test]
    fn test_wrapped_opcode_solidify_balance() {
        let opcode = Opcode { code: 0x31, name: "BALANCE", mingas: 1, inputs: 1, outputs: 1 };
        let inputs = vec![WrappedInput::Raw(U256::from(0x1234u16))];
        let wrapped_opcode = WrappedOpcode { opcode, inputs };

        assert_eq!(wrapped_opcode.solidify(), "address(0x1234).balance");
    }

    #[test]
    fn test_wrapped_opcode_solidify_origin() {
        let opcode = Opcode { code: 0x32, name: "ORIGIN", mingas: 1, inputs: 0, outputs: 1 };
        let inputs = vec![];
        let wrapped_opcode = WrappedOpcode { opcode, inputs };

        assert_eq!(wrapped_opcode.solidify(), "tx.origin");
    }

    #[test]
    fn test_wrapped_opcode_solidify_caller() {
        let opcode = Opcode { code: 0x33, name: "CALLER", mingas: 1, inputs: 0, outputs: 1 };
        let inputs = vec![];
        let wrapped_opcode = WrappedOpcode { opcode, inputs };

        assert_eq!(wrapped_opcode.solidify(), "msg.sender");
    }

    #[test]
    fn test_wrapped_opcode_solidify_callvalue() {
        let opcode = Opcode { code: 0x34, name: "CALLVALUE", mingas: 1, inputs: 0, outputs: 1 };
        let inputs = vec![];
        let wrapped_opcode = WrappedOpcode { opcode, inputs };

        assert_eq!(wrapped_opcode.solidify(), "msg.value");
    }

    #[test]
    fn test_wrapped_opcode_solidify_calldataload() {
        let opcode = Opcode { code: 0x35, name: "CALLDATALOAD", mingas: 1, inputs: 1, outputs: 1 };
        let inputs = vec![WrappedInput::Raw(U256::from(0x1234u16))];
        let wrapped_opcode = WrappedOpcode { opcode, inputs };

        assert_eq!(wrapped_opcode.solidify(), "arg145");
    }

    #[test]
    fn test_wrapped_opcode_solidify_calldatasize() {
        let opcode = Opcode { code: 0x36, name: "CALLDATASIZE", mingas: 1, inputs: 0, outputs: 1 };
        let inputs = vec![];
        let wrapped_opcode = WrappedOpcode { opcode, inputs };

        assert_eq!(wrapped_opcode.solidify(), "msg.data.length");
    }

    #[test]
    fn test_wrapped_opcode_solidify_codesize() {
        let opcode = Opcode { code: 0x38, name: "CODESIZE", mingas: 1, inputs: 0, outputs: 1 };
        let inputs = vec![];
        let wrapped_opcode = WrappedOpcode { opcode, inputs };

        assert_eq!(wrapped_opcode.solidify(), "this.code.length");
    }

    #[test]
    fn test_wrapped_opcode_solidify_extcodesize() {
        let opcode = Opcode { code: 0x3b, name: "EXTCODESIZE", mingas: 1, inputs: 1, outputs: 1 };
        let inputs = vec![WrappedInput::Raw(U256::from(0x1234u16))];
        let wrapped_opcode = WrappedOpcode { opcode, inputs };

        assert_eq!(wrapped_opcode.solidify(), "address(0x1234).code.length");
    }

    #[test]
    fn test_wrapped_opcode_solidify_extcodehash() {
        let opcode = Opcode { code: 0x3f, name: "EXTCODEHASH", mingas: 1, inputs: 1, outputs: 1 };
        let inputs = vec![WrappedInput::Raw(U256::from(0x1234u16))];
        let wrapped_opcode = WrappedOpcode { opcode, inputs };

        assert_eq!(wrapped_opcode.solidify(), "address(0x1234).codehash");
    }

    #[test]
    fn test_wrapped_opcode_solidify_blockhash() {
        let opcode = Opcode { code: 0x40, name: "BLOCKHASH", mingas: 1, inputs: 1, outputs: 1 };
        let inputs = vec![WrappedInput::Raw(U256::from(0x1234u16))];
        let wrapped_opcode = WrappedOpcode { opcode, inputs };

        assert_eq!(wrapped_opcode.solidify(), "blockhash(0x1234)");
    }

    #[test]
    fn test_wrapped_opcode_solidify_coinbase() {
        let opcode = Opcode { code: 0x41, name: "COINBASE", mingas: 1, inputs: 0, outputs: 1 };
        let inputs = vec![];
        let wrapped_opcode = WrappedOpcode { opcode, inputs };

        assert_eq!(wrapped_opcode.solidify(), "block.coinbase");
    }

    #[test]
    fn test_wrapped_opcode_solidify_timestamp() {
        let opcode = Opcode { code: 0x42, name: "TIMESTAMP", mingas: 1, inputs: 0, outputs: 1 };
        let inputs = vec![];
        let wrapped_opcode = WrappedOpcode { opcode, inputs };

        assert_eq!(wrapped_opcode.solidify(), "block.timestamp");
    }

    #[test]
    fn test_wrapped_opcode_solidify_number() {
        let opcode = Opcode { code: 0x43, name: "NUMBER", mingas: 1, inputs: 0, outputs: 1 };
        let inputs = vec![];
        let wrapped_opcode = WrappedOpcode { opcode, inputs };

        assert_eq!(wrapped_opcode.solidify(), "block.number");
    }

    #[test]
    fn test_wrapped_opcode_solidify_difficulty() {
        let opcode = Opcode { code: 0x44, name: "DIFFICULTY", mingas: 1, inputs: 0, outputs: 1 };
        let inputs = vec![];
        let wrapped_opcode = WrappedOpcode { opcode, inputs };

        assert_eq!(wrapped_opcode.solidify(), "block.difficulty");
    }

    #[test]
    fn test_wrapped_opcode_solidify_gaslimit() {
        let opcode = Opcode { code: 0x45, name: "GASLIMIT", mingas: 1, inputs: 0, outputs: 1 };
        let inputs = vec![];
        let wrapped_opcode = WrappedOpcode { opcode, inputs };

        assert_eq!(wrapped_opcode.solidify(), "block.gaslimit");
    }

    #[test]
    fn test_wrapped_opcode_solidify_chainid() {
        let opcode = Opcode { code: 0x46, name: "CHAINID", mingas: 1, inputs: 0, outputs: 1 };
        let inputs = vec![];
        let wrapped_opcode = WrappedOpcode { opcode, inputs };

        assert_eq!(wrapped_opcode.solidify(), "block.chainid");
    }

    #[test]
    fn test_wrapped_opcode_solidify_selfbalance() {
        let opcode = Opcode { code: 0x47, name: "SELFBALANCE", mingas: 1, inputs: 0, outputs: 1 };
        let inputs = vec![];
        let wrapped_opcode = WrappedOpcode { opcode, inputs };

        assert_eq!(wrapped_opcode.solidify(), "address(this).balance");
    }

    #[test]
    fn test_wrapped_opcode_solidify_basefee() {
        let opcode = Opcode { code: 0x48, name: "BASEFEE", mingas: 1, inputs: 0, outputs: 1 };
        let inputs = vec![];
        let wrapped_opcode = WrappedOpcode { opcode, inputs };

        assert_eq!(wrapped_opcode.solidify(), "block.basefee");
    }

    #[test]
    fn test_wrapped_opcode_solidify_gas() {
        let opcode = Opcode { code: 0x5a, name: "GAS", mingas: 1, inputs: 0, outputs: 1 };
        let inputs = vec![];
        let wrapped_opcode = WrappedOpcode { opcode, inputs };

        assert_eq!(wrapped_opcode.solidify(), "gasleft()");
    }

    #[test]
    fn test_wrapped_opcode_solidify_gasprice() {
        let opcode = Opcode { code: 0x3a, name: "GASPRICE", mingas: 1, inputs: 0, outputs: 1 };
        let inputs = vec![];
        let wrapped_opcode = WrappedOpcode { opcode, inputs };

        assert_eq!(wrapped_opcode.solidify(), "tx.gasprice");
    }

    #[test]
    fn test_wrapped_opcode_solidify_sload() {
        let opcode = Opcode { code: 0x54, name: "SLOAD", mingas: 1, inputs: 1, outputs: 1 };
        let inputs = vec![WrappedInput::Raw(U256::from(0x1234u16))];
        let wrapped_opcode = WrappedOpcode { opcode, inputs };

        assert_eq!(wrapped_opcode.solidify(), "storage[0x1234]");
    }

    #[test]
    fn test_wrapped_opcode_solidify_mload() {
        let opcode = Opcode { code: 0x51, name: "MLOAD", mingas: 1, inputs: 1, outputs: 1 };
        let inputs = vec![WrappedInput::Raw(U256::from(0x1234u16))];
        let wrapped_opcode = WrappedOpcode { opcode, inputs };

        assert_eq!(wrapped_opcode.solidify(), "memory[0x1234]");
    }

    #[test]
    fn test_wrapped_opcode_solidify_msize() {
        let opcode = Opcode { code: 0x59, name: "MSIZE", mingas: 1, inputs: 0, outputs: 1 };
        let inputs = vec![];
        let wrapped_opcode = WrappedOpcode { opcode, inputs };

        assert_eq!(wrapped_opcode.solidify(), "memory.length");
    }

    #[test]
    fn test_wrapped_opcode_solidify_call() {
        let opcode = Opcode { code: 0xf1, name: "CALL", mingas: 1, inputs: 7, outputs: 1 };
        let inputs = vec![
            WrappedInput::Raw(U256::from(0x1234u16)),
            WrappedInput::Raw(U256::from(0x01u8)),
            WrappedInput::Raw(U256::from(0x02u8)),
            WrappedInput::Raw(U256::from(0x03u8)),
            WrappedInput::Raw(U256::from(0x04u8)),
            WrappedInput::Raw(U256::from(0x05u8)),
            WrappedInput::Raw(U256::from(0x06u8)),
        ];
        let wrapped_opcode = WrappedOpcode { opcode, inputs };

        assert_eq!(wrapped_opcode.solidify(), "memory[0x05]");
    }

    #[test]
    fn test_wrapped_opcode_solidify_callcode() {
        let opcode = Opcode { code: 0xf2, name: "CALLCODE", mingas: 1, inputs: 7, outputs: 1 };
        let inputs = vec![
            WrappedInput::Raw(U256::from(0x1234u16)),
            WrappedInput::Raw(U256::from(0x01u8)),
            WrappedInput::Raw(U256::from(0x02u8)),
            WrappedInput::Raw(U256::from(0x03u8)),
            WrappedInput::Raw(U256::from(0x04u8)),
            WrappedInput::Raw(U256::from(0x05u8)),
            WrappedInput::Raw(U256::from(0x06u8)),
        ];
        let wrapped_opcode = WrappedOpcode { opcode, inputs };

        assert_eq!(wrapped_opcode.solidify(), "memory[0x05]");
    }

    #[test]
    fn test_wrapped_opcode_solidify_delegatecall() {
        let opcode = Opcode { code: 0xf4, name: "DELEGATECALL", mingas: 1, inputs: 6, outputs: 1 };
        let inputs = vec![
            WrappedInput::Raw(U256::from(0x1234u16)),
            WrappedInput::Raw(U256::from(0x01u8)),
            WrappedInput::Raw(U256::from(0x02u8)),
            WrappedInput::Raw(U256::from(0x03u8)),
            WrappedInput::Raw(U256::from(0x04u8)),
            WrappedInput::Raw(U256::from(0x05u8)),
        ];
        let wrapped_opcode = WrappedOpcode { opcode, inputs };

        assert_eq!(wrapped_opcode.solidify(), "memory[0x05]");
    }

    #[test]
    fn test_wrapped_opcode_solidify_staticcall() {
        let opcode = Opcode { code: 0xfa, name: "STATICCALL", mingas: 1, inputs: 6, outputs: 1 };
        let inputs = vec![
            WrappedInput::Raw(U256::from(0x1234u16)),
            WrappedInput::Raw(U256::from(0x01u8)),
            WrappedInput::Raw(U256::from(0x02u8)),
            WrappedInput::Raw(U256::from(0x03u8)),
            WrappedInput::Raw(U256::from(0x04u8)),
            WrappedInput::Raw(U256::from(0x05u8)),
        ];
        let wrapped_opcode = WrappedOpcode { opcode, inputs };

        assert_eq!(wrapped_opcode.solidify(), "memory[0x05]");
    }

    #[test]
    fn test_wrapped_opcode_solidify_returndatasize() {
        let opcode =
            Opcode { code: 0x3d, name: "RETURNDATASIZE", mingas: 1, inputs: 0, outputs: 1 };
        let inputs = vec![];
        let wrapped_opcode = WrappedOpcode { opcode, inputs };

        assert_eq!(wrapped_opcode.solidify(), "ret0.length");
    }

    #[test]
    fn test_wrapped_opcode_solidify_push() {
        let opcode = Opcode { code: 0x5f, name: "PUSH0", mingas: 1, inputs: 0, outputs: 1 };
        let inputs = vec![];
        let wrapped_opcode = WrappedOpcode { opcode, inputs };

        assert_eq!(wrapped_opcode.solidify(), "0");
    }

    #[test]
    fn test_wrapped_opcode_solidify_unknown() {
        let opcode = Opcode { code: 0xff, name: "unknown", mingas: 1, inputs: 0, outputs: 0 };
        let inputs = vec![];
        let wrapped_opcode = WrappedOpcode { opcode, inputs };

        assert_eq!(wrapped_opcode.solidify(), "unknown");
    }
}
