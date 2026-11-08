//! End-to-end output routing tests.

use std::{fs, process::Command};

#[test]
fn raw_inputs_with_address_and_transaction_lengths_use_local_output() {
    let home = std::env::temp_dir().join(format!("heimdall-output-targets-{}", std::process::id()));
    fs::create_dir_all(&home).unwrap();
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_heimdall"))
            .args(args)
            .current_dir(&home)
            .env("HOME", &home)
            .env("MESC_MODE", "DISABLED")
            .env("HTTP_PROXY", "http://127.0.0.1:0")
            .env("HTTPS_PROXY", "http://127.0.0.1:0")
            .env("ALL_PROXY", "http://127.0.0.1:0")
            .env("NO_PROXY", "")
            .env("http_proxy", "http://127.0.0.1:0")
            .env("https_proxy", "http://127.0.0.1:0")
            .env("all_proxy", "http://127.0.0.1:0")
            .env("no_proxy", "")
            .output()
            .unwrap()
    };

    let assembly = run(&["disassemble", "0x6000000000000000000000000000000000000000"]);
    assert!(assembly.status.success(), "{}", String::from_utf8_lossy(&assembly.stderr));
    let output = home.join("output/local");
    let disassembly = fs::read_to_string(output.join("disassembled.asm")).unwrap();
    assert!(disassembly.contains("PUSH1"));

    let decoded = run(&[
        "decode",
        "0x0000000000000000000000000000000000000000000000000000000000000000",
        "--raw",
        "--skip-resolving",
        "--truncate-calldata",
        "--output",
        "output",
    ]);
    assert!(decoded.status.success(), "{}", String::from_utf8_lossy(&decoded.stderr));
    let json: serde_json::Value =
        serde_json::from_slice(&fs::read(output.join("decoded.json")).unwrap()).unwrap();
    assert_eq!(json["name"], "Unresolved_00000000");
    fs::remove_dir_all(home).unwrap();
}
