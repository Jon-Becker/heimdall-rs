//! Integration tests for inspect functionality.

#[cfg(test)]
mod integration_tests {
    use heimdall_common::utils::{sync::blocking_await, threading::task_pool};
    use heimdall_inspect::{InspectArgs, InspectArgsBuilder};
    use serde_json::Value;

    #[tokio::test]
    async fn test_inspect_simple() {
        let rpc_url = std::env::var("RPC_URL").unwrap_or_else(|_| {
            println!("RPC_URL not set, skipping test");
            std::process::exit(0);
        });

        let args = InspectArgs {
            abi: None,
            target: String::from(
                "0xa5f676d0ee4c23cc1ccb0b802be5aaead5827a3337c06e9da8b0a85dfa3e7dd5",
            ),
            rpc_url,
            default: true,
            transpose_api_key: String::from(""),
            name: String::from(""),
            output: String::from("output"),
            skip_resolving: true,
        };

        let _ = heimdall_inspect::inspect(args).await.expect("failed to inspect");
    }

    #[tokio::test]
    async fn test_inspect_create() {
        let rpc_url = std::env::var("RPC_URL").unwrap_or_else(|_| {
            println!("RPC_URL not set, skipping test");
            std::process::exit(0);
        });

        let args = InspectArgs {
            abi: None,
            target: String::from(
                "0x37321f192623002fc4b398b90ea825c37f81e29526fd355cff93ef6962fc0fba",
            ),
            rpc_url,
            default: true,
            transpose_api_key: String::from(""),
            name: String::from(""),
            output: String::from("output"),
            skip_resolving: true,
        };

        let _ = heimdall_inspect::inspect(args).await.expect("failed to inspect");
    }

    /// Thorough testing for inspect across a large number of transactions.
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
            let args = InspectArgsBuilder::new()
                .target(txid.to_string())
                .rpc_url(rpc_url.to_string())
                .build()
                .expect("failed to build args");

            blocking_await(move || {
                // get new blocking runtime
                let rt = tokio::runtime::Runtime::new().expect("failed to get runtime");

                // get the storage diff for this transaction
                println!("inspecting txid: {}", txid);
                match rt.block_on(heimdall_inspect::inspect(args)) {
                    Ok(_) => {
                        println!("inspecting txid: {} ... succeeded", txid);
                        1
                    }
                    Err(e) => {
                        println!("inspecting txid: {} ... failed", txid);
                        println!("  \\- error: {:?}", e);

                        // we dont want to count RPC errors as failures
                        if let heimdall_inspect::Error::FetchError(_) = e {
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
            "heavy_test_inspect_thorough:\n * total: {}\n * failed: {}\n * success rate: {}",
            total,
            total - success_count,
            success_rate * 100.0
        );

        assert!(success_rate >= 0.92);
    }
}
