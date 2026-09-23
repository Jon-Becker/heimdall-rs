//! Integration tests for decompile functionality.

#[cfg(test)]
mod integration_tests {
    use std::path::PathBuf;

    use alloy_json_abi::JsonAbi;
    use heimdall_decompiler::{decompile, DecompilerArgs, DecompilerArgsBuilder, HardFork};
    use serde_json::Value;

    #[tokio::test]
    async fn test_decompile_precompile() {
        let rpc_url = std::env::var("RPC_URL").unwrap_or_else(|_| {
            println!("RPC_URL not set, skipping test");
            std::process::exit(0);
        });

        let result = decompile(DecompilerArgs {
            target: String::from("0x1bf797219482a29013d804ad96d1c6f84fba4c45"),
            rpc_url,
            default: true,
            skip_resolving: true,
            include_solidity: true,
            include_yul: false,
            output: String::from(""),
            name: String::from(""),
            timeout: 10000,
            abi: None,
            openrouter_api_key: String::from(""),
            model: String::from(""),
            llm_postprocess: false,
            etherscan_api_key: String::from(""),
            hardfork: HardFork::Latest,
        })
        .await
        .expect("failed to decompile");

        // assert that the output is correct
        for line in &["function Unresolved_19045a25(uint256 arg0, uint256 arg1) public payable returns (address) {",
            " = ecrecover("] {
            println!("{line}");
            assert!(result.source.as_ref().expect("decompile source is empty").contains(line));
        }
    }

    #[tokio::test]
    async fn test_decompile_edge_case_u256_conversion_overflow_1() {
        let rpc_url = std::env::var("RPC_URL").unwrap_or_else(|_| {
            println!("RPC_URL not set, skipping test");
            std::process::exit(0);
        });

        let _ = decompile(DecompilerArgs {
            target: String::from("0x914d7Fec6aaC8cd542e72Bca78B30650d45643d7"),
            rpc_url,
            default: true,
            skip_resolving: true,
            include_solidity: true,
            include_yul: false,
            output: String::from(""),
            name: String::from(""),
            timeout: 10000,
            abi: None,
            openrouter_api_key: String::from(""),
            model: String::from(""),
            llm_postprocess: false,
            etherscan_api_key: String::from(""),
            hardfork: HardFork::Latest,
        })
        .await
        .expect("failed to decompile");
    }

    #[tokio::test]
    async fn test_decompile_edge_case_u256_conversion_overflow_2() {
        let rpc_url = std::env::var("RPC_URL").unwrap_or_else(|_| {
            println!("RPC_URL not set, skipping test");
            std::process::exit(0);
        });

        let _ = decompile(DecompilerArgs {
            target: String::from("0x5141b82f5ffda4c6fe1e372978f1c5427640a190"),
            rpc_url,
            default: true,
            skip_resolving: true,
            include_solidity: true,
            include_yul: false,
            output: String::from(""),
            name: String::from(""),
            timeout: 10000,
            abi: None,
            openrouter_api_key: String::from(""),
            model: String::from(""),
            llm_postprocess: false,
            etherscan_api_key: String::from(""),
            hardfork: HardFork::Latest,
        })
        .await
        .expect("failed to decompile");
    }

    #[tokio::test]
    async fn test_decompile_edge_case_vec_overflow() {
        let rpc_url = std::env::var("RPC_URL").unwrap_or_else(|_| {
            println!("RPC_URL not set, skipping test");
            std::process::exit(0);
        });

        let _ = decompile(DecompilerArgs {
            target: String::from("0x8579970692bf77fafeeb017f07dec9a8fdb4893d"),
            rpc_url,
            default: true,
            skip_resolving: true,
            include_solidity: true,
            include_yul: false,
            output: String::from(""),
            name: String::from(""),
            timeout: 10000,
            abi: None,
            openrouter_api_key: String::from(""),
            model: String::from(""),
            llm_postprocess: false,
            etherscan_api_key: String::from(""),
            hardfork: HardFork::Latest,
        })
        .await
        .expect("failed to decompile");
    }

