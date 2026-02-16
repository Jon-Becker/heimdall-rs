//! Integration tests for Vyper selector detection.
//!
//! These tests verify that the selector detection infrastructure correctly
//! handles Vyper-compiled contracts using CALLDATA flow tracing.

#[cfg(test)]
mod vyper_selector_tests {
    use alloy::primitives::Address;
    use heimdall_common::ether::compiler::{detect_compiler, Compiler};
    use heimdall_vm::core::vm::VM;
    use heimdall_vm::ext::selectors::{find_function_selectors, resolve_entry_point};

    /// Construct a Vyper-style dispatcher bytecode with known selectors.
    ///
    /// This bytecode mimics a Vyper contract with 3 functions:
    /// - transfer(address,uint256) = 0xa9059cbb, entry @ 0x3b
    /// - balanceOf(address) = 0x70a08231, entry @ 0x3d
    /// - approve(address,uint256) = 0x095ea7b3, entry @ 0x3f
    ///
    /// The bytecode starts with the Vyper prefix (0x60 0x04 0x36 0x10 0x15)
    /// and includes Vyper CBOR metadata so compiler detection identifies it as Vyper.
    fn vyper_test_bytecode() -> Vec<u8> {
        vec![
            // Vyper prefix: PUSH1 4, CALLDATASIZE, LT, ISZERO, PUSH2 0x000e, JUMPI
            0x60, 0x04, 0x36, 0x10, 0x15, 0x61, 0x00, 0x0e, 0x57,
            // revert if calldata < 4 bytes
            0x60, 0x00, 0x60, 0x00, 0xfd,
            // JUMPDEST: dispatcher start
            0x5b,
            // extract 4-byte selector: PUSH1 0, CALLDATALOAD, PUSH1 0xe0, SHR
            0x60, 0x00, 0x35, 0x60, 0xe0, 0x1c,
            // compare selector against transfer (0xa9059cbb)
            0x80, 0x63, 0xa9, 0x05, 0x9c, 0xbb, 0x14, 0x61, 0x00, 0x3b, 0x57,
            // compare selector against balanceOf (0x70a08231)
            0x80, 0x63, 0x70, 0xa0, 0x82, 0x31, 0x14, 0x61, 0x00, 0x3d, 0x57,
            // compare selector against approve (0x095ea7b3)
            0x80, 0x63, 0x09, 0x5e, 0xa7, 0xb3, 0x14, 0x61, 0x00, 0x3f, 0x57,
            // no match: revert
            0x60, 0x00, 0x60, 0x00, 0xfd,
            // function entry points
            0x5b, 0x00, // transfer @ 0x3b
            0x5b, 0x00, // balanceOf @ 0x3d
            0x5b, 0x00, // approve @ 0x3f
            // Vyper CBOR metadata: "vyper" (0x76797065_72) + 0x83 + version 0.3.10
            0x76, 0x79, 0x70, 0x65, 0x72, 0x83, 0x00, 0x03, 0x0a,
        ]
    }

    fn create_vm(bytecode: &[u8]) -> VM {
        VM::new(
            bytecode,
            &[],
            Address::default(),
            Address::default(),
            Address::default(),
            0,
            u128::MAX,
        )
    }

    #[test]
    fn test_compiler_detection_identifies_vyper() {
        let bytecode = vyper_test_bytecode();
        let (compiler, version) = detect_compiler(&bytecode);
        assert_eq!(compiler, Compiler::Vyper, "bytecode should be detected as Vyper");
        assert!(!version.is_empty(), "version should be detected");
    }

    #[test]
    fn test_vyper_selector_detection() {
        let bytecode = vyper_test_bytecode();
        let evm = create_vm(&bytecode);

        // find_function_selectors detects compiler and routes to vyper strategy
        let selectors = find_function_selectors(&evm, "");

        // verify all 3 selectors are found
        assert!(
            selectors.contains_key("0xa9059cbb"),
            "should detect transfer(address,uint256) selector"
        );
        assert!(
            selectors.contains_key("0x70a08231"),
            "should detect balanceOf(address) selector"
        );
        assert!(
            selectors.contains_key("0x095ea7b3"),
            "should detect approve(address,uint256) selector"
        );
        assert_eq!(selectors.len(), 3, "should find exactly 3 selectors");
    }

    #[test]
    fn test_vyper_entry_point_resolution() {
        let bytecode = vyper_test_bytecode();

        // test each selector resolves to the correct entry point
        let test_cases = vec![
            ("0xa9059cbb", 0x3b_u128), // transfer
            ("0x70a08231", 0x3d_u128), // balanceOf
            ("0x095ea7b3", 0x3f_u128), // approve
        ];

        for (selector, expected_entry) in test_cases {
            let mut vm = create_vm(&bytecode);
            let entry = resolve_entry_point(&mut vm, selector);
            assert_eq!(
                entry, expected_entry,
                "entry point for selector {} should be 0x{:x}",
                selector, expected_entry
            );
        }
    }

    #[test]
    fn test_vyper_unknown_selector_returns_zero() {
        let bytecode = vyper_test_bytecode();
        let mut vm = create_vm(&bytecode);

        let entry = resolve_entry_point(&mut vm, "0xdeadbeef");
        assert_eq!(entry, 0, "unknown selector should return entry point 0");
    }

    #[test]
    fn test_vyper_selectors_with_non_zero_entry_points() {
        let bytecode = vyper_test_bytecode();
        let evm = create_vm(&bytecode);

        let selectors = find_function_selectors(&evm, "");

        // all entry points should be non-zero
        for (selector, entry_point) in &selectors {
            assert_ne!(*entry_point, 0, "selector {} should have non-zero entry point", selector);
        }
    }
}
