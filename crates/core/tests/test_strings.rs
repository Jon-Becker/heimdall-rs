//! Integration tests for string extraction from deployed Ethereum contract bytecode.

use heimdall_common::ether::bytecode::{get_bytecode_from_target, write_strings};

async fn extract_fixture(name: &str, min_length: usize) -> Vec<u8> {
    let target = format!("{}/tests/testdata/strings/{name}.hex", env!("CARGO_MANIFEST_DIR"));
    let bytecode =
        get_bytecode_from_target(&target, "", "").await.expect("failed to load bytecode");
    let mut output = Vec::new();
    write_strings(&bytecode, min_length, &mut output).expect("failed to extract strings");
    output
}

#[tokio::test]
async fn test_strings_uniswap_v2() {
    let output = extract_fixture("uniswap_v2_usdc_weth", 4).await;
    let expected: Vec<String> =
        serde_json::from_str(include_str!("testdata/strings/uniswap_v2_usdc_weth.json")).unwrap();
    assert_eq!(output, format!("{}\n", expected.join("\n")).as_bytes());
    let strings = std::str::from_utf8(&output).unwrap();
    assert!(strings.lines().any(|line| line == "UniswapV2: INVALID_SIGNATURE"));
    assert_eq!(strings.lines().filter(|line| *line == "UniswapV2: LOCKED").count(), 5);
}

#[tokio::test]
async fn test_strings_dai() {
    let output = extract_fixture("dai", 4).await;
    let expected: Vec<String> =
        serde_json::from_str(include_str!("testdata/strings/dai.json")).unwrap();
    assert_eq!(output, format!("{}\n", expected.join("\n")).as_bytes());
    let strings = std::str::from_utf8(&output).unwrap();
    assert!(strings.lines().any(|line| line == "Dai/insufficient-balance"));
    assert!(strings.lines().any(|line| line == "Dai/invalid-permit"));
}

#[tokio::test]
async fn test_strings_real_contracts_minimum_length() {
    for (name, expected) in [
        ("uniswap_v2_usdc_weth", include_str!("testdata/strings/uniswap_v2_usdc_weth.json")),
        ("dai", include_str!("testdata/strings/dai.json")),
    ] {
        let expected: Vec<String> = serde_json::from_str(expected).unwrap();
        let expected: String = expected
            .iter()
            .filter(|line| line.len() >= 16)
            .map(|line| format!("{line}\n"))
            .collect();
        assert_eq!(extract_fixture(name, 16).await, expected.as_bytes(), "{name}");
    }
}
