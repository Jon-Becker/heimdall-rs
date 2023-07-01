#[cfg(test)]
mod benchmark {
    use clap_verbosity_flag::Verbosity;
    use heimdall_common::testing::benchmarks::benchmark;

    use heimdall::cfg::CFGArgs;

    #[test]
    fn benchmark_generate_cfg_simple() {
        fn bench() {
            let args = CFGArgs {
                target: String::from("0x1bf797219482a29013d804ad96d1c6f84fba4c45"),
                verbose: Verbosity::new(0, 0),
                output: String::from(""),
                rpc_url: String::from("https://eth.llamarpc.com"),
                default: true,
                color_edges: false,
                format: String::from(""),
            };
            heimdall::cfg::cfg(args)
        }

        benchmark("benchmark_generate_cfg_simple", 100, bench)
    }

    #[test]
    fn benchmark_generate_cfg_complex() {
        fn bench() {
            let args = CFGArgs {
                target: String::from("0xE90d8Fb7B79C8930B5C8891e61c298b412a6e81a"),
                verbose: Verbosity::new(0, 0),
                output: String::from(""),
                rpc_url: String::from("https://eth.llamarpc.com"),
                default: true,
                color_edges: false,
                format: String::from(""),
            };
            heimdall::cfg::cfg(args)
        }

        benchmark("benchmark_generate_cfg_complex", 100, bench)
    }
}

#[cfg(test)]
mod integration_tests {
    use heimdall_common::io::file::{delete_path, read_file};

    use heimdall::cfg::CFGBuilder;

    #[test]
    fn test_cfg_simple() {
        let expected_lines = vec![
            String::from("0 [ label = \"0x01 PUSH20 0x1bf797219482a29013d804ad96d1c6f84fba4c45\\l0x16 ADDRESS \\l0x17 EQ \\l0x18 PUSH1 0x80\\l0x1a PUSH1 0x40\\l0x1c MSTORE \\l0x1d PUSH1 0x04\\l0x1f CALLDATASIZE \\l0x20 LT \\l0x21 PUSH2 0x58\\l0x24 JUMPI \\l\" ]"),
            String::from("0 -> 13 []")
        ];

        CFGBuilder::new("0x1bf797219482a29013d804ad96d1c6f84fba4c45")
            .rpc("https://eth.llamarpc.com")
            .output("./output/tests/cfg/test1")
            .generate();

        let dot = read_file(&String::from("./output/tests/cfg/test1/cfg.dot"));

        for line in expected_lines {
            assert!(dot.contains(&line))
        }

        delete_path(&String::from("./output/tests/cfg/test1"));
    }

    #[test]
    fn test_cfg_complex() {
        let expected_lines = vec![
            String::from("471 [ label = \"0x03a0 JUMPDEST \\l0x03a1 STOP \\l\" ]"),
            String::from("5 -> 7 []"),
        ];

        CFGBuilder::new("0xE90d8Fb7B79C8930B5C8891e61c298b412a6e81a")
            .rpc("https://eth.llamarpc.com")
            .output("./output/tests/cfg/test2/")
            .generate();

        let dot = read_file(&String::from("./output/tests/cfg/test2/cfg.dot"));

        for line in expected_lines {
            assert!(dot.contains(&line))
        }

        delete_path(&String::from("./output/tests/cfg/test2"));
    }
}
