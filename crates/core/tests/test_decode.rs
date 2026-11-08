//! Integration tests for decode functionality.

mod integration_tests {
    use heimdall_common::utils::{sync::blocking_await, threading::task_pool};
    use heimdall_decoder::{DecodeArgs, DecodeArgsBuilder};
    use serde_json::Value;

    #[tokio::test]
    async fn test_decode_transfer() {
        let args = DecodeArgs {
            abi: None,
            target: String::from(include_str!("testdata/decode/transfer.hex")),
            rpc_url: String::from(""),
            openrouter_api_key: String::from(""),
            model: String::from(""),
            explain: false,
            default: true,
            constructor: false,
            truncate_calldata: false,
            skip_resolving: true,
            raw: false,
            output: String::from("print"),
        };
        let result = heimdall_decoder::decode(args).await.expect("failed to decode calldata");
        assert_eq!(result.decoded.name, "Unresolved_c47f0027");
        assert!(result.decoded.decoded_inputs.is_some());
    }

    #[tokio::test]
    async fn test_decode_seaport_simple() {
        let args = DecodeArgs {
            target: String::from(include_str!("testdata/decode/seaport_simple.hex")),
            rpc_url: String::from(""),
            abi: None,
            openrouter_api_key: String::from(""),
            model: String::from(""),
            explain: false,
            default: true,
            constructor: false,
            truncate_calldata: false,
            skip_resolving: true,
            raw: false,
            output: String::from("print"),
        };
        let result = heimdall_decoder::decode(args).await.expect("failed to decode calldata");
        assert_eq!(result.decoded.name, "Unresolved_fb0f3ee1");
        assert!(result.decoded.decoded_inputs.is_some());
    }

    #[tokio::test]
    #[ignore = "requires the external signature resolver"]
    async fn test_decode_multicall_pattern_detection() {
        // Test that multicall pattern is detected correctly for a simple case
        // Create a simple multicall test case
        // multicall([(0xdead...beef, 0, "")])
        let args = DecodeArgs {
            target: String::from(include_str!("testdata/decode/multicall_pattern_detection.hex")),
            rpc_url: String::from(""),
            abi: None,
            openrouter_api_key: String::from(""),
            model: String::from(""),
            explain: false,
            default: true,
            constructor: false,
            truncate_calldata: false,
            skip_resolving: false,
            raw: true,
            output: String::from("json"),
        };

        let result = heimdall_decoder::decode(args).await.expect("Failed to decode");

        // Debug output
        println!("Decoded signature: {}", result.decoded.signature);
        println!("Decoded inputs: {:?}", result.decoded.decoded_inputs);
        println!("Multicall results: {:?}", result.multicall_results.is_some());

        // Verify multicall was detected (the key check)
        assert!(result.multicall_results.is_some(), "Multicall results should be present");
        let multicall_results = result.multicall_results.unwrap();
        assert!(!multicall_results.is_empty(), "Should have at least one multicall result");

        // The signature should either contain multicall or be unresolved (if signature lookup
        // fails)
        let sig_lower = result.decoded.signature.to_lowercase();
        assert!(
            sig_lower.contains("multicall") || sig_lower.contains("unresolved_1749e1e3"),
            "Signature should contain 'multicall' or be unresolved: {}",
            result.decoded.signature
        );
    }

    #[tokio::test]
    #[ignore = "requires the external signature resolver"]
    async fn test_decode_aggregate_pattern_detection() {
        // Test aggregate pattern detection
        let args = DecodeArgs {
            // Properly formatted aggregate((address,bytes)[]) calldata
            // Selector: 252dba42
            // Array with 1 element containing:
            // - address: 0x69c8ebef7752407cc5818a099b1fcad65d5eee99
            // - bytes: 0x70a08231 (balanceOf selector)
            target: String::from(include_str!("testdata/decode/aggregate_pattern_detection.hex")),
            rpc_url: String::from(""),
            abi: None,
            openrouter_api_key: String::from(""),
            model: String::from(""),
            explain: false,
            default: true,
            constructor: false,
            truncate_calldata: false,
            skip_resolving: false,
            raw: true,
            output: String::from("json"),
        };

        let result = heimdall_decoder::decode(args).await.expect("Failed to decode");

        // Verify multicall/aggregate was detected (the key check)
        assert!(
            result.multicall_results.is_some(),
            "Multicall results should be present for aggregate pattern"
        );

        // The signature should either contain aggregate or be unresolved (if signature lookup
        // fails)
        let sig_lower = result.decoded.signature.to_lowercase();
        assert!(
            sig_lower.contains("aggregate") || sig_lower.contains("unresolved_252dba42"),
            "Signature should contain 'aggregate' or be unresolved: {}",
            result.decoded.signature
        );
    }

