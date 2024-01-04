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
            transpose_api_key: String::from(""),
            name: String::from(""),
            output: String::from("output"),
            skip_resolving: true,
        };

        let _ = heimdall_core::inspect::inspect(args).await.unwrap();
    }

    #[tokio::test]
    async fn test_inspect_compelex_seaport() {
        let args = InspectArgs {
            target: String::from(
                "0x7124bd182ec69053ca4c9b643afcde39ccdb3b926db9fc6756660075e504f186",
            ),
            verbose: Verbosity::new(0, 0),
            // override the default rpc url for this test, since it's a huge transaction
            rpc_url: String::from("https://eth.llamarpc.com"),
            default: true,
            transpose_api_key: String::from(""),
            name: String::from(""),
            output: String::from("output"),
            skip_resolving: true,
        };

        let result = heimdall_core::inspect::inspect(args).await.unwrap();
        println!("{:#?}", result);

        // DecodedLog {
        //     address: 0x00000000000000adc04c56bf30ac9d3c0aaf14dc,
        //     topics: [
        //         0x9d9af8e38d66c62e2c12f0225249fd9d721c54b83f48d9352c97c6cacdcb6f31,
        //         0x000000000000000000000000a15ceffae873189757f94e497f649b4adda2fecf,
        //         0x000000000000000000000000004c00500000ad104d7dbd00e3ae0a5c00560c00,
        //     ],
        //     data: Bytes(0x312fb0b2d703f4bc534fe4a29c2338dea4111020c1d7d8e85e9a403f84ad0a1900000000000000000000000034a690c0372dd7a21f28b55edb3bbea4926ee8a60000000000000000000000000000000000000000000000000000000000000080000000000000000000000000000000000000000000000000000000000000012000000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000000002000000000000000000000000329fa32f6520fb67bb3c1fc3a6909beb8239544c0000000000000000000000000000000000000000000000000000000000001d66000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000097ebeac26b5e00000000000000000000000000a15ceffae873189757f94e497f649b4adda2fecf0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000c36fd017b2000000000000000000000000000000a26b00c1f0df003000390027140000faa719),
        //     resolved_event: None, // this should be Some(ResolvedLog { ... })
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
                    .unwrap();

                let rt = tokio::runtime::Runtime::new().unwrap();
                let result = rt.block_on(heimdall_core::inspect::inspect(args));

                *finished_for_thread.lock().unwrap() = true; // Signal that processing is finished

                result
            });

            let start_time = Instant::now();
            loop {
                if *finished.lock().unwrap() {
                    break // Exit loop if processing is finished
                }

                if start_time.elapsed() > Duration::from_secs(60) {
                    println!("inspecting txid: {} ... slow", txid);
                }

                thread::sleep(Duration::from_millis(100));
            }

            match handle.join().unwrap() {
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
