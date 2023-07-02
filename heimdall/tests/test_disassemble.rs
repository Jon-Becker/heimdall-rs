#[cfg(test)]
mod benchmarks {
    use clap_verbosity_flag::Verbosity;

    use heimdall_common::{
        ether::evm::disassemble::{disassemble, DisassemblerArgs},
        testing::benchmarks::benchmark,
    };

    #[test]
    fn benchmark_disassemble_simple() {
        fn bench() {
            disassemble(DisassemblerArgs {
                target: String::from("731bf797219482a29013d804ad96d1c6f84fba4c453014608060405260043610610058576000357c0100000000000000000000000000000000000000000000000000000000900463ffffffff16806319045a251461005d575b600080fd5b6100c56004803603810190808035600019169060200190929190803590602001908201803590602001908080601f0160208091040260200160405190810160405280939291908181526020018383808284378201915050505050509192919290505050610107565b604051808273ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff16815260200191505060405180910390f35b6000806000806041855114151561012157600093506101f6565b6020850151925060408501519150606085015160001a9050601b8160ff16101561014c57601b810190505b601b8160ff16141580156101645750601c8160ff1614155b1561017257600093506101f6565b600186828585604051600081526020016040526040518085600019166000191681526020018460ff1660ff1681526020018360001916600019168152602001826000191660001916815260200194505050505060206040516020810390808403906000865af11580156101e9573d6000803e3d6000fd5b5050506020604051035193505b505050929150505600a165627a7a72305820aacffa0494cd3f043493eee9c720bca9d5ef505ae7230ffc3d88c49ceeb7441e0029"),
                verbose: Verbosity::new(0, 0),
                output: String::from(""),
                rpc_url: String::from(""),
            });
        }

        benchmark("benchmark_disassemble_simple", 100, bench)
    }
}

#[cfg(test)]
mod integration_tests {
    use std::io::Write;

    use clap_verbosity_flag::Verbosity;

    use heimdall_common::ether::evm::disassemble::{disassemble, DisassemblerArgs};

    #[test]
    fn test_disassemble_nominal() {
        let bytecode = "366000600037611000600036600073";
        let expected = String::from("0 CALLDATASIZE \n2 PUSH1 00\n4 PUSH1 00\n5 CALLDATACOPY \n8 PUSH2 1000\n10 PUSH1 00\n11 CALLDATASIZE \n13 PUSH1 00\n");

        let assembly = disassemble(DisassemblerArgs {
            target: bytecode.to_owned(),
            verbose: Verbosity::new(0, 0),
            output: String::from(""),
            rpc_url: String::from(""),
        });

        assert_eq!(expected, assembly);
    }

    #[test]
    fn test_disassemble_with_custom_output() {
        let bytecode = "366000600037611000600036600073";
        let expected = String::from("0 CALLDATASIZE \n2 PUSH1 00\n4 PUSH1 00\n5 CALLDATACOPY \n8 PUSH2 1000\n10 PUSH1 00\n11 CALLDATASIZE \n13 PUSH1 00\n");

        let assembly = disassemble(DisassemblerArgs {
            target: bytecode.to_owned(),
            verbose: Verbosity::new(0, 0),
            output: String::from("/tmp/heimdall-rs/"),
            rpc_url: String::from(""),
        });

        assert_eq!(expected, assembly);
    }

    #[test]
    fn test_disassemble_silent() {
        let bytecode = "366000600037611000600036600073";
        let expected = String::from("0 CALLDATASIZE \n2 PUSH1 00\n4 PUSH1 00\n5 CALLDATACOPY \n8 PUSH2 1000\n10 PUSH1 00\n11 CALLDATASIZE \n13 PUSH1 00\n");

        let assembly = disassemble(DisassemblerArgs {
            target: bytecode.to_owned(),
            verbose: Verbosity::new(0, 1),
            output: String::from("/tmp/heimdall-rs/"),
            rpc_url: String::from(""),
        });

        assert_eq!(expected, assembly);
    }

    #[test]
    fn test_disassemble_from_file() {
        let bytecode = "366000600037611000600036600073";
        let expected = String::from("0 CALLDATASIZE \n2 PUSH1 00\n4 PUSH1 00\n5 CALLDATACOPY \n8 PUSH2 1000\n10 PUSH1 00\n11 CALLDATASIZE \n13 PUSH1 00\n");

        // write bytecode to file at the cwd
        let mut file = std::fs::File::create("test_disassemble_from_file").unwrap();
        file.write_all(bytecode.as_bytes()).unwrap();
        let assembly = disassemble(DisassemblerArgs {
            target: String::from("test_disassemble_from_file"),
            verbose: Verbosity::new(0, 0),
            output: String::from("/tmp/heimdall-rs/"),
            rpc_url: String::from(""),
        });

        assert_eq!(expected, assembly);
        // delete the file
        std::fs::remove_file("test_disassemble_from_file").unwrap();
    }

    #[test]
    fn test_disassemble_from_rpc() {
        let expected = String::from("1 PUSH1 80\n3 PUSH1 40\n4 MSTORE \n25 PUSH20 ffffffffffffffffffffffffffffffffffffffff\n27 PUSH1 00\n28 SLOAD \n29 AND \n30 CALLDATASIZE \n32 PUSH1 00\n33 DUP1 \n34 CALLDATACOPY \n36 PUSH1 00\n37 DUP1 \n38 CALLDATASIZE \n40 PUSH1 00\n41 DUP5 \n42 GAS \n43 DELEGATECALL \n44 RETURNDATASIZE \n46 PUSH1 00\n47 DUP1 \n48 RETURNDATACOPY \n50 PUSH1 00\n51 DUP2 \n52 EQ \n53 ISZERO \n55 PUSH1 3d\n56 JUMPI \n57 RETURNDATASIZE \n59 PUSH1 00\n60 REVERT \n61 JUMPDEST \n62 RETURNDATASIZE \n64 PUSH1 00\n65 RETURN \n66 INVALID \n67 LOG1 \n74 PUSH6 627a7a723058\n75 SHA3 \n76 unknown \n107 PUSH30 648b83cfac072cbccefc2ffc62a6999d4a050ee87a721942de1da9670db8\n108 STOP \n109 unknown \n");

        let assembly = disassemble(DisassemblerArgs {
            target: String::from("0xafc2f2d803479a2af3a72022d54cc0901a0ec0d6"),
            verbose: Verbosity::new(0, 0),
            output: String::from("/tmp/heimdall-rs/"),
            rpc_url: String::from("https://eth.llamarpc.com"),
        });

        assert_eq!(expected, assembly);
    }
}
