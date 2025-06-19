//! Integration tests for decode functionality.

mod integration_tests {
    use heimdall_common::utils::{sync::blocking_await, threading::task_pool};
    use heimdall_decoder::{DecodeArgs, DecodeArgsBuilder};
    use serde_json::Value;

    #[tokio::test]
    async fn test_decode_transfer() {
        let args = DecodeArgs {
            abi: None,
            target: String::from("0xc47f00270000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000000b6a6265636b65722e657468000000000000000000000000000000000000000000"),
            rpc_url: String::from(""),
            openai_api_key: String::from(""),
            explain: false,
            default: true,
            constructor: false,
            truncate_calldata: false,
            skip_resolving: false,
            raw: false,
            output: String::from("print"),
        };
        let _ = heimdall_decoder::decode(args).await;
    }

    #[tokio::test]
    async fn test_decode_seaport_simple() {
        let args = DecodeArgs {
            target: String::from("0xfb0f3ee100000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000ec9c58de0a8000000000000000000000000000d2f8a98bde7c701ae961d10d0d1fc3a751be737f000000000000000000000000004c00500000ad104d7dbd00e3ae0a5c00560c000000000000000000000000005008c2a3af41024e9f0bd0432df4f75828602598000000000000000000000000000000000000000000000000000000000000110600000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000006358934b00000000000000000000000000000000000000000000000000000000637e22710000000000000000000000000000000000000000000000000000000000000000360c6ebe000000000000000000000000000000000000000038844ef19f04aecf0000007b02230091a7ed01230072f7006a004d60a8d4e71d599b8104250f000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000024000000000000000000000000000000000000000000000000000000000000002e0000000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000066517289880000000000000000000000000000000a26b00c1f0df003000390027140000faa719000000000000000000000000000000000000000000000000000cca2e51310000000000000000000000000000cecf12f47d2896c90f6e19b7376fa3b169fabd920000000000000000000000000000000000000000000000000000000000000041447858c6d8251fb8ffba546bedb410457ff77148fdf59ac8e046993936a134b028f535c5b1f760508b6e0c3c18d44927d82da0502c66688c0dc961a434a9b0071c00000000000000000000000000000000000000000000000000000000000000"),
            rpc_url: String::from(""),
            abi: None,
            openai_api_key: String::from(""),
            explain: false,
            default: true,
            constructor: false,
            truncate_calldata: false,
            skip_resolving: false,
            raw: false,
            output: String::from("print"),

        };
        let _ = heimdall_decoder::decode(args).await;
    }

    #[tokio::test]
    async fn test_decode_multicall_pattern_detection() {
        // Test that multicall pattern is detected correctly for a simple case
        // Create a simple multicall test case
        // multicall([(0xdead...beef, 0, "")])
        let args = DecodeArgs {
            target: String::from("0x1749e1e30000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000600000000000000000000000000000000000000000000000000000000000000000"),
            rpc_url: String::from(""),
            abi: None,
            openai_api_key: String::from(""),
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
        assert!(multicall_results.len() >= 1, "Should have at least one multicall result");

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
    async fn test_decode_aggregate_pattern_detection() {
        // Test aggregate pattern detection
        let args = DecodeArgs {
            // Properly formatted aggregate((address,bytes)[]) calldata
            // Selector: 252dba42
            // Array with 1 element containing:
            // - address: 0x69c8ebef7752407cc5818a099b1fcad65d5eee99
            // - bytes: 0x70a08231 (balanceOf selector)
            target: String::from("0x252dba4200000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000002000000000000000000000000069c8ebef7752407cc5818a099b1fcad65d5eee990000000000000000000000000000000000000000000000000000000000000040000000000000000000000000000000000000000000000000000000000000000470a0823100000000000000000000000000000000000000000000000000000000"),
            rpc_url: String::from(""),
            abi: None,
            openai_api_key: String::from(""),
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

    #[test]
    #[ignore]
    fn heavy_integration_test() {
        let rpc_url = std::env::var("RPC_URL").unwrap_or_else(|_| {
            println!("RPC_URL not set, skipping test");
            std::process::exit(0);
        });

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
