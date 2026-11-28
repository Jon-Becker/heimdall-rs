//! Offline regressions for dynamic calldata recovery (issue #771).

use heimdall_decompiler::{decompile, DecompileResult, DecompilerArgsBuilder, HardFork};

async fn decompile_fixture(bytecode: &str, include_solidity: bool) -> DecompileResult {
    decompile(
        DecompilerArgsBuilder::new()
            .target(bytecode.trim().to_owned())
            .hardfork(HardFork::Cancun)
            .skip_resolving(true)
            .include_solidity(include_solidity)
            .timeout(5000)
            .build()
            .unwrap(),
    )
    .await
    .expect("failed to decompile dynamic calldata fixture")
}

fn assert_inputs(result: &DecompileResult, selector: &str, expected: &[&str]) {
    let name = format!("Unresolved_{selector}");
    let functions = result.abi.functions.get(&name).expect("missing fixture function");
    assert_eq!(functions.len(), 1);
    let inputs: Vec<_> = functions[0].inputs.iter().map(|input| input.ty.as_str()).collect();
    assert_eq!(inputs, expected, "incorrect inputs for {name}");

    let details = result
        .abi_with_details
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["name"] == name)
        .expect("missing extended ABI function");
    assert_eq!(details["selector"], format!("0x{selector}"));
}

#[tokio::test]
async fn preserves_forwarded_bytes_argument() {
    for include_solidity in [false, true] {
        let result = decompile_fixture(
            include_str!("testdata/dynamic-calldata/ArbitraryCallVulnerable.hex"),
            include_solidity,
        )
        .await;
        assert_inputs(&result, "1cff79cd", &["address", "bytes"]);

        if include_solidity {
            let source = result.source.expect("missing Solidity source");
            assert!(
                source.lines().any(|line| {
                    line.contains(".call") && line.contains("arg0") && line.contains("arg1")
                }),
                "forwarded bytes must remain an argument of the external call:\n{source}"
            );
        }
    }
}

#[tokio::test]
async fn preserves_all_dynamic_heads_and_array_shapes() {
    for include_solidity in [false, true] {
        let result = decompile_fixture(
            include_str!("testdata/dynamic-calldata/AbiInferenceMatrix.hex"),
            include_solidity,
        )
        .await;

        // The unused arrays share a decoder, so their element types and nesting are erased.
        // Likewise, string and bytes share a decoder. Assert the shapes the runtime proves.
        assert_inputs(&result, "8484e693", &["bytes", "uint256[]", "uint256[]", "bytes"]);
    }
}
