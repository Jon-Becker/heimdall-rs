#[cfg(test)]
mod benchmark {
    use clap_verbosity_flag::Verbosity;
    use heimdall_common::testing::benchmarks::benchmark;

    use heimdall::decompile::DecompilerArgs;

    #[test]
    fn benchmark_decompile_complex() {
        fn bench() {
            let args = DecompilerArgs {
                target: String::from("0xE90d8Fb7B79C8930B5C8891e61c298b412a6e81a"),
                verbose: Verbosity::new(0, 0),
                output: String::from(""),
                rpc_url: String::from("https://eth.llamarpc.com"),
                default: true,
                skip_resolving: true,
                include_solidity: true,
                include_yul: false,
            };
            heimdall::decompile::decompile(args)
        }

        benchmark("benchmark_decompile_complex", 100, bench)
    }

    #[test]
    fn benchmark_decompile_simple() {
        fn bench() {
            let args = DecompilerArgs {
                target: String::from("0x1bf797219482a29013d804ad96d1c6f84fba4c45"),
                verbose: Verbosity::new(0, 0),
                output: String::from(""),
                rpc_url: String::from("https://eth.llamarpc.com"),
                default: true,
                skip_resolving: true,
                include_solidity: true,
                include_yul: false,
            };
            heimdall::decompile::decompile(args)
        }

        benchmark("benchmark_decompile_simple", 100, bench)
    }

    #[test]
    fn benchmark_build_abi_simple() {
        fn bench() {
            let args = DecompilerArgs {
                target: String::from("0x1bf797219482a29013d804ad96d1c6f84fba4c45"),
                verbose: Verbosity::new(0, 0),
                output: String::from(""),
                rpc_url: String::from("https://eth.llamarpc.com"),
                default: true,
                skip_resolving: true,
                include_solidity: false,
                include_yul: false,
            };
            heimdall::decompile::decompile(args)
        }

        benchmark("benchmark_build_abi_simple", 100, bench)
    }

    #[test]
    fn benchmark_build_abi_complex() {
        fn bench() {
            let args = DecompilerArgs {
                target: String::from("0xE90d8Fb7B79C8930B5C8891e61c298b412a6e81a"),
                verbose: Verbosity::new(0, 0),
                output: String::from(""),
                rpc_url: String::from("https://eth.llamarpc.com"),
                default: true,
                skip_resolving: true,
                include_solidity: false,
                include_yul: false,
            };
            heimdall::decompile::decompile(args)
        }

        benchmark("benchmark_build_abi_complex", 100, bench)
    }
}

#[cfg(test)]
mod tests {
    use heimdall::decompile::DecompileBuilder;
    use heimdall_common::io::file::{delete_path, read_file};

    #[test]
    fn test_decompile_precompile() {
        DecompileBuilder::new("0x1bf797219482a29013d804ad96d1c6f84fba4c45")
            .output("./output/tests/decompile/test1")
            .rpc("https://eth.llamarpc.com")
            .include_sol(true)
            .default(true)
            .skip_resolving(true)
            .decompile();

        // throws if not found. asserts success
        let output = read_file(&String::from("./output/tests/decompile/test1/decompiled.sol"));

        // assert that the output is correct
        for line in &["function Unresolved_19045a25(bytes memory arg0, bytes memory arg1) public payable returns (address) {",
            " = ecrecover("] {
            println!("{line}");
            assert!(output.contains(line));
        }

        // drop path
        delete_path(&String::from("./output/tests/decompile/test1"));
    }

    #[test]
    fn test_decompile_weth() {
        DecompileBuilder::new("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2")
            .output("./output/tests/decompile/test2")
            .rpc("https://eth.llamarpc.com")
            .include_sol(true)
            .default(true)
            .skip_resolving(true)
            .decompile();

        // throws if not found. asserts success
        let output = read_file(&String::from("./output/tests/decompile/test2/decompiled.sol"));

        // assert that the output is correct
        for line in &["function Unresolved_06fdde03() public view returns (bytes memory) {",
            "function Unresolved_095ea7b3(address arg0, bytes memory arg1) public returns (bool) {",
            "function Unresolved_18160ddd() public view returns (address) {",
            "function Unresolved_23b872dd(address arg0, address arg1, bytes memory arg2) public returns (bool) {",
            "function Unresolved_2e1a7d4d(bool arg0) public {",
            "function Unresolved_313ce567() public view returns (bool) {",
            "function Unresolved_70a08231(address arg0) public view returns (uint256) {",
            "function Unresolved_95d89b41() public view returns (bytes memory) {",
            "function Unresolved_a9059cbb(address arg0, bytes memory arg1) public returns (bool) {",
            "function Unresolved_d0e30db0() public payable {",
            "function Unresolved_dd62ed3e(address arg0, address arg1) public view returns (uint256) {"] {
            println!("{line}");
            assert!(output.contains(line));
        }

        // drop path
        delete_path(&String::from("./output/tests/decompile/test2"));
    }

    #[test]
    fn test_decompile_ctf() {
        DecompileBuilder::new("0x9f00c43700bc0000Ff91bE00841F8e04c0495000")
            .output("./output/tests/decompile/test3")
            .rpc("https://eth.llamarpc.com")
            .include_sol(true)
            .default(true)
            .skip_resolving(true)
            .decompile();

        // throws if not found. asserts success
        let output = read_file(&String::from("./output/tests/decompile/test3/decompiled.sol"));

        // assert that the output is correct
        for line in &["function Unresolved_2fa61cd8(address arg0) public view payable returns (uint16) {",
            "function Unresolved_41161b10(bytes memory arg0, address arg1) public payable returns (bool) {",
            "function Unresolved_06fdde03() public pure payable returns (bytes memory) {"] {
            println!("{line}");
            assert!(output.contains(line));
        }

        // drop path
        delete_path(&String::from("./output/tests/decompile/test3"));
    }
}
