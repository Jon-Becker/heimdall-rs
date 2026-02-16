//! Integration tests for Vyper selector detection.
//!
//! Tests that the symbolic execution-based calldata flow tracking correctly
//! identifies function selectors in Vyper-compiled contracts, covering:
//! - Dense selector dispatch (sequential EQ comparisons)
//! - Sparse selector dispatch
//! - Hash bucket dispatch (MOD/AND patterns)
//! - Various Vyper compiler versions

#[cfg(test)]
mod vyper_integration_tests {
    use std::time::Instant;

    use heimdall_decompiler::{decompile, DecompilerArgs, HardFork};

    const RPC_URL_ENV: &str = "RPC_URL";
    const DEFAULT_RPC_URL: &str =
        "https://rpc.ankr.com/eth/cb39792f2f35bd7da677b3a24783fd59c6daffdfc3cba1cca7a4ea93dc2f1c16";

    fn get_rpc_url() -> String {
        std::env::var(RPC_URL_ENV).unwrap_or_else(|_| DEFAULT_RPC_URL.to_string())
    }

    fn make_decompiler_args(target: &str, rpc_url: &str) -> DecompilerArgs {
        DecompilerArgs {
            target: String::from(target),
            rpc_url: String::from(rpc_url),
            default: true,
            skip_resolving: true,
            include_solidity: true,
            include_yul: false,
            output: String::from(""),
            name: String::from(""),
            timeout: 10000,
            abi: None,
            openrouter_api_key: String::from(""),
            model: String::from(""),
            llm_postprocess: false,
            etherscan_api_key: String::from(""),
            hardfork: HardFork::Latest,
        }
    }

    /// Extract function selectors from the decompiled ABI details.
    /// Returns a set of selector strings (e.g., "0x12345678").
    fn extract_selectors_from_abi(abi_details: &serde_json::Value) -> Vec<String> {
        let mut selectors = Vec::new();

        if let Some(arr) = abi_details.as_array() {
            for entry in arr {
                if let Some(selector) = entry.get("selector").and_then(|s| s.as_str()) {
                    selectors.push(selector.to_string());
                }
            }
        }

        selectors
    }

    /// Test: Curve 3Pool Zap (Vyper contract with dense selector dispatch)
    ///
    /// Contract: 0xa79828df1850e8a3a3064576f380d90aecdd3359
    /// Vyper version: 0.2.x
    /// Expected selectors include:
    /// - 0x4515cef3 (add_liquidity)
    /// - 0xc2985578 (remove_liquidity)
    /// - etc.
    #[tokio::test]
    async fn test_vyper_contract_curve_3pool_zap() {
        let rpc_url = get_rpc_url();
        let start = Instant::now();

        let result = decompile(make_decompiler_args(
            "0xa79828df1850e8a3a3064576f380d90aecdd3359",
            &rpc_url,
        ))
        .await
        .expect("failed to decompile Curve 3Pool Zap");

        let selectors = extract_selectors_from_abi(&result.abi_with_details);
        let elapsed = start.elapsed();

        println!("Curve 3Pool Zap: found {} selectors in {:?}", selectors.len(), elapsed);
        for sel in &selectors {
            println!("  {}", sel);
        }

        // Curve 3Pool Zap has multiple public functions
        assert!(
            !selectors.is_empty(),
            "Expected to find selectors for Curve 3Pool Zap, but found none"
        );

        // The contract should have at least a few selectors
        assert!(
            selectors.len() >= 2,
            "Expected at least 2 selectors for Curve 3Pool Zap, found {}",
            selectors.len()
        );

        // Verify source was generated
        assert!(
            result.source.is_some(),
            "Expected decompiled source output"
        );
    }

    /// Test: Curve Vyper 2 - Dense selector section pattern
    ///
    /// Contract: 0xd446A98F88E1d053d1F64986E3Ed083bb1Ab7E5A
    /// Tests the dense selector section dispatch pattern used by newer Vyper versions
    #[tokio::test]
    async fn test_vyper_contract_curve_vyper_2() {
        let rpc_url = get_rpc_url();
        let start = Instant::now();

        let result = decompile(make_decompiler_args(
            "0xd446A98F88E1d053d1F64986E3Ed083bb1Ab7E5A",
            &rpc_url,
        ))
        .await
        .expect("failed to decompile Curve Vyper 2");

        let selectors = extract_selectors_from_abi(&result.abi_with_details);
        let elapsed = start.elapsed();

        println!(
            "Curve Vyper 2: found {} selectors in {:?}",
            selectors.len(),
            elapsed
        );
        for sel in &selectors {
            println!("  {}", sel);
        }

        assert!(
            !selectors.is_empty(),
            "Expected to find selectors for Curve Vyper 2, but found none"
        );

        assert!(
            selectors.len() >= 2,
            "Expected at least 2 selectors for Curve Vyper 2, found {}",
            selectors.len()
        );
    }

