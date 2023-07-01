#[cfg(test)]
mod test_solidity {
    use crate::ether::{
        evm::opcodes::{Opcode, WrappedInput, WrappedOpcode},
        solidity::is_ext_call_precompile,
    };
    use ethers::types::U256;

    #[test]
    fn test_is_ext_call_precompile() {
        assert_eq!(is_ext_call_precompile(U256::from(1)), true);
        assert_eq!(is_ext_call_precompile(U256::from(2)), true);
        assert_eq!(is_ext_call_precompile(U256::from(3)), true);
        assert_eq!(is_ext_call_precompile(U256::from(4)), false);
        assert_eq!(is_ext_call_precompile(U256::MAX), false);
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

#[cfg(test)]
mod test_yul {

    use ethers::types::U256;

    use crate::ether::evm::opcodes::{WrappedInput, WrappedOpcode};

    #[test]
    fn test_push0() {
        // wraps an ADD operation with 2 raw inputs
        let add_operation_wrapped = WrappedOpcode::new(0x5f, vec![]);
        assert_eq!(add_operation_wrapped.yulify(), "0");
    }

    #[test]
    fn test_yulify_add() {
        // wraps an ADD operation with 2 raw inputs
        let add_operation_wrapped = WrappedOpcode::new(
            0x01,
            vec![WrappedInput::Raw(U256::from(1u8)), WrappedInput::Raw(U256::from(2u8))],
        );
        assert_eq!(add_operation_wrapped.yulify(), "add(0x01, 0x02)");
    }

    #[test]
    fn test_yulify_add_complex() {
        // wraps an ADD operation with 2 raw inputs
        let add_operation_wrapped = WrappedOpcode::new(
            0x01,
            vec![WrappedInput::Raw(U256::from(1u8)), WrappedInput::Raw(U256::from(2u8))],
        );
        let complex_add_operation = WrappedOpcode::new(
            0x01,
            vec![WrappedInput::Opcode(add_operation_wrapped), WrappedInput::Raw(U256::from(3u8))],
        );
        assert_eq!(complex_add_operation.yulify(), "add(add(0x01, 0x02), 0x03)");
    }
}

#[cfg(test)]
mod test_signatures {
    use heimdall_cache::{delete_cache, store_cache};

    use crate::ether::signatures::{
        score_signature, ResolveSelector, ResolvedError, ResolvedFunction, ResolvedLog,
    };

    #[test]
    fn resolve_function_signature_should_return_none_when_cached_results_not_found() {
        let signature = String::from("test_signature_nocache");
        let result = ResolvedFunction::resolve(&signature);

        assert_eq!(result, None,)
    }

    #[test]
    fn resolve_function_signature_should_return_none_when_json_url_returns_empty_signatures() {
        delete_cache(&format!("selector.{}", "test_signature"));
        let signature = String::from("test_signature");
        let result = ResolvedFunction::resolve(&signature);
        assert_eq!(result, None);
    }

    #[test]
    fn resolve_error_signature_should_return_none_when_cached_results_not_found() {
        let signature = String::from("test_signature_notfound");
        let result = ResolvedError::resolve(&signature);
        assert_eq!(result, None);
    }

    #[test]
    fn resolve_error_signature_should_return_cached_results_when_found() {
        let signature = String::from("test_signature");
        let mut cached_results = Vec::new();
        cached_results.push(ResolvedError {
            name: String::from("test_event"),
            signature: String::from("test_signature"),
            inputs: vec![String::from("input_1"), String::from("input_2")],
        });
        store_cache(&format!("selector.{}", &signature), cached_results.clone(), None);

        let result = ResolvedError::resolve(&signature);
        assert_eq!(result, Some(cached_results));
    }

    #[test]
    fn resolve_error_signature_should_return_none_when_json_url_returns_none() {
        let signature = String::from("test_signature_notfound");
        let result = ResolvedError::resolve(&signature);
        assert_eq!(result, None);
    }

    #[test]
    fn resolve_error_signature_should_return_none_when_json_url_returns_empty_signatures() {
        let signature = String::from("test_signature_notfound");
        let result = ResolvedError::resolve(&signature);
        assert_eq!(result, None);
    }

    #[test]
    fn resolve_event_signature_should_return_none_when_cached_results_not_found() {
        let signature = String::from("test_signature_notfound");
        let result = ResolvedLog::resolve(&signature);
        assert_eq!(result, None);
    }

    #[test]
    fn resolve_event_signature_should_return_cached_results_when_found() {
        let signature = String::from("test_signature");
        let mut cached_results = Vec::new();
        cached_results.push(ResolvedLog {
            name: String::from("test_event"),
            signature: String::from("test_signature"),
            inputs: vec![String::from("input_1"), String::from("input_2")],
        });
        store_cache(&format!("selector.{}", &signature), cached_results.clone(), None);

        let result = ResolvedLog::resolve(&signature);
        assert_eq!(result, Some(cached_results));
    }

    #[test]
    fn resolve_event_signature_should_return_none_when_json_url_returns_none() {
        let signature = String::from("test_signature_notfound");
        let result = ResolvedLog::resolve(&signature);
        assert_eq!(result, None);
    }

    #[test]
    fn resolve_event_signature_should_return_none_when_json_url_returns_empty_signatures() {
        let signature = String::from("test_signature_notfound");
        let result = ResolvedLog::resolve(&signature);
        assert_eq!(result, None);
    }

    #[test]
    fn score_signature_should_return_correct_score() {
        let signature = String::from("test_signature");
        let score = score_signature(&signature);
        let expected_score = 1000 -
            (signature.len() as u32) -
            (signature.matches(|c: char| c.is_numeric()).count() as u32) * 3;
        assert_eq!(score, expected_score);
    }
}

#[cfg(test)]
mod test_selector {}

#[cfg(test)]
mod test_compiler {
    use crate::ether::compiler::detect_compiler;

    #[test]
    fn test_detect_compiler_proxy_minimal() {
        let bytecode = "363d3d373d3d3d363d73".to_string();
        let expected_result = ("proxy".to_string(), "minimal".to_string());
        assert_eq!(detect_compiler(bytecode), expected_result);
    }

    #[test]
    fn test_detect_compiler_proxy_vyper() {
        let bytecode = "366000600037611000600036600073".to_string();
        let expected_result = ("proxy".to_string(), "vyper".to_string());
        assert_eq!(detect_compiler(bytecode), expected_result);
    }

    #[test]
    fn test_detect_compiler_vyper_range_1() {
        let bytecode = "6004361015".to_string();
        let expected_result = ("vyper".to_string(), "0.2.0-0.2.4,0.2.11-0.3.3".to_string());
        assert_eq!(detect_compiler(bytecode), expected_result);
    }

    #[test]
    fn test_detect_compiler_vyper_range_2() {
        let bytecode = "341561000a".to_string();
        let expected_result = ("vyper".to_string(), "0.2.5-0.2.8".to_string());
        assert_eq!(detect_compiler(bytecode), expected_result);
    }

    #[test]
    fn test_detect_compiler_solc_range_1() {
        let bytecode = "731bf797".to_string();
        let expected_result = ("solc".to_string(), "0.4.10-0.4.24".to_string());
        assert_eq!(detect_compiler(bytecode), expected_result);
    }

    #[test]
    fn test_detect_compiler_solc_range_2() {
        let bytecode = "6080604052".to_string();
        let expected_result = ("solc".to_string(), "0.4.22+".to_string());
        assert_eq!(detect_compiler(bytecode), expected_result);
    }

    #[test]
    fn test_detect_compiler_solc_range_3() {
        let bytecode = "6060604052".to_string();
        let expected_result = ("solc".to_string(), "0.4.11-0.4.21".to_string());
        assert_eq!(detect_compiler(bytecode), expected_result);
    }

    #[test]
    fn test_detect_compiler_vyper() {
        let bytecode = "7679706572".to_string();
        let expected_result = ("vyper".to_string(), "unknown".to_string());
        assert_eq!(detect_compiler(bytecode), expected_result);
    }

    #[test]
    fn test_detect_compiler_solc() {
        let bytecode = "736f6c63".to_string();
        let expected_result = ("solc".to_string(), "unknown".to_string());
        assert_eq!(detect_compiler(bytecode), expected_result);
    }

    #[test]
    fn test_detect_compiler_solc_metadata() {
        let bytecode = "736f6c63434d4e".to_string();
        let expected_result = ("solc".to_string(), "unknown".to_string());
        assert_eq!(detect_compiler(bytecode), expected_result);
    }

    #[test]
    fn test_detect_compiler_vyper_metadata() {
        let bytecode = "7679706572833135353030".to_string();
        let expected_result = ("vyper".to_string(), "49.53.53".to_string());
        assert_eq!(detect_compiler(bytecode), expected_result);
    }
}
