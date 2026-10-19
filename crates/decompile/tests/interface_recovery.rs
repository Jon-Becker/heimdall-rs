//! Offline regressions for dispatcher and ABI interface recovery.

use alloy::primitives::Address;
use hashbrown::HashMap;
use heimdall_common::utils::strings::decode_hex;
use heimdall_disassembler::{disassemble, DisassemblerArgsBuilder};
use heimdall_vm::{core::vm::VM, ext::selectors::find_function_selectors};
use serde::Deserialize;

async fn selectors(runtime: &str) -> HashMap<String, u128> {
    let bytecode = decode_hex(runtime.trim().trim_start_matches("0x")).unwrap();
    let vm = VM::new(&bytecode, &[], Address::ZERO, Address::ZERO, Address::ZERO, 0, u128::MAX);
    let assembly = disassemble(
        DisassemblerArgsBuilder::new().target(runtime.trim().to_string()).build().unwrap(),
    )
    .await
    .unwrap();
    find_function_selectors(&vm, &assembly)
}

#[tokio::test]
async fn discovers_dispatch_branches_without_source_string_matching() {
    let fixtures = [
        (
            "computed divisor",
            "60003560e060020a900463deadbeef1461001857600080fd5b00",
            "deadbeef",
            24,
        ),
        ("equality", "60003560e01c63deadbeef1461001457600080fd5b00", "deadbeef", 20),
        ("subtraction", "60003560e01c63deadbeef0361001157005b600080fd", "deadbeef", 16),
        ("xor", "60003560e01c63deadbeef1861001157005b600080fd", "deadbeef", 16),
        ("inverse equality", "60003560e01c63deadbeef141561001257005b600080fd", "deadbeef", 17),
        ("short push", "60003560e01c627611111461001357600080fd5b00", "00761111", 19),
        ("leading-zero push4", "60003560e01c63007611111461001457600080fd5b00", "00761111", 20),
        ("push2", "60003560e01c61beef1461001257600080fd5b00", "0000beef", 18),
        ("push1", "60003560e01c60ef1461001157600080fd5b00", "000000ef", 17),
        ("push0 equality", "60003560e01c5f1461001057600080fd5b00", "00000000", 16),
        (
            "memory offset",
            "600061016052610160513560e01c63deadbeef1461001c57600080fd5b00",
            "deadbeef",
            28,
        ),
        ("zero selector", "60003560e01c1561000f57600080fd5b00", "00000000", 15),
    ];
    for (name, bytecode, selector, entry) in fixtures {
        let found = selectors(bytecode).await;
        assert_eq!(found.len(), 1, "{name}: {found:?}");
        assert_eq!(found.get(selector), Some(&entry), "{name}");
    }
}

#[tokio::test]
async fn rejects_constants_without_a_selector_dispatch_comparison() {
    for bytecode in [
        "63deadbeef5000",
        "7f63deadbeef1461001457600080fd5b0000000000000000000000000000000000005000",
        "60043560e01c63deadbeef1461001457600080fd5b00",
        "600461016052610160513560e01c63deadbeef1461001c57600080fd5b00",
        "3363deadbeef1461001057600080fd5b00",
    ] {
        assert!(selectors(bytecode).await.is_empty(), "{bytecode}");
    }
}

#[derive(Deserialize)]
struct Case {
    chain: String,
    address: String,
    runtime: String,
    runtime_keccak256: String,
    expected_selectors: Vec<String>,
}

#[tokio::test]
async fn discovers_observed_selectors_in_captured_runtime_corpus() {
    let cases: Vec<Case> =
        serde_json::from_str(include_str!("testdata/interface-recovery/cases.json")).unwrap();
    let directory =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/testdata/interface-recovery");
    let mut missing = Vec::new();
    let mut expected_count = 0;
    for case in &cases {
        let runtime = std::fs::read_to_string(directory.join(&case.runtime)).unwrap();
        let bytes = decode_hex(runtime.trim().trim_start_matches("0x")).unwrap();
        assert_eq!(
            alloy::primitives::keccak256(bytes).to_string(),
            case.runtime_keccak256,
            "{}:{}",
            case.chain,
            case.address
        );
        let found = selectors(&runtime).await;
        for selector in &case.expected_selectors {
            expected_count += 1;
            if !found.contains_key(selector) {
                missing.push(format!("{}:{}:0x{}", case.chain, case.address, selector));
            }
        }
    }
    assert_eq!(cases.len(), 74);
    assert_eq!(expected_count, 94);
    assert!(missing.is_empty(), "Missing observed selectors: {missing:#?}");
}

#[tokio::test]
async fn decompiles_short_selector_runtime_with_an_invalid_fallback_jump() {
    let args = heimdall_decompiler::DecompilerArgsBuilder::new()
        .target(
            include_str!(
                "testdata/interface-recovery/base-0x434ec1685630d347c85327d8aab97ad69e7aa050.hex"
            )
            .trim()
            .to_string(),
        )
        .skip_resolving(true)
        .timeout(10000)
        .build()
        .unwrap();
    let result = heimdall_decompiler::decompile(args).await.unwrap();
    assert!(result.abi.functions().any(|function| function.name == "Unresolved_00735c98"));
}
