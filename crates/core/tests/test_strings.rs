//! Integration tests for string extraction from deployed Ethereum contract bytecode.

use std::num::NonZeroUsize;

use heimdall_core::heimdall_strings::{strings, StringsArgsBuilder};

async fn extract_fixture(name: &str, min_length: usize, full_scan: bool) -> Vec<u8> {
    let target = format!("{}/tests/testdata/strings/{name}.hex", env!("CARGO_MANIFEST_DIR"));
    let args = StringsArgsBuilder::new()
        .target(target)
        .min_length(NonZeroUsize::new(min_length).unwrap())
        .full_scan(full_scan)
        .build()
        .unwrap();
    let mut output = Vec::new();
    strings(&args, &mut output).await.expect("failed to extract strings");
    output
}

#[tokio::test]
async fn test_strings_uniswap_v2() {
    let output = extract_fixture("uniswap_v2_usdc_weth", 4, false).await;
    let expected: Vec<String> =
        serde_json::from_str(include_str!("testdata/strings/uniswap_v2_usdc_weth.json")).unwrap();
    assert_eq!(output, format!("{}\n", expected.join("\n")).as_bytes());
    let strings = std::str::from_utf8(&output).unwrap();
    assert!(strings.lines().any(|line| line == "UniswapV2: INVALID_SIGNATURE"));
    assert_eq!(strings.lines().filter(|line| *line == "UniswapV2: LOCKED").count(), 5);
}

#[tokio::test]
async fn test_strings_dai() {
    let output = extract_fixture("dai", 4, false).await;
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
        assert_eq!(extract_fixture(name, 16, false).await, expected.as_bytes(), "{name}");
    }
}

#[tokio::test]
async fn test_strings_real_contracts_full_scan() {
    for (name, expected) in [
        (
            "uniswap_v2_usdc_weth",
            include_str!("testdata/strings/uniswap_v2_usdc_weth.full_scan.json"),
        ),
        ("dai", include_str!("testdata/strings/dai.full_scan.json")),
    ] {
        let expected: Vec<String> = serde_json::from_str(expected).unwrap();
        assert_eq!(
            extract_fixture(name, 4, true).await,
            format!("{}\n", expected.join("\n")).as_bytes(),
            "{name}"
        );
    }
}

#[tokio::test]
async fn test_strings_library_defaults() {
    let args = StringsArgsBuilder::new()
        .target("0x41424344006468656c6c6f0062616263".to_string())
        .build()
        .unwrap();
    let mut output = Vec::new();
    strings(&args, &mut output).await.unwrap();
    assert_eq!(output, b"hello\n");
}

#[tokio::test]
async fn test_strings_library_fetch_error() {
    let args = StringsArgsBuilder::new().target("0xzz".to_string()).build().unwrap();
    let mut output = Vec::new();
    let error = strings(&args, &mut output).await.unwrap_err();
    assert!(matches!(error, heimdall_core::heimdall_strings::Error::FetchError(_)));
    assert!(output.is_empty());
}

#[tokio::test]
async fn test_strings_library_write_error() {
    let args = StringsArgsBuilder::new().target("0x6468656c6c6f".to_string()).build().unwrap();
    let mut output = [0; 2];
    let error = strings(&args, &mut output.as_mut_slice()).await.unwrap_err();
    match error {
        heimdall_core::heimdall_strings::Error::WriteError(error) => {
            assert_eq!(error.kind(), std::io::ErrorKind::WriteZero);
        }
        error => panic!("expected a write error, got {error}"),
    }
}

#[tokio::test]
async fn test_strings_recognized_selectors_and_packed_text() {
    // Ethereum mainnet block 26,063,763. The router builds four Panic(uint256)
    // reverts; Seaport uses a PUSH8 containing a length byte followed by its name.
    // Router: 0x66a9893cc07d91d95644aedd05d03f95e1dba8af
    // Seaport: 0x0000000000000068f116a894984e2db1123eb395
    let router = extract_fixture("universal_router", 4, false).await;
    let router = std::str::from_utf8(&router).unwrap();
    assert!(!router.lines().any(|line| line == "NH{q"));
    assert!(router.lines().any(|line| line == "ETH_TRANSFER_FAILED"));
    assert!(router.lines().any(|line| line == "TRANSFER_FAILED"));
    // Unclassified binary constants must remain; we favor recall over noise removal.
    assert!(router.lines().any(|line| line == "UR}$Ox"));
    let full = extract_fixture("universal_router", 4, true).await;
    assert_eq!(std::str::from_utf8(&full).unwrap().matches("NH{q").count(), 4);

    for full_scan in [false, true] {
        let seaport = extract_fixture("seaport_1_6", 4, full_scan).await;
        assert!(std::str::from_utf8(&seaport).unwrap().contains("Seaport"));
    }
}