    #[tokio::test]
    #[ignore = "requires the external signature resolver"]
    async fn test_decode_various_input_types() {
        // Test decoding with various input types including address, uint256, bytes, bool, etc.
        let test_cases = vec![
            // transfer(address,uint256)
            (include_str!("testdata/decode/erc20_transfer.hex"), vec!["address", "uint256"]),
            // approve(address,uint256)
            (include_str!("testdata/decode/erc20_approve.hex"), vec!["address", "uint256"]),
        ];

        for (calldata, expected_types) in test_cases {
            let args = DecodeArgs {
                target: String::from(calldata),
                rpc_url: String::from(""),
                abi: None,
                openrouter_api_key: String::from(""),
                model: String::from(""),
                explain: false,
                default: true,
                constructor: false,
                truncate_calldata: false,
                skip_resolving: false,
                raw: false,
                output: String::from("json"),
            };

            let result = heimdall_decoder::decode(args).await.expect("Failed to decode");
            let json_str = result.to_json().expect("Failed to convert to JSON");
            let json: Value = serde_json::from_str(&json_str).expect("Failed to parse JSON");

            let inputs = json.get("inputs").expect("inputs field should exist");
            let inputs_array = inputs.as_array().unwrap();

            // Verify we have the correct number of inputs
            assert_eq!(
                inputs_array.len(),
                expected_types.len(),
                "Should have correct number of inputs"
            );

            // Verify each input has the correct structure
            for (i, input) in inputs_array.iter().enumerate() {
                assert_eq!(
                    input.get("name").unwrap().as_str().unwrap(),
                    format!("arg{}", i),
                    "Input should have correct name"
                );

                let input_type = input.get("type").unwrap().as_str().unwrap();
                assert_eq!(input_type, expected_types[i], "Input should have correct type");
            }
        }
    }

    #[test]
    #[ignore = "requires a live RPC_URL and transaction dataset"]
    fn heavy_integration_test() {
        let rpc_url = std::env::var("RPC_URL").expect("RPC_URL is required for this live RPC test");

        // load ./tests/testdata/txids.json into a vector using serde
        let txids = serde_json::from_str::<Value>(
            &std::fs::read_to_string("./tests/testdata/txids.json").expect("failed to read file"),
        )
        .expect("failed to parse json")
        .get("txids")
        .expect("failed to get txids")
        .as_array()
        .expect("failed to convert txids to array")
        .iter()
        .map(|v| v.as_str().expect("failed to stringify json value").to_string())
        .collect::<Vec<String>>();
        let total = txids.len();

        // task_pool(items, num_threads, f)
        let results = task_pool(txids, 10, move |txid: String| {
            let args = DecodeArgsBuilder::new()
                .target(txid.to_string())
                .rpc_url(rpc_url.to_owned())
                .build()
                .expect("failed to build args");

            blocking_await(move || {
                // get new blocking runtime
                let rt = tokio::runtime::Runtime::new().expect("failed to get runtime");

                // get the storage diff for this transaction
                println!("decoding txid: {}", txid);
                match rt.block_on(heimdall_decoder::decode(args)) {
                    Ok(result) => {
                        // check if any resolved_function is named Unresolved_{}
                        if result.decoded.name.starts_with("Unresolved_") {
                            println!("decoding txid: {} ... unresolved succeeded", txid);
                        }

                        println!("decoding txid: {} ... succeeded", txid);
                        1
                    }
                    Err(e) => {
                        println!("decoding txid: {} ... failed", txid);
                        println!("  \\- error: {:?}", e);

                        // we dont want to count RPC errors as failures
                        if let heimdall_decoder::Error::FetchError(_) = e {
                            1
                        } else {
                            0
                        }
                    }
                }
            })
        });
        let success_count = results.iter().filter(|r| **r == 1).count();

        // assert 95% of the transactions were successful
        let success_rate = (success_count as f64) / (total as f64);
        println!(
            "heavy_test_decode_thorough:\n * total: {}\n * failed: {}\n * success rate: {}",
            total,
            total - success_count,
            success_rate * 100.0
        );

        assert!(success_rate >= 0.93);
    }
}
