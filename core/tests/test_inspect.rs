#[cfg(test)]
mod integration_tests {
    use std::{
        sync::{Arc, Mutex},
        thread,
        time::{Duration, Instant},
    };

    use clap_verbosity_flag::Verbosity;
    use heimdall_common::utils::threading::task_pool;
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
            transpose_api_key: String::from(""),
            name: String::from(""),
            output: String::from("output"),
            skip_resolving: true,
        };

        let _ = heimdall_core::inspect::inspect(args).await.expect("failed to inspect");
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
            transpose_api_key: String::from(""),
            name: String::from(""),
            output: String::from("output"),
            skip_resolving: true,
        };

        let _ = heimdall_core::inspect::inspect(args).await.expect("failed to inspect");
    }

    /// Thorough testing for inspect across a large number of transactions.
    #[test]
    #[ignore]
    fn heavy_test_inspect_thorough() {
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
        let results = task_pool(txids, 10, |txid: String| {
            let txid_for_thread = txid.clone(); // Clone txid for use in the thread
            let finished = Arc::new(Mutex::new(false)); // Shared state to communicate between threads
            let finished_for_thread = finished.clone();

            let handle = thread::spawn(move || {
                let args = InspectArgsBuilder::new()
                    .target(txid_for_thread.clone())
                    .verbose(Verbosity::new(-1, 0))
                    .rpc_url("https://eth.llamarpc.com".to_string())
                    .skip_resolving(true)
                    .build()
                    .expect("failed to build inspect args");

                let rt = tokio::runtime::Runtime::new().expect("failed to create runtime");
                let result = rt.block_on(heimdall_core::inspect::inspect(args));

                *finished_for_thread
                    .lock()
                    .expect("failed to acquire lock on `finished_for_thread`") = true; // Signal that processing is finished

                result
            });

            let start_time = Instant::now();
            loop {
                if *finished.lock().expect("failed to acquire lock on `finished`") {
                    break // Exit loop if processing is finished
                }

                if start_time.elapsed() > Duration::from_secs(60) {
                    println!("inspecting txid: {} ... slow", txid);
                }

                thread::sleep(Duration::from_millis(100));
            }

            match handle.join().expect("faied to join thread") {
                Ok(_) => 1,
                Err(_) => 0,
            }
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
