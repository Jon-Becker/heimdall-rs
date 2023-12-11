#[cfg(test)]
mod integration_tests {
    use clap_verbosity_flag::Verbosity;
    use heimdall_common::utils::{sync::blocking_await, threading::task_pool};
    use heimdall_core::inspect::{InspectArgs, InspectArgsBuilder};
    use serde_json::Value;

    #[tokio::test]
    async fn test_inspect_simple() {
        let args = InspectArgs {
            target: String::from(
                "0xa5f676d0ee4c23cc1ccb0b802be5aaead5827a3337c06e9da8b0a85dfa3e7dd5",
            ),
            verbose: Verbosity::new(0, 0),
            rpc_url: String::from("https://eth.llamarpc.com"),
            default: true,
            transpose_api_key: None,
            name: String::from(""),
            output: String::from("output"),
        };

        let _ = heimdall_core::inspect::inspect(args).await.unwrap();
    }

    #[tokio::test]
    async fn test_inspect_create() {
        let args = InspectArgs {
            target: String::from(
                "0x37321f192623002fc4b398b90ea825c37f81e29526fd355cff93ef6962fc0fba",
            ),
            verbose: Verbosity::new(0, 0),
            rpc_url: String::from("https://eth.llamarpc.com"),
            default: true,
            transpose_api_key: None,
            name: String::from(""),
            output: String::from("output"),
        };

        let _ = heimdall_core::inspect::inspect(args).await.unwrap();
    }

    /// Thorough testing for inspect across a large number of transactions.
    #[test]
    #[ignore]
    fn heavy_test_inspect_thorough() {
        // load ./tests/testdata/txids.json into a vector using serde
        let txids = serde_json::from_str::<Value>(
            &std::fs::read_to_string("./tests/testdata/txids.json").unwrap(),
        )
        .unwrap()
        .get("txids")
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect::<Vec<String>>();
        let total = txids.len();

        // task_pool(items, num_threads, f)
        let results = task_pool(txids, 10, |txid: String| {
            let args = InspectArgsBuilder::new()
                .target(txid.to_string())
                .verbose(Verbosity::new(-1, 0))
                .rpc_url("https://eth.llamarpc.com".to_string())
                .build()
                .unwrap();

            blocking_await(move || {
                // get new blocking runtime
                let rt = tokio::runtime::Runtime::new().unwrap();

                // get the storage diff for this transaction
                println!("inspecting txid: {}", txid);
                match rt.block_on(heimdall_core::inspect::inspect(args)) {
                    Ok(_) => {
                        println!("inspecting txid: {} ... succeeded", txid);
                        1
                    }
                    Err(_) => {
                        println!("inspecting txid: {} ... failed", txid);
                        0
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

        assert!(success_rate >= 0.93);
    }
}
