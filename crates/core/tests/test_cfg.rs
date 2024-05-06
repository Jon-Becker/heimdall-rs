#[cfg(test)]
mod benchmark {
    use heimdall_common::utils::testing::benchmarks::async_bench;

    use heimdall_cfg::CFGArgs;

    #[tokio::test]
    async fn benchmark_generate_cfg_simple() {
        async fn bench() {
            let args = CFGArgs {
                target: String::from("0x1bf797219482a29013d804ad96d1c6f84fba4c45"),
                rpc_url: String::from("https://eth.llamarpc.com"),
                default: true,
                color_edges: false,
                output: String::from(""),
                name: String::from(""),
                timeout: 10000,
            };
            let _ = heimdall_cfg::cfg(args).await;
        }

        async_bench("benchmark_generate_cfg_simple", 100, bench).await;
    }

    #[tokio::test]
    async fn benchmark_generate_cfg_complex() {
        async fn bench() {
            let args = CFGArgs {
                target: String::from("0xE90d8Fb7B79C8930B5C8891e61c298b412a6e81a"),
                rpc_url: String::from("https://eth.llamarpc.com"),
                default: true,
                color_edges: false,
                output: String::from(""),
                name: String::from(""),
                timeout: 10000,
            };
            let _ = heimdall_cfg::cfg(args).await;
        }

        async_bench("benchmark_generate_cfg_complex", 100, bench).await;
    }
}

#[cfg(test)]
mod integration_tests {
    use heimdall_cfg::CFGArgs;
    use petgraph::dot::Dot;

    #[tokio::test]
    async fn test_cfg_simple() {
        let result = heimdall_cfg::cfg(CFGArgs {
            target: String::from("0x1bf797219482a29013d804ad96d1c6f84fba4c45"),
            rpc_url: String::from("https://eth.llamarpc.com"),
            default: true,
            color_edges: false,
            output: String::from(""),
            name: String::from(""),
            timeout: 10000,
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
        let result = heimdall_cfg::cfg(CFGArgs {
            target: String::from("0xE90d8Fb7B79C8930B5C8891e61c298b412a6e81a"),
            rpc_url: String::from("https://eth.llamarpc.com"),
            default: true,
            color_edges: false,
            output: String::from(""),
            name: String::from(""),
            timeout: 10000,
        })
        .await
        .expect("failed to generate cfg");

        let output = format!("{}", Dot::with_config(&result.graph, &[]));

        for line in &[String::from("\"0x03a0 JUMPDEST \\l0x03a1 STOP \\l\"")] {
            assert!(output.contains(line))
        }
    }
}
