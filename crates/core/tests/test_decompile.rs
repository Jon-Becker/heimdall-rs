#[cfg(test)]
mod integration_tests {
    use std::path::PathBuf;

    use alloy_json_abi::JsonAbi;
    use heimdall_decompiler::{decompile, DecompilerArgs, DecompilerArgsBuilder};
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
        })
        .await
        .expect("failed to decompile");

        // assert that the output is correct
        for line in &[
            "function Unresolved_095ea7b3(address arg0, uint256 arg1) public returns (bool) {",
            "function Unresolved_18160ddd() public view returns (uint256) {",
            "function Unresolved_23b872dd(address arg0, address arg1, uint256 arg2) public returns (bool) {",
            "function Unresolved_2e1a7d4d(uint256 arg0) public {",
            "function Unresolved_70a08231(address arg0) public view returns (uint256) {",
            "function Unresolved_a9059cbb(address arg0, uint256 arg1) public returns (bool) {",
            "function Unresolved_d0e30db0() public payable {",
            "function Unresolved_dd62ed3e(address arg0, address arg1) public view returns (uint256) {"] {
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
        })
        .await
        .expect("failed to decompile");

        // assert that the output is correct
        for line in &["function Unresolved_2fa61cd8(address arg0) public payable returns (uint16) {",
            "function Unresolved_41161b10(uint240 arg0, address arg1) public payable returns (bool) {",
            "function Unresolved_06fdde03() public payable returns (bytes memory) {"] {
            println!("{line}");
            assert!(result.source.as_ref().expect("decompile source is empty").contains(line));
        }
    }

    #[tokio::test]
    async fn test_decompile_vyper() {
        let result = decompile(DecompilerArgs {
            target: String::from("0x5f3560e01c63fdf80bda811861005d57602436103417610061576004358060a01c610061576040525f5c6002146100615760025f5d6040515a595f5f36365f8537835f8787f1905090509050610057573d5f5f3e3d5ffd5b60035f5d005b5f5ffd5b5f80fd"),
            rpc_url: String::from(""),
            default: true,
            skip_resolving: true,
            include_solidity: false,
            include_yul: true,
            output: String::from(""),
            name: String::from(""),
            timeout: 10000,
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
    async fn test_decompile_huff() {
        let result = decompile(DecompilerArgs {
            target: String::from("0x5f3560e01c806306fdde03146100295780632fa61cd81461004457806341161b1014610058575f5ffd5b60205f52684c61627972696e7468602952600960205260605ff35b6004355f524360205260405f205f5260205ff35b6004356024355f5f61020d565b60016100b2565b61021f57610149565b806100f9565b5f6100ca565b61012d57610278565b086101c7565b61012d57610249565b60026100e6565b526101b3565b806100f2565b82610114565b5f6100a0565b016100c4565b91610154565b906101c1565bf35b60016100ec565b6010610108565b836101a7565b9361017d565b146101f7565b01610100565b6001610234565b6003610143565b9161014f565bf35b83610159565b0261013d565b60ff61019b565b10610240565b836101cd565b1661023a565b602061026c565b6101ba576100a6565b1c610255565b1461027e565b80610099565b61024f565b61024f565b066101a1565b036100be565b16610192565b90610226565b81610272565b916101e1565b106101e8565b016101db565b6100655761016b565b61012d576100ac565b14610189565b15610090565b1c61022d565b15610134565b602061007b565b6010610171565b50610213565b15610081565b600161008a565b600261010e565b80610206565b6010610183565b61012d5761025c565b91610267565b6100d357610075565b806100e0565b60ff61011b565b82610261565b61020d565b600161015f565b6010610121565b60016100b8565b6001610165565b1461006c565b806101ad565b61012d576101f1565b91610218565b846100da565b6003610127565b61024f565b816101d4565b61024f565b5f610106565b03610200565b916100cc565b61017757"),
            rpc_url: String::from(""),
            default: true,
            skip_resolving: true,
            include_solidity: false,
            include_yul: true,
            output: String::from(""),
            name: String::from(""),
            timeout: 10000,
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
}