    /// Test: Curve Vyper 1 - Sparse selector section pattern
    ///
    /// Contract: 0xE84f5b1582BA325fDf9cE6B0c1F087ccfC924e54
    /// Tests the sparse selector section dispatch pattern
    #[tokio::test]
    async fn test_vyper_contract_curve_vyper_1() {
        let rpc_url = get_rpc_url();
        let start = Instant::now();

        let result = decompile(make_decompiler_args(
            "0xE84f5b1582BA325fDf9cE6B0c1F087ccfC924e54",
            &rpc_url,
        ))
        .await
        .expect("failed to decompile Curve Vyper 1");

        let selectors = extract_selectors_from_abi(&result.abi_with_details);
        let elapsed = start.elapsed();

        println!(
            "Curve Vyper 1: found {} selectors in {:?}",
            selectors.len(),
            elapsed
        );
        for sel in &selectors {
            println!("  {}", sel);
        }

        assert!(
            !selectors.is_empty(),
            "Expected to find selectors for Curve Vyper 1, but found none"
        );

        assert!(
            selectors.len() >= 2,
            "Expected at least 2 selectors for Curve Vyper 1, found {}",
            selectors.len()
        );
    }

    /// Test: Curve General - Hash bucket dispatch pattern
    ///
    /// Contract: 0x838af967537350d2c44abb8c010e49e32673ab94
    /// Tests the hash bucket dispatch pattern (MOD/AND operations on selector)
    #[tokio::test]
    async fn test_vyper_contract_curve_general() {
        let rpc_url = get_rpc_url();
        let start = Instant::now();

        let result = decompile(make_decompiler_args(
            "0x838af967537350d2c44abb8c010e49e32673ab94",
            &rpc_url,
        ))
        .await
        .expect("failed to decompile Curve General");

        let selectors = extract_selectors_from_abi(&result.abi_with_details);
        let elapsed = start.elapsed();

        println!(
            "Curve General: found {} selectors in {:?}",
            selectors.len(),
            elapsed
        );
        for sel in &selectors {
            println!("  {}", sel);
        }

        assert!(
            !selectors.is_empty(),
            "Expected to find selectors for Curve General, but found none"
        );

        assert!(
            selectors.len() >= 2,
            "Expected at least 2 selectors for Curve General, found {}",
            selectors.len()
        );
    }

    /// Test: Lido stETH/ETH Curve Pool - Different Vyper version
    ///
    /// Contract: 0xdc24316b9ae028f1497c275eb9192a3ea0f67022
    /// Tests selector detection with a different Vyper compiler version
    #[tokio::test]
    async fn test_vyper_contract_lido_curve_pool() {
        let rpc_url = get_rpc_url();
        let start = Instant::now();

        let result = decompile(make_decompiler_args(
            "0xdc24316b9ae028f1497c275eb9192a3ea0f67022",
            &rpc_url,
        ))
        .await
        .expect("failed to decompile Lido Curve Pool");

        let selectors = extract_selectors_from_abi(&result.abi_with_details);
        let elapsed = start.elapsed();

        println!(
            "Lido Curve Pool: found {} selectors in {:?}",
            selectors.len(),
            elapsed
        );
        for sel in &selectors {
            println!("  {}", sel);
        }

        assert!(
            !selectors.is_empty(),
            "Expected to find selectors for Lido Curve Pool, but found none"
        );

        // Lido Curve Pool is a fairly large contract with many functions
        assert!(
            selectors.len() >= 2,
            "Expected at least 2 selectors for Lido Curve Pool, found {}",
            selectors.len()
        );
    }

    /// Test: Verify no regression for Solidity contracts
    ///
    /// Ensure that adding Vyper support doesn't break Solidity selector detection.
    /// Uses WETH contract which is known to have 8 functions.
    #[tokio::test]
    async fn test_vyper_no_regression_solidity_weth() {
        let rpc_url = get_rpc_url();

        let result = decompile(make_decompiler_args(
            "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2",
            &rpc_url,
        ))
        .await
        .expect("failed to decompile WETH");

        let selectors = extract_selectors_from_abi(&result.abi_with_details);
        println!("WETH (Solidity): found {} selectors", selectors.len());

        // WETH should have its known selectors
        assert!(
            selectors.len() >= 6,
            "Expected at least 6 selectors for WETH, found {}. Solidity detection may be broken.",
            selectors.len()
        );
    }
}