    #[tokio::test]
    async fn test_decompile_weth() {
        let rpc_url = std::env::var("RPC_URL").unwrap_or_else(|_| {
            println!("RPC_URL not set, skipping test");
            std::process::exit(0);
        });

        let result = decompile(DecompilerArgs {
            target: String::from("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"),
            rpc_url,
            default: true,
            skip_resolving: true,
            include_solidity: true,
            include_yul: false,
            output: String::from(""),
            name: String::from(""),
            timeout: 10000,
            abi: None,
            openrouter_api_key: String::from(""),
            model: String::from(""),
            llm_postprocess: false,
            etherscan_api_key: String::from(""),
            hardfork: HardFork::Latest,
        })
        .await
        .expect("failed to decompile");

        // assert that the output is correct
        for line in &[
            "function Unresolved_095ea7b3(address arg0, uint256 arg1) public returns (bool) {",
            "function Unresolved_18160ddd() public view returns (uint256) {",
            "function Unresolved_23b872dd(address arg0, address arg1, uint256 arg2) public returns (bool) {",
            "function Unresolved_2e1a7d4d(uint256 arg0) public {",
            "string public unresolved_06fdde03; // storage slot: 0x00",
            "string public unresolved_95d89b41; // storage slot: 0x01",
            "uint8 store_d; // storage slot: 0x02",
            "mapping(address => uint256) public unresolved_70a08231; // storage slot: 0x03",
            "mapping(address => mapping(address => uint256)) public unresolved_dd62ed3e; // storage slot: 0x04",
            "function Unresolved_a9059cbb(address arg0, uint256 arg1) public returns (bool) {",
            "function Unresolved_d0e30db0() public payable {"] {
            println!("{line}");
            assert!(result.source.as_ref().expect("decompile source is empty").contains(line));
        }
    }

    #[tokio::test]
    async fn test_decompile_ctf() {
        let rpc_url = std::env::var("RPC_URL").unwrap_or_else(|_| {
            println!("RPC_URL not set, skipping test");
            std::process::exit(0);
        });

        let result = decompile(DecompilerArgs {
            target: String::from("0x9f00c43700bc0000Ff91bE00841F8e04c0495000"),
            rpc_url,
            default: true,
            skip_resolving: true,
            include_solidity: true,
            include_yul: false,
            output: String::from(""),
            name: String::from(""),
            timeout: 10000,
            abi: None,
            openrouter_api_key: String::from(""),
            model: String::from(""),
            llm_postprocess: false,
            etherscan_api_key: String::from(""),
            hardfork: HardFork::Latest,
        })
        .await
        .expect("failed to decompile");

        // assert that the output is correct
        for line in &["function Unresolved_2fa61cd8(address arg0) public view returns (uint16) {",
            "function Unresolved_41161b10(uint240 arg0, address arg1) public payable returns (bool) {",
            "constant unresolved_06fdde03"] {
            println!("{line}");
            assert!(result.source.as_ref().expect("decompile source is empty").contains(line));
        }
    }

    #[tokio::test]
    async fn test_decompile_vyper() {
        let result = decompile(DecompilerArgs {
            target: String::from(include_str!("testdata/decompile/vyper.hex")),
            rpc_url: String::from(""),
            default: true,
            skip_resolving: true,
            include_solidity: false,
            include_yul: true,
            output: String::from(""),
            name: String::from(""),
            timeout: 10000,
            abi: None,
            openrouter_api_key: String::from(""),
            model: String::from(""),
            llm_postprocess: false,
            etherscan_api_key: String::from(""),
            hardfork: HardFork::Latest,
        })
        .await
        .expect("failed to decompile");

        // assert that the output is correct
        for line in &[
            "default {",
            "if eq(0x02, tload(0)) { revert(0, 0); } else {",
            "tstore(0, 0x02)",
            "call(gas(), mload(0x40), 0, msize(), calldatasize(), 0, 0)",
        ] {
            println!("{line}");
            assert!(result.source.as_ref().expect("decompile source is empty").contains(line));
        }
    }

    #[tokio::test]
    async fn test_decompile_clamping() {
        // NOTE: this test is only checking for runtime. decompilation *must* finish within 5
        // seconds, or the test fails.
        let start = std::time::Instant::now();
        let _ = decompile(DecompilerArgs {
            target: String::from(include_str!("testdata/decompile/clamping.hex")),
            rpc_url: String::from(""),
            default: true,
            skip_resolving: true,
            include_solidity: false,
            include_yul: true,
            output: String::from(""),
            name: String::from(""),
            timeout: 10000,
            abi: None,
            openrouter_api_key: String::from(""),
            model: String::from(""),
            llm_postprocess: false,
            etherscan_api_key: String::from(""),
            hardfork: HardFork::Latest,
        })
        .await
        .expect("failed to decompile");

        // assert that runtime <= 3000 ms
        let end = std::time::Instant::now();
        let runtime = end.duration_since(start);
        assert!(runtime.as_millis() <= 3000, "decompile took too long: {} ms", runtime.as_millis());
    }

    #[tokio::test]
    async fn test_decompile_huff() {
        let result = decompile(DecompilerArgs {
            target: String::from(include_str!("testdata/decompile/huff.hex")),
            rpc_url: String::from(""),
            default: true,
            skip_resolving: true,
            include_solidity: false,
            include_yul: true,
            output: String::from(""),
            name: String::from(""),
            timeout: 10000,
            abi: None,
            openrouter_api_key: String::from(""),
            model: String::from(""),
            llm_postprocess: false,
            etherscan_api_key: String::from(""),
            hardfork: HardFork::Latest,
        })
        .await
        .expect("failed to decompile");

        // assert that the output is correct
        for line in &["case 0x41161b10", "case 0x06fdde03", "mstore(0, 0x01)", "return(0, 0x20)"] {
            println!("{line}");
            assert!(result.source.as_ref().expect("decompile source is empty").contains(line));
        }
    }

