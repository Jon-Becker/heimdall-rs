//! Integration tests for string extraction from deployed Ethereum contract bytecode.

use std::{collections::BTreeMap, num::NonZeroUsize};

use serde::Deserialize;

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
async fn test_strings_contract_snapshots() {
    #[derive(Deserialize)]
    struct Expected {
        push: Vec<String>,
        full_scan: Vec<String>,
    }

    let contracts: BTreeMap<String, Expected> =
        serde_json::from_str(include_str!("testdata/strings/expected.json")).unwrap();
    for (name, expected) in contracts {
        for (full_scan, lines) in [(false, expected.push), (true, expected.full_scan)] {
            for min_length in [4, 16] {
                let expected: String = lines
                    .iter()
                    .filter(|line| line.len() >= min_length)
                    .map(|line| format!("{line}\n"))
                    .collect();
                assert_eq!(
                    extract_fixture(&name, min_length, full_scan).await,
                    expected.as_bytes(),
                    "{name}, full_scan={full_scan}, min_length={min_length}"
                );
            }
        }
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
