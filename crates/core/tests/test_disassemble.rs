//! Integration tests for disassemble functionality.

#[cfg(test)]
mod integration_tests {
    use std::{io::Write, path::PathBuf};

    use heimdall_disassembler::{disassemble, DisassemblerArgs, DisassemblerArgsBuilder};
    use serde_json::Value;

    #[tokio::test]
    async fn test_disassemble_nominal() {
        let bytecode = "366000600037611000600036600073";
        let expected = String::from("000000 CALLDATASIZE \n000001 PUSH1 00\n000003 PUSH1 00\n000005 CALLDATACOPY \n000006 PUSH2 1000\n000009 PUSH1 00\n00000b CALLDATASIZE \n00000c PUSH1 00\n");

        let assembly = disassemble(DisassemblerArgs {
            target: bytecode.to_owned(),
            rpc_url: String::from(""),
            decimal_counter: false,
            name: String::from(""),
            output: String::from(""),
        })
        .await
        .expect("failed to disassemble");

        assert_eq!(expected, assembly);
    }

    #[tokio::test]
    async fn test_disassemble_decimal_counter_nominal() {
        let bytecode = "366000600037611000600036600073";
        let expected = String::from("0 CALLDATASIZE \n1 PUSH1 00\n3 PUSH1 00\n5 CALLDATACOPY \n6 PUSH2 1000\n9 PUSH1 00\n11 CALLDATASIZE \n12 PUSH1 00\n");

        let assembly = disassemble(DisassemblerArgs {
            target: bytecode.to_owned(),
            rpc_url: String::from(""),
            decimal_counter: true,
            name: String::from(""),
            output: String::from(""),
        })
        .await
        .expect("failed to disassemble");

        assert_eq!(expected, assembly);
    }

    #[tokio::test]
    async fn test_disassemble_20_byte_non_address() {
        let bytecode = "608060405234801561000f575f5ffd5b50600400";
        let expected = String::from("000000 PUSH1 80\n000002 PUSH1 40\n000004 MSTORE \n000005 CALLVALUE \n000006 DUP1 \n000007 ISZERO \n000008 PUSH2 000f\n00000b JUMPI \n00000c PUSH0 \n00000d PUSH0 \n00000e REVERT \n00000f JUMPDEST \n000010 POP \n000011 PUSH1 04\n000013 STOP \n");

        let assembly = disassemble(DisassemblerArgs {
            target: bytecode.to_owned(),
            rpc_url: String::from(""),
            decimal_counter: false,
            name: String::from(""),
            output: String::from(""),
        })
        .await
        .expect("failed to disassemble");

        assert_eq!(expected, assembly);
    }

    #[tokio::test]
    async fn test_disassemble_with_custom_output() {
        let bytecode = "366000600037611000600036600073";
        let expected = String::from("0 CALLDATASIZE \n1 PUSH1 00\n3 PUSH1 00\n5 CALLDATACOPY \n6 PUSH2 1000\n9 PUSH1 00\n11 CALLDATASIZE \n12 PUSH1 00\n");

        let assembly = disassemble(DisassemblerArgs {
            target: bytecode.to_owned(),
            rpc_url: String::from(""),
            decimal_counter: true,
            name: String::from(""),
            output: String::from(""),
        })
        .await
        .expect("failed to disassemble");

        assert_eq!(expected, assembly);
    }

    #[tokio::test]
    async fn test_disassemble_silent() {
        let bytecode = "366000600037611000600036600073";
        let expected = String::from("0 CALLDATASIZE \n1 PUSH1 00\n3 PUSH1 00\n5 CALLDATACOPY \n6 PUSH2 1000\n9 PUSH1 00\n11 CALLDATASIZE \n12 PUSH1 00\n");

        let assembly = disassemble(DisassemblerArgs {
            target: bytecode.to_owned(),
            rpc_url: String::from(""),
            decimal_counter: true,
            name: String::from(""),
            output: String::from(""),
        })
        .await
        .expect("failed to disassemble");

        assert_eq!(expected, assembly);
    }