    #[tokio::test]
    async fn test_decompile_base_edge_case_abi() {
        let _ = decompile(DecompilerArgs {
            target: String::from(include_str!("testdata/decompile/base_edge_case_abi.hex")),
            rpc_url: String::from(""),
            openrouter_api_key: String::from(""),
            model: String::from(""),
            llm_postprocess: false,
            etherscan_api_key: String::from(""),
            default: true,
            skip_resolving: true,
            include_solidity: true,
            include_yul: false,
            output: String::from(""),
            name: String::from(""),
            timeout: 10000,
            abi: None,
            hardfork: HardFork::Latest,
        })
        .await
        .expect("failed to decompile");
    }

    #[tokio::test]
    async fn test_decompile_auto_hardfork() {
        let rpc_url = std::env::var("RPC_URL").unwrap_or_else(|_| {
            println!("RPC_URL not set, skipping test");
            std::process::exit(0);
        });

        // Test auto hardfork detection with a real contract
        let result = decompile(DecompilerArgs {
            target: String::from("0x1bf797219482a29013d804ad96d1c6f84fba4c45"),
            rpc_url,
            default: true,
            skip_resolving: true,
            include_solidity: true,
            include_yul: false,
            output: String::from(""),
            name: String::from(""),
            timeout: 10000,
            abi: None,
            openrouter_api_key: String::from(""),
            model: String::from(""),
            llm_postprocess: false,
            etherscan_api_key: String::from(""),
            hardfork: HardFork::Auto,
        })
        .await
        .expect("failed to decompile with auto hardfork");

        // Verify the decompilation succeeded
        assert!(result.source.is_some());
        assert!(!result.source.as_ref().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_decompile_auto_hardfork_fallback() {
        // When target is bytecode (not an address), auto hardfork should fall back to Latest
        let bytecode = include_str!("testdata/decompile/auto_hardfork_fallback.hex");

        let result = decompile(DecompilerArgs {
            target: bytecode.to_string(),
            rpc_url: String::from(""),
            default: true,
            skip_resolving: true,
            include_solidity: true,
            include_yul: false,
            output: String::from(""),
            name: String::from(""),
            timeout: 10000,
            abi: None,
            openrouter_api_key: String::from(""),
            model: String::from(""),
            llm_postprocess: false,
            etherscan_api_key: String::from(""),
            hardfork: HardFork::Auto,
        })
        .await
        .expect("failed to decompile with auto hardfork fallback");

        // Verify the decompilation succeeded
        assert!(result.source.is_some());
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

        // define flag checks
        let mut is_function_covered = false;
        let mut is_event_covered = false;
        let mut is_require_covered = false;
        let mut is_error_covered = false;

        let mut success_count = 0;
        let mut fail_count = 0;

        for (contract_address, bytecode) in contracts {
            println!("Testing contract: {contract_address}");
            let args = DecompilerArgsBuilder::new()
                .target(bytecode)
                .skip_resolving(true)
                .include_solidity(true)
                .timeout(10000)
                .build()
                .expect("failed to build args");

            let result = match decompile(args).await.map_err(|e| {
                eprintln!("failed to decompile {contract_address}: {e}");
                e
            }) {
                Ok(result) => {
                    success_count += 1;
                    result
                }
                Err(_) => {
                    fail_count += 1;
                    continue;
                }
            };

            let output = result.source.expect("decompile source is empty");

            // perform flag checks
            if output.contains("function Unresolved_") {
                is_function_covered = true;
            }
            if output.contains("event Event_") {
                is_event_covered = true;
            }
            if output.contains("require(") {
                is_require_covered = true;
            }
            if output.contains("error CustomError_") {
                is_error_covered = true;
            }

            let abi_serialized = serde_json::to_string(&result.abi).unwrap();
            let abi_deserialized = JsonAbi::from_json_str(&abi_serialized);
            assert!(abi_deserialized.is_ok());
        }

        // assert that all flags are true
        assert!(is_function_covered);
        assert!(is_event_covered);
        assert!(is_require_covered);
        assert!(is_error_covered);

        // assert 99% success rate
        assert!(
            success_count as f64 / (success_count + fail_count) as f64 > 0.99,
            "success rate is less than 99%"
        );
    }

    #[tokio::test]
    async fn test_decompile_extended_abi() {
        // Test that the extended ABI includes selector and signature fields
        let bytecode = include_str!("testdata/decompile/extended_abi.hex");

        let args = DecompilerArgsBuilder::new()
            .target(bytecode.to_string())
            .skip_resolving(true)
            .include_solidity(true)
            .timeout(10000)
            .build()
            .expect("failed to build args");

        let result = decompile(args).await.expect("failed to decompile");

        // Check that the standard ABI is valid
        let abi_serialized = serde_json::to_string(&result.abi).unwrap();
        let abi_deserialized = JsonAbi::from_json_str(&abi_serialized);
        assert!(abi_deserialized.is_ok());

        // Check that the extended ABI contains selector and signature fields
        let extended_abi = &result.abi_with_details;

        // The extended ABI is an array of ABI items
        assert!(extended_abi.is_array(), "Extended ABI should be an array");
        let abi_items = extended_abi.as_array().unwrap();
        assert!(!abi_items.is_empty(), "Extended ABI should contain items");

        // Group items by type
        let mut functions = Vec::new();
        let mut events = Vec::new();
        let mut errors = Vec::new();

        for item in abi_items {
            assert!(item.is_object());
            let item_obj = item.as_object().unwrap();

            if let Some(item_type) = item_obj.get("type").and_then(|v| v.as_str()) {
                match item_type {
                    "function" => functions.push(item_obj),
                    "event" => events.push(item_obj),
                    "error" => errors.push(item_obj),
                    _ => {}
                }
            }
        }

        // Check functions
        assert!(!functions.is_empty(), "Extended ABI should contain functions");
        for func_obj in &functions {
            // Verify selector field exists and is a string
            assert!(func_obj.contains_key("selector"), "Function should have a selector field");
            let selector = func_obj.get("selector").unwrap();
            assert!(selector.is_string(), "Selector should be a string");
            let selector_str = selector.as_str().unwrap();
            assert!(selector_str.starts_with("0x"), "Selector should start with 0x");
            assert_eq!(
                selector_str.len(),
                10,
                "Selector should be 10 characters (0x + 8 hex chars)"
            );

            // Verify signature field exists and is a string
            assert!(func_obj.contains_key("signature"), "Function should have a signature field");
            let signature = func_obj.get("signature").unwrap();
            assert!(signature.is_string(), "Signature should be a string");
            let sig_str = signature.as_str().unwrap();
            assert!(sig_str.contains("("), "Signature should contain opening parenthesis");
            assert!(sig_str.contains(")"), "Signature should contain closing parenthesis");
        }

        // Check events if present
        for event_obj in &events {
            // Events should have selector (topic0) and signature
            assert!(event_obj.contains_key("selector"), "Event should have a selector field");
            assert!(event_obj.contains_key("signature"), "Event should have a signature field");
        }

        // Check errors if present
        for error_obj in &errors {
            // Errors should have selector and signature
            assert!(error_obj.contains_key("selector"), "Error should have a selector field");
            assert!(error_obj.contains_key("signature"), "Error should have a signature field");
        }
    }

    /// A minimal local dispatcher which routes three selectors, in an order which does not match
    /// their alphabetical order. Used to assert deterministic output without any RPC dependency.
    const MULTI_SELECTOR_BYTECODE: &str = include_str!("testdata/decompile/multi_selector.hex");

    async fn decompile_local(include_solidity: bool) -> String {
        let args = DecompilerArgsBuilder::new()
            .target(MULTI_SELECTOR_BYTECODE.to_string())
            .skip_resolving(true)
            .include_solidity(include_solidity)
            .include_yul(!include_solidity)
            .build()
            .expect("failed to build args");

        decompile(args)
            .await
            .expect("failed to decompile")
            .source
            .expect("decompile source is empty")
    }

    #[tokio::test]
    async fn test_decompile_output_is_deterministic_solidity() {
        let first = decompile_local(true).await;
        let second = decompile_local(true).await;

        // repeated decompilation of the same bytecode is byte-identical
        assert_eq!(first, second);

        // functions are emitted in alphabetical order
        let names = first
            .lines()
            .filter_map(|line| line.trim().strip_prefix("function "))
            .map(|signature| signature.split('(').next().expect("empty signature").to_string())
            .collect::<Vec<_>>();
        assert_eq!(
            names,
            vec!["Unresolved_11111101", "Unresolved_88888801", "Unresolved_ffffff01"]
        );
    }

    #[tokio::test]
    async fn test_decompile_output_is_deterministic_yul() {
        let first = decompile_local(false).await;
        let second = decompile_local(false).await;

        // repeated decompilation of the same bytecode is byte-identical
        assert_eq!(first, second);

        // cases are emitted in alphabetical order of their emitted function names
        let cases = first
            .lines()
            .map(|line| line.trim())
            .filter(|line| line.starts_with("case 0x"))
            .collect::<Vec<_>>();
        assert_eq!(cases, vec!["case 0x11111101 {", "case 0x88888801 {", "case 0xffffff01 {"]);
    }
}
