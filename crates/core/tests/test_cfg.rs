//! Integration tests for CFG (Control Flow Graph) functionality.

#[cfg(test)]
mod integration_tests {
    use memory_stats::memory_stats;
    use std::path::PathBuf;

    use heimdall_cfg::{cfg, CfgArgs, CfgArgsBuilder, HardFork};
    use petgraph::{dot::Dot, graph::NodeIndex, visit::EdgeRef};
    use serde_json::Value;

    #[tokio::test]
    async fn test_cfg_simple() {
        let rpc_url = std::env::var("RPC_URL").unwrap_or_else(|_| {
            println!("RPC_URL not set, skipping test");
            std::process::exit(0);
        });

        let result = heimdall_cfg::cfg(CfgArgs {
            target: String::from("0x1bf797219482a29013d804ad96d1c6f84fba4c45"),
            rpc_url,
            default: true,
            color_edges: false,
            output: String::from(""),
            name: String::from(""),
            timeout: 10000,
            hardfork: HardFork::Latest,
            etherscan_api_key: String::from(""),
        })
        .await
        .expect("failed to generate cfg");

        let output: String = format!("{}", Dot::with_config(&result.graph, &[]));

        for line in &[
            String::from("0 [ label = \"0x01 PUSH20 0x1bf797219482a29013d804ad96d1c6f84fba4c45\\l0x16 ADDRESS \\l0x17 EQ \\l0x18 PUSH1 0x80\\l0x1a PUSH1 0x40\\l0x1c MSTORE \\l0x1d PUSH1 0x04\\l0x1f CALLDATASIZE \\l0x20 LT \\l0x21 PUSH2 0x58\\l0x24 JUMPI \\l\" ]"),
            String::from("0 -> 13 []")
        ] {
            output.contains(line);
        }
    }

    #[tokio::test]
    async fn test_cfg_complex() {
        let rpc_url = std::env::var("RPC_URL").unwrap_or_else(|_| {
            println!("RPC_URL not set, skipping test");
            std::process::exit(0);
        });

        let result = heimdall_cfg::cfg(CfgArgs {
            target: String::from("0xE90d8Fb7B79C8930B5C8891e61c298b412a6e81a"),
            rpc_url,
            default: true,
            color_edges: false,
            output: String::from(""),
            name: String::from(""),
            timeout: 10000,
            hardfork: HardFork::Latest,
            etherscan_api_key: String::from(""),
        })
        .await
        .expect("failed to generate cfg");

        let output = format!("{}", Dot::with_config(&result.graph, &[]));

        for line in &[String::from("\"0x039f JUMPDEST \\l0x03a0 STOP \\l\"")] {
            assert!(output.contains(line))
        }
    }

    #[tokio::test]
    async fn test_cfg_links_fallback_block() {
        // regression test for https://github.com/Jon-Becker/heimdall-rs/issues/627
        // the entry node dispatches to the fallback handler when `CALLDATASIZE < 4`, but the
        // edge into that block was previously dropped when the block had already been visited
        // through another path (e.g. the dispatcher's fall-through).
        let rpc_url = std::env::var("RPC_URL").unwrap_or_else(|_| {
            println!("RPC_URL not set, skipping test");
            std::process::exit(0);
        });

        // WETH (0xc02a...cc2) exhibits the missing fallback edge from the entry node.
        let result = heimdall_cfg::cfg(CfgArgs {
            target: String::from("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"),
            rpc_url,
            default: true,
            color_edges: false,
            output: String::from(""),
            name: String::from(""),
            timeout: 10000,
            hardfork: HardFork::Latest,
            etherscan_api_key: String::from(""),
        })
        .await
        .expect("failed to generate cfg");

        // the entry node is always the first node added to the graph.
        let entry = NodeIndex::new(0);
        assert!(
            result.graph[entry].contains("CALLDATASIZE"),
            "entry node should contain the calldatasize dispatch check"
        );

        // the entry node must link to the fallback handler at 0xaf.
        let links_to_fallback = result
            .graph
            .edges(entry)
            .any(|edge| result.graph[edge.target()].starts_with("0xaf JUMPDEST"));
        assert!(
            links_to_fallback,
            "entry node should link to the fallback block (0xaf) when calldatasize < 4"
        );
    }

    #[tokio::test]
    async fn test_cfg_auto_hardfork() {
        let rpc_url = std::env::var("RPC_URL").unwrap_or_else(|_| {
            println!("RPC_URL not set, skipping test");
            std::process::exit(0);
        });

        // Test auto hardfork detection with a real contract
        let result = heimdall_cfg::cfg(CfgArgs {
            target: String::from("0x1bf797219482a29013d804ad96d1c6f84fba4c45"),
            rpc_url,
            default: true,
            color_edges: false,
            output: String::from(""),
            name: String::from(""),
            timeout: 10000,
            hardfork: HardFork::Auto,
            etherscan_api_key: String::from(""),
        })
        .await
        .expect("failed to generate cfg with auto hardfork");

        let output: String = format!("{}", Dot::with_config(&result.graph, &[]));

        // Verify the CFG was generated successfully
        assert!(!output.is_empty());
        assert!(output.contains("digraph"));
    }

    #[tokio::test]
    async fn test_cfg_auto_hardfork_fallback() {
        // When target is bytecode (not an address), auto hardfork should fall back to Latest
        let bytecode = "6080604052";

        let result = heimdall_cfg::cfg(CfgArgs {
            target: bytecode.to_string(),
            rpc_url: String::from(""),
            default: true,
            color_edges: false,
            output: String::from(""),
            name: String::from(""),
            timeout: 10000,
            hardfork: HardFork::Auto,
            etherscan_api_key: String::from(""),
        })
        .await
        .expect("failed to generate cfg with auto hardfork fallback");

        let output: String = format!("{}", Dot::with_config(&result.graph, &[]));

        // Verify the CFG was generated successfully
        assert!(!output.is_empty());
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

        let mut success_count = 0;
        let mut fail_count = 0;

        for (contract_address, bytecode) in contracts {
            println!("Generating Cfg for contract {contract_address}");
            let args = CfgArgsBuilder::new()
                .target(bytecode)
                .timeout(10000)
                .build()
                .expect("failed to build args");

            match cfg(args).await {
                Ok(_) => {
                    success_count += 1;
                }
                Err(_) => {
                    fail_count += 1;
                }
            };

            if let Some(usage) = memory_stats() {
                println!("Current physical memory usage: {}", usage.physical_mem);
                println!("Current virtual memory usage: {}", usage.virtual_mem);
            } else {
                println!("Couldn't get the current memory usage :(");
            }
        }

        // assert 99% success rate
        assert!(
            success_count as f64 / (success_count + fail_count) as f64 > 0.99,
            "success rate is less than 99%"
        );
    }
}
