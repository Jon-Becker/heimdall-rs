//! Offline regression coverage for Yul conditional reverts.

use heimdall_decompiler::{decompile, DecompilerArgsBuilder};

// solc 0.8.19, optimizer disabled; the source is checked in beside the runtime bytecode.
const REQUIRE_RUNTIME: &str = include_str!("testdata/decompile/yul-require/R.hex");

async fn decompile_require(include_yul: bool) -> String {
    let args = DecompilerArgsBuilder::new()
        .target(REQUIRE_RUNTIME.trim().to_string())
        .skip_resolving(true)
        .include_yul(include_yul)
        .include_solidity(!include_yul)
        .build()
        .expect("failed to build decompiler arguments");

    decompile(args).await.expect("failed to decompile require fixture").source.expect("no source")
}

/// Extract a complete block and its continuation, so assertions cannot accidentally match a
/// statement belonging to the sibling branch or an enclosing selector case.
fn block_after<'a>(source: &'a str, marker: &str) -> (&'a str, &'a str) {
    let (_, suffix) = source.split_once(marker).unwrap_or_else(|| panic!("missing {marker}"));
    let start = suffix.find('{').expect("missing opening brace");
    let mut depth = 0;
    for (index, character) in suffix.char_indices().skip_while(|(index, _)| *index < start) {
        match character {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return (&suffix[start + 1..index], &suffix[index + 1..]);
                }
            }
            _ => {}
        }
    }
    panic!("unclosed block after {marker}")
}

fn statements(block: &str) -> Vec<&str> {
    block.lines().map(str::trim).filter(|line| !line.is_empty()).collect()
}

#[tokio::test]
async fn yul_require_keeps_revert_payload_and_return_in_their_own_branches() {
    let source = decompile_require(true).await;
    let (function, _) = block_after(&source, "case 0xb3de648b");

    // The JUMPI is taken when x <= 8. Yul's switch normalizes the condition with iszero,
    // so case 0 is success and default is the original fallthrough error path.
    let (_, conditional) = function
        .split_once("switch iszero(iszero(gt(calldataload(0x04), 0x08)))")
        .expect("missing require condition");
    let (success, following) = block_after(conditional, "case 0");
    let (failure, _) = block_after(following, "default");

    assert_eq!(
        statements(success),
        ["mstore(0x80, 0xffff)", "return(mload(0x40), sub(add(mload(0x40), 0x20), mload(0x40)))",]
    );
    assert_eq!(
        statements(failure),
        [
            "mstore(0x80, 0x08c379a000000000000000000000000000000000000000000000000000000000)",
            "mstore(0x84, sub(add(add(0x04, mload(0x40)), 0x20), add(0x04, mload(0x40))))",
            "mstore(0xa4, 0x02)",
            "mstore(0xc4, 0x4530000000000000000000000000000000000000000000000000000000000000)",
            "revert(mload(0x40), sub(add(add(add(add(0x04, mload(0x40)), 0x20), 0x20), 0x20), mload(0x40)))",
        ]
    );

    // ABI validation remains present; the compiler's tautological argument check must not
    // become an unconditional revert guard again.
    assert!(function.contains("revert(0, 0)"));
    assert!(!function.contains("eq(calldataload(0x04), calldataload(0x04))"));
}

#[tokio::test]
async fn solidity_require_preserves_the_passing_condition_and_reason() {
    let source = decompile_require(false).await;
    let (function, _) = block_after(&source, "function Unresolved_b3de648b(");
    assert!(function.contains("require(arg0 <= 0x08, \"E0\");"), "{function}");
    assert!(function.contains("return 0xffff;"), "{function}");
    assert!(!function.contains("require(arg0 > 0x08"), "{function}");
}

#[tokio::test]
async fn yul_branch_reconstruction_preserves_argument_mask_inference() {
    // The three two-array decoders bound each tail by offset + 32 + length * 32.
    // Their address/bool/bytes4 masks apply to tail elements, not top-level arguments.
    // The complex 3334e663 decoder currently reaches an array and a scalar address;
    // its remaining nested decoding is outside this regression's coverage.
    // d9c45357 has one byte-length tail (length * 1); its nested abi.decode reads
    // must not become additional top-level arguments.
    let expected_inputs = [
        ("33320de3", vec!["uint256[]", "uint256[]"]),
        ("3334e663", vec!["uint256[]", "address"]),
        ("3d4a345c", vec!["uint256[]", "uint256[]"]),
        ("bf298c34", vec!["uint256[]", "uint256[]"]),
        ("d9c45357", vec!["bytes"]),
    ];

    // Rendering taken/fallthrough branches must preserve the argument observations
    // made by ABI-only analysis, including masks on actual scalar head words.
    for include_yul in [false, true] {
        let args = DecompilerArgsBuilder::new()
            .target(include_str!("testdata/decompile/clamping.hex").trim().to_string())
            .skip_resolving(true)
            .include_yul(include_yul)
            .build()
            .expect("failed to build decompiler arguments");
        let result = decompile(args).await.expect("failed to decompile clamping fixture");

        for (selector, expected) in &expected_inputs {
            let function = &result.abi.functions[&format!("Unresolved_{selector}")][0];
            let inputs =
                function.inputs.iter().map(|parameter| parameter.ty.as_str()).collect::<Vec<_>>();
            assert_eq!(
                &inputs, expected,
                "argument types changed for {selector} (include_yul={include_yul})"
            );
        }
    }
}

#[tokio::test]
async fn yul_preserves_dynamic_byte_condition_before_a_complete_revert_branch() {
    // if byte(0, callvalue()) == callvalue(): revert(0, 0), otherwise return 0xffff.
    // The condition is true for value 0 and false for value 1. Solidity expression
    // lowering drops BYTE, so treating its apparent identity as a constant is unsound.
    let args = DecompilerArgsBuilder::new()
        .target("348060001a1460145761ffff60005260206000f35b60006000fd".to_string())
        .skip_resolving(true)
        .include_yul(true)
        .build()
        .expect("failed to build decompiler arguments");
    let source =
        decompile(args).await.expect("failed to decompile BYTE guard").source.expect("no source");
    let (fallback, _) = block_after(&source, "default");
    assert!(
        fallback.trim_start().starts_with("switch iszero(eq(byte(0, callvalue()), callvalue()))"),
        "{fallback}"
    );
    let (failure, following) = block_after(fallback, "case 0");
    let (success, _) = block_after(following, "default");
    assert_eq!(statements(failure), ["revert(0, 0)"]);
    assert_eq!(statements(success), ["mstore(0, 0xffff)", "return(0, 0x20)"]);
}
