//! Integration tests for disassemble functionality.

#[cfg(test)]
mod integration_tests {
    use std::{io::Write, path::PathBuf};

    use heimdall_disassembler::{disassemble, DisassemblerArgs, DisassemblerArgsBuilder, HardFork};
    use serde_json::Value;

    #[tokio::test]
    async fn test_disassemble_nominal() {
        let bytecode = include_str!("testdata/disassemble/nominal.hex");
        let expected = String::from(include_str!("testdata/disassemble/nominal.asm"));

        let assembly = disassemble(DisassemblerArgs {
            target: bytecode.to_owned(),
            rpc_url: String::from(""),
            decimal_counter: false,
            name: String::from(""),
            output: String::from(""),
            hardfork: HardFork::Latest,
            etherscan_api_key: String::from(""),
        })
        .await
        .expect("failed to disassemble");

        assert_eq!(expected, assembly);
    }

    #[tokio::test]
    async fn test_disassemble_decimal_counter_nominal() {
        let bytecode = include_str!("testdata/disassemble/nominal.hex");
        let expected =
            String::from(include_str!("testdata/disassemble/decimal_counter_nominal.asm"));

        let assembly = disassemble(DisassemblerArgs {
            target: bytecode.to_owned(),
            rpc_url: String::from(""),
            decimal_counter: true,
            name: String::from(""),
            output: String::from(""),
            hardfork: HardFork::Latest,
            etherscan_api_key: String::from(""),
        })
        .await
        .expect("failed to disassemble");

        assert_eq!(expected, assembly);
    }

    #[tokio::test]
    async fn test_disassemble_20_byte_non_address() {
        let bytecode = include_str!("testdata/disassemble/20_byte_non_address.hex");
        let expected = String::from(include_str!("testdata/disassemble/20_byte_non_address.asm"));

        let assembly = disassemble(DisassemblerArgs {
            target: bytecode.to_owned(),
            rpc_url: String::from(""),
            decimal_counter: false,
            name: String::from(""),
            output: String::from(""),
            hardfork: HardFork::Latest,
            etherscan_api_key: String::from(""),
        })
        .await
        .expect("failed to disassemble");

        assert_eq!(expected, assembly);
    }

    #[tokio::test]
    async fn test_disassemble_with_custom_output() {
        let bytecode = include_str!("testdata/disassemble/nominal.hex");
        let expected =
            String::from(include_str!("testdata/disassemble/decimal_counter_nominal.asm"));

        let assembly = disassemble(DisassemblerArgs {
            target: bytecode.to_owned(),
            rpc_url: String::from(""),
            decimal_counter: true,
            name: String::from(""),
            output: String::from(""),
            hardfork: HardFork::Latest,
            etherscan_api_key: String::from(""),
        })
        .await
        .expect("failed to disassemble");

        assert_eq!(expected, assembly);
    }

    #[tokio::test]
    async fn test_disassemble_silent() {
        let bytecode = include_str!("testdata/disassemble/nominal.hex");
        let expected =
            String::from(include_str!("testdata/disassemble/decimal_counter_nominal.asm"));

        let assembly = disassemble(DisassemblerArgs {
            target: bytecode.to_owned(),
            rpc_url: String::from(""),
            decimal_counter: true,
            name: String::from(""),
            output: String::from(""),
            hardfork: HardFork::Latest,
            etherscan_api_key: String::from(""),
        })
        .await
        .expect("failed to disassemble");

        assert_eq!(expected, assembly);
    }

    #[tokio::test]
    async fn test_disassemble_from_file() {
        let bytecode = include_str!("testdata/disassemble/nominal.hex");
        let expected =
            String::from(include_str!("testdata/disassemble/decimal_counter_nominal.asm"));

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
            hardfork: HardFork::Latest,
            etherscan_api_key: String::from(""),
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

        // This contract was deployed before Fusaka, so use Pectra hardfork
        // to show CLZ (0x1e) as unknown (it's part of the contract metadata)
        let expected = String::from(include_str!("testdata/disassemble/from_rpc.asm"));

        let assembly = disassemble(DisassemblerArgs {
            target: String::from("0xafc2f2d803479a2af3a72022d54cc0901a0ec0d6"),
            rpc_url,
            decimal_counter: true,
            name: String::from(""),
            output: String::from(""),
            hardfork: HardFork::Pectra,
            etherscan_api_key: String::from(""),
        })
        .await
        .expect("failed to disassemble");

        assert_eq!(expected, assembly);
    }

    #[tokio::test]
    async fn test_disassemble_auto_hardfork() {
        let rpc_url = std::env::var("RPC_URL").unwrap_or_else(|_| {
            println!("RPC_URL not set, skipping test");
            std::process::exit(0);
        });

        // WETH contract deployed at block 4719568 (Byzantium era)
        // Auto hardfork detection should correctly identify this
        let result = disassemble(DisassemblerArgs {
            target: String::from("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"),
            rpc_url,
            decimal_counter: true,
            name: String::from(""),
            output: String::from(""),
            hardfork: HardFork::Auto,
            etherscan_api_key: String::from(""),
        })
        .await
        .expect("failed to disassemble with auto hardfork");

        // Verify the disassembly succeeded and contains expected opcodes
        assert!(result.contains("PUSH1"));
        assert!(result.contains("MSTORE"));
    }

    #[tokio::test]
    async fn test_disassemble_auto_hardfork_fallback() {
        // When no RPC URL is provided, auto hardfork should fall back to Latest
        let bytecode = include_str!("testdata/disassemble/nominal.hex");
        let expected =
            String::from(include_str!("testdata/disassemble/decimal_counter_nominal.asm"));

        let assembly = disassemble(DisassemblerArgs {
            target: bytecode.to_owned(),
            rpc_url: String::from(""),
            decimal_counter: true,
            name: String::from(""),
            output: String::from(""),
            hardfork: HardFork::Auto,
            etherscan_api_key: String::from(""),
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