    #[tokio::test]
    async fn test_disassemble_from_file() {
        let bytecode = "366000600037611000600036600073";
        let expected = String::from("0 CALLDATASIZE \n1 PUSH1 00\n3 PUSH1 00\n5 CALLDATACOPY \n6 PUSH2 1000\n9 PUSH1 00\n11 CALLDATASIZE \n12 PUSH1 00\n");

        // write bytecode to file at the cwd
        let mut file =
            std::fs::File::create("test_disassemble_from_file").expect("failed to create file");
        file.write_all(bytecode.as_bytes()).expect("failed to write file");
        let assembly = disassemble(DisassemblerArgs {
            target: String::from("test_disassemble_from_file"),
            rpc_url: String::from(""),
            decimal_counter: true,
            name: String::from(""),
            output: String::from(""),
        })
        .await
        .expect("failed to disassemble");

        assert_eq!(expected, assembly);

        // delete the file
        std::fs::remove_file("test_disassemble_from_file").expect("failed to delete file");
    }

    #[tokio::test]
    async fn test_disassemble_from_rpc() {
        let rpc_url = std::env::var("RPC_URL").unwrap_or_else(|_| {
            println!("RPC_URL not set, skipping test");
            std::process::exit(0);
        });

        let expected = String::from("0 PUSH1 80\n2 PUSH1 40\n4 MSTORE \n5 PUSH20 ffffffffffffffffffffffffffffffffffffffff\n26 PUSH1 00\n28 SLOAD \n29 AND \n30 CALLDATASIZE \n31 PUSH1 00\n33 DUP1 \n34 CALLDATACOPY \n35 PUSH1 00\n37 DUP1 \n38 CALLDATASIZE \n39 PUSH1 00\n41 DUP5 \n42 GAS \n43 DELEGATECALL \n44 RETURNDATASIZE \n45 PUSH1 00\n47 DUP1 \n48 RETURNDATACOPY \n49 PUSH1 00\n51 DUP2 \n52 EQ \n53 ISZERO \n54 PUSH1 3d\n56 JUMPI \n57 RETURNDATASIZE \n58 PUSH1 00\n60 REVERT \n61 JUMPDEST \n62 RETURNDATASIZE \n63 PUSH1 00\n65 RETURN \n66 INVALID \n67 LOG1 \n68 PUSH6 627a7a723058\n75 SHA3 \n76 unknown \n77 PUSH30 648b83cfac072cbccefc2ffc62a6999d4a050ee87a721942de1da9670db8\n108 STOP \n109 unknown \n");

        let assembly = disassemble(DisassemblerArgs {
            target: String::from("0xafc2f2d803479a2af3a72022d54cc0901a0ec0d6"),
            rpc_url,
            decimal_counter: true,
            name: String::from(""),
            output: String::from(""),
        })
        .await
        .expect("failed to disassemble");

        assert_eq!(expected, assembly);
    }

    #[tokio::test]
    #[ignore]
    async fn heavy_integration_test() {
        let root_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("no parent")
            .parent()
            .expect("no parent")
            .to_owned();

        // if the ./largest1k directory does not exist, download it from https://jbecker.dev/data/largest1k.tar.gz
        let dataset_dir = root_dir.join("largest1k");
        if !dataset_dir.exists() {
            eprintln!("dataset not found in root, skipping test");
            std::process::exit(0);
        }

        // list files in root_dir
        let contracts = std::fs::read_dir(dataset_dir)
            .expect("failed to read dataset directory")
            .map(|res| {
                // HashMap from filename (without extension) to bytecode (from serde_json::Value)
                res.map(|e| {
                    let path = e.path();
                    let filename = path
                        .file_stem()
                        .expect("no file stem")
                        .to_str()
                        .expect("no file stem")
                        .to_owned();

                    // read contents as json and parse to serde_json::Value
                    let contents_json: Value = serde_json::from_str(
                        &std::fs::read_to_string(path).expect("failed to read file"),
                    )
                    .expect("failed to parse json");
                    let bytecode = contents_json["code"].as_str().expect("no bytecode").to_owned();

                    (filename, bytecode)
                })
            })
            .collect::<Result<Vec<_>, std::io::Error>>()
            .expect("failed to collect files");

        for (contract_address, bytecode) in contracts {
            println!("Disassembling contract: {contract_address}");
            let args = DisassemblerArgsBuilder::new()
                .target(bytecode)
                .output(String::from("./output/tests/disassemble/integration"))
                .build()
                .expect("failed to build args");

            let _ = disassemble(args)
                .await
                .map_err(|e| {
                    eprintln!("failed to disassemble {contract_address}: {e}");
                    e
                })
                .expect("failed to disassemble");
        }
    }
}
