//! End-to-end tests for the strings command.

use std::{fs, process::Command};

fn strings(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_heimdall"))
        .arg("strings")
        .args(args)
        .output()
        .expect("failed to run heimdall strings")
}

#[test]
fn extracts_hex_with_and_without_prefix() {
    for target in ["0x6448656c6c6f0064776f726c64", "6448656c6c6f0064776f726c64"] {
        let output = strings(&[target]);
        assert!(output.status.success(), "{:?}", output);
        assert_eq!(output.stdout, b"Hello\nworld\n");
        assert!(output.stderr.is_empty());
    }
}

#[test]
fn extracts_hex_file() {
    let path = std::env::temp_dir().join(format!("heimdall-strings-{}.hex", std::process::id()));
    fs::write(&path, "0x6448656c6c6f00\n64776f726c64\n").unwrap();
    let output = strings(&[path.to_str().unwrap()]);
    fs::remove_file(path).unwrap();
    assert!(output.status.success(), "{:?}", output);
    assert_eq!(output.stdout, b"Hello\nworld\n");
}

#[test]
fn supports_custom_minimum_length() {
    let output = strings(&["0x61616263616263646278797a", "-n", "3"]);
    assert!(output.status.success(), "{:?}", output);
    assert_eq!(output.stdout, b"abcd\nxyz\n");
}

#[test]
fn no_matches_is_successful_with_empty_output() {
    for target in ["0x", "0x00017fff", "0x616263"] {
        let output = strings(&[target]);
        assert!(output.status.success(), "{:?}", output);
        assert!(output.stdout.is_empty());
    }
}

#[test]
fn rejects_invalid_input_and_minimum_length() {
    for args in [vec!["0xzz"], vec!["0x123"], vec!["€"], vec!["0x00", "-n", "0"]] {
        let output = strings(&args);
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert!(!output.stderr.is_empty());
        assert!(!String::from_utf8_lossy(&output.stderr).contains("panicked"));
    }
}

#[test]
fn documents_strings_in_cli_help() {
    let output = strings(&["--help"]);
    assert!(output.status.success());
    let help = String::from_utf8(output.stdout).unwrap();
    assert!(help.contains("--min-length"));
    assert!(help.contains("--rpc-url"));
    assert!(help.contains("--full-scan"));
}

#[test]
fn reports_rpc_connection_errors() {
    let output =
        strings(&["0x48656c6c6f48656c6c6f48656c6c6f48656c6c6f", "--rpc-url", "invalid://endpoint"]);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(error.contains("failed to load bytecode"), "{error}");
    assert!(!error.contains("panicked"), "{error}");
}

#[test]
fn extracts_real_contract_bytecode() {
    for (name, expected) in [
        (
            "uniswap_v2_usdc_weth",
            include_str!("../../core/tests/testdata/strings/uniswap_v2_usdc_weth.json"),
        ),
        ("dai", include_str!("../../core/tests/testdata/strings/dai.json")),
    ] {
        let target =
            format!("{}/../core/tests/testdata/strings/{name}.hex", env!("CARGO_MANIFEST_DIR"));
        let output = strings(&[&target]);
        assert!(output.status.success(), "{name}: {output:?}");
        let expected: Vec<String> = serde_json::from_str(expected).unwrap();
        assert_eq!(output.stdout, format!("{}\n", expected.join("\n")).as_bytes(), "{name}");
        assert!(output.stderr.is_empty(), "{name}: {output:?}");
    }
}

#[test]
fn full_scan_includes_bytes_outside_push_payloads() {
    let target = "0x4142434400";
    let output = strings(&[target]);
    assert!(output.status.success());
    assert!(output.stdout.is_empty());
    let output = strings(&[target, "--full-scan"]);
    assert!(output.status.success());
    assert_eq!(output.stdout, b"ABCD\n");
}

#[test]
fn full_scan_matches_real_contract_snapshots() {
    for (name, expected) in [
        (
            "uniswap_v2_usdc_weth",
            include_str!("../../core/tests/testdata/strings/uniswap_v2_usdc_weth.full_scan.json"),
        ),
        ("dai", include_str!("../../core/tests/testdata/strings/dai.full_scan.json")),
    ] {
        let target =
            format!("{}/../core/tests/testdata/strings/{name}.hex", env!("CARGO_MANIFEST_DIR"));
        let output = strings(&[&target, "--full-scan"]);
        assert!(output.status.success(), "{name}: {output:?}");
        let expected: Vec<String> = serde_json::from_str(expected).unwrap();
        assert_eq!(output.stdout, format!("{}\n", expected.join("\n")).as_bytes(), "{name}");
        assert!(output.stderr.is_empty());
    }
}
