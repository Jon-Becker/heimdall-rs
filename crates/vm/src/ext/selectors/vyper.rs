use hashbrown::{HashMap, HashSet};
use tracing::{info, trace};

use alloy::primitives::U256;

use crate::core::{
    opcodes::{
        WrappedInput, AND, CALLDATACOPY, CALLDATALOAD, DIV, EQ, GT, ISZERO, JUMPDEST, JUMPI, LT,
        MOD, SHR, XOR,
    },
    vm::VM,
};

/// Maximum number of VM steps to execute during dispatcher tracing
const MAX_STEPS: usize = 30000;

/// Maximum recursion depth for forked execution paths (bucket/binary dispatch)
const MAX_DEPTH: usize = 64;

/// Find function selectors in Vyper-compiled contracts using symbolic execution.
///
/// Vyper contracts use different dispatch patterns than Solidity:
/// 1. `CALLDATALOAD(0)` + `SHR(224)` to extract the function selector
/// 2. Sequential EQ comparisons (sparse dispatch)
/// 3. Hash bucket dispatch using MOD/AND operations
/// 4. Binary search trees for selector matching
///
/// This function traces the dispatcher by executing the VM with zeroed calldata,
/// causing all selector comparisons to fail and the execution to fall through
/// every comparison, allowing us to observe all selectors.
pub fn find_vyper_selectors(evm: &VM, _assembly: &str) -> HashMap<String, u128> {
    let mut selectors = HashMap::new();
    let mut vm = evm.clone();

    // Set calldata to zeros so no selector comparison matches.
    // This causes the dispatcher to fall through all comparisons,
    // allowing us to observe every selector.
    vm.calldata = vec![0x00; 36];

    trace_dispatcher(&mut vm, &mut selectors, &mut HashSet::new(), MAX_STEPS, 0);

    info!("vyper: discovered {} function selectors", selectors.len());
    selectors
}

/// Recursively trace the dispatcher, extracting selectors from EQ comparisons
/// and forking execution at bucket dispatch (MOD/AND) and binary search (GT/LT) points.
fn trace_dispatcher(
    vm: &mut VM,
    selectors: &mut HashMap<String, u128>,
    visited: &mut HashSet<u128>,
    mut remaining_steps: usize,
    depth: usize,
) {
    if depth > MAX_DEPTH {
        return;
    }

    while vm.bytecode.len() >= vm.instruction as usize && remaining_steps > 0 {
        remaining_steps -= 1;

        let pc = vm.instruction;

        let state = match vm.step() {
            Ok(state) => state,
            Err(_) => break,
        };

        let instruction = &state.last_instruction;

        // Check for JUMPI - this is where selectors and dispatch branching happen
        if instruction.opcode == JUMPI {
            let jump_dest: u128 = instruction.inputs[0].try_into().unwrap_or(0);

            // Get the condition operation (what produced the JUMPI condition)
            let condition_op = match instruction.input_operations.get(1) {
                Some(op) => op,
                None => continue,
            };

            // Try to extract a selector from the condition
            if let Some(selector_val) = extract_selector_from_condition(condition_op) {
                // Validate the jump target is a JUMPDEST
                if jump_dest > 0 && is_valid_jumpdest(vm, jump_dest) {
                    let selector_hex = format!("0x{:08x}", selector_val);
                    if !selectors.contains_key(&selector_hex) {
                        trace!(
                            "vyper: selector {} -> entry point {} (at PC {})",
                            selector_hex,
                            jump_dest,
                            pc
                        );
                        selectors.insert(selector_hex, jump_dest);
                    }
                }

                // Track visited PCs to avoid infinite loops
                if visited.contains(&pc) {
                    break;
                }
                visited.insert(pc);

                // Continue falling through (calldata is zero, so EQ is always false)
                continue;
            }

            // Check if this is a calldata-derived branch (GT/LT for binary search)
            if involves_calldata_wrapped(condition_op) {
                if visited.contains(&pc) {
                    break;
                }
                visited.insert(pc);

                // Fork: explore the jump-taken path
                if jump_dest > 0 && is_valid_jumpdest(vm, jump_dest) {
                    let mut fork_vm = vm.clone();
                    // Set instruction to jump_dest (the step after JUMPDEST)
                    fork_vm.instruction = jump_dest + 1;
                    trace_dispatcher(
                        &mut fork_vm,
                        selectors,
                        &mut visited.clone(),
                        remaining_steps / 2,
                        depth + 1,
                    );
                }

                // Current VM continues the fall-through path
                continue;
            }

            // Non-calldata JUMPI - track visited but don't fork
            if visited.contains(&pc) {
                break;
            }
            visited.insert(pc);
        }

        // Check for MOD/AND bucket dispatch on calldata-derived values
        if instruction.opcode == MOD || instruction.opcode == AND {
            let has_calldata = instruction
                .input_operations
                .iter()
                .any(|op| involves_calldata_wrapped(op));

            if has_calldata {
                if let Some(n_buckets) = detect_bucket_count(instruction.opcode, &instruction.inputs)
                {
                    trace!(
                        "vyper: detected bucket dispatch with {} buckets at PC {}",
                        n_buckets,
                        pc
                    );

                    // Fork execution for each bucket value
                    let steps_per_bucket =
                        remaining_steps / (n_buckets as usize).max(1);
                    for bucket in 0..n_buckets.min(64) {
                        let mut fork_vm = vm.clone();
                        // Replace the MOD/AND result (top of stack) with the bucket number
                        if let Some(top) = fork_vm.stack.stack.front_mut() {
                            top.value = U256::from(bucket);
                        }
                        trace_dispatcher(
                            &mut fork_vm,
                            selectors,
                            &mut visited.clone(),
                            steps_per_bucket,
                            depth + 1,
                        );
                    }
                    return; // Stop this execution path, we've forked
                }
            }
        }

        if vm.exitcode != 255 || !vm.returndata.is_empty() {
            break;
        }
    }
}

/// Extract a function selector value from a JUMPI condition operation.
///
/// Handles several patterns:
/// 1. `EQ(constant, calldata_derived)` or `EQ(calldata_derived, constant)` - direct comparison
/// 2. `ISZERO(XOR(constant, calldata_derived))` - XOR-based comparison (equivalent to EQ)
/// 3. `ISZERO(SUB(constant, calldata_derived))` - subtraction-based comparison
fn extract_selector_from_condition(condition: &crate::core::opcodes::WrappedOpcode) -> Option<u64> {
    // Pattern 1: Direct EQ comparison
    if condition.opcode == EQ {
        return extract_selector_from_eq_inputs(&condition.inputs);
    }

    // Pattern 2: ISZERO(XOR(a, b)) which is equivalent to EQ(a, b)
    if condition.opcode == ISZERO {
        if let Some(WrappedInput::Opcode(inner)) = condition.inputs.first() {
            if inner.opcode == XOR {
                return extract_selector_from_eq_inputs(&inner.inputs);
            }
        }
    }

    // Pattern 3: Check if the condition solidifies to something with msg.data and ==
    // This catches cases where the WrappedOpcode tree doesn't exactly match the above patterns
    let solidified = condition.solidify();
    if solidified.contains("msg.data") {
        if solidified.contains("==") || solidified.contains("^ ") {
            return extract_selector_from_solidified(&solidified);
        }
    }

    None
}

/// Extract a selector value from EQ/XOR inputs where one input is a constant
/// and the other is derived from calldata.
fn extract_selector_from_eq_inputs(inputs: &[WrappedInput]) -> Option<u64> {
    if inputs.len() != 2 {
        return None;
    }

    // Try each input as the potential constant selector
    for i in 0..2 {
        let other_idx = 1 - i;

        // Check if the other input involves calldata
        if !involves_calldata_input(&inputs[other_idx]) {
            continue;
        }

        // Extract the constant value from this input
        if let Some(val) = extract_constant_value(&inputs[i]) {
            if is_plausible_selector(val) {
                return Some(val);
            }
        }
    }

    None
}

/// Extract a constant U256 value from a WrappedInput, traversing through PUSH operations.
fn extract_constant_value(input: &WrappedInput) -> Option<u64> {
    match input {
        WrappedInput::Raw(val) => (*val).try_into().ok(),
        WrappedInput::Opcode(op) => {
            // PUSH opcodes (0x60-0x7f) contain a constant value
            if (0x60..=0x7f).contains(&op.opcode) {
                if let Some(WrappedInput::Raw(val)) = op.inputs.first() {
                    return (*val).try_into().ok();
                }
            }
            None
        }
    }
}

/// Check if a value looks like a plausible 4-byte function selector.
fn is_plausible_selector(val: u64) -> bool {
    // Must be non-zero and fit in 4 bytes
    val > 0 && val <= 0xFFFFFFFF
}

/// Check if a WrappedInput involves calldata (CALLDATALOAD or CALLDATACOPY).
fn involves_calldata_input(input: &WrappedInput) -> bool {
    match input {
        WrappedInput::Raw(_) => false,
        WrappedInput::Opcode(op) => involves_calldata_wrapped(op),
    }
}

/// Check if a WrappedOpcode or any of its inputs involves calldata.
fn involves_calldata_wrapped(op: &crate::core::opcodes::WrappedOpcode) -> bool {
    if op.opcode == CALLDATALOAD || op.opcode == CALLDATACOPY {
        return true;
    }

    // Recursively check inputs (with depth limit to prevent stack overflow)
    involves_calldata_recursive(&op.inputs, 0)
}

/// Recursively check if any input in the tree involves calldata operations.
fn involves_calldata_recursive(inputs: &[WrappedInput], depth: usize) -> bool {
    if depth > 32 {
        return false;
    }

    for input in inputs {
        match input {
            WrappedInput::Raw(_) => {}
            WrappedInput::Opcode(op) => {
                if op.opcode == CALLDATALOAD || op.opcode == CALLDATACOPY {
                    return true;
                }
                if involves_calldata_recursive(&op.inputs, depth + 1) {
                    return true;
                }
            }
        }
    }

    false
}

/// Parse a selector value from a solidified condition string.
///
/// Looks for hex values that could be 4-byte selectors in strings like:
/// - `"(msg.data[0x00] >> 0xe0) == 0x12345678"`
/// - `"0x12345678 == (msg.data[0x00] >> 0xe0)"`
fn extract_selector_from_solidified(condition: &str) -> Option<u64> {
    // Find hex values in the condition string
    let mut i = 0;
    let bytes = condition.as_bytes();

    while i + 2 < bytes.len() {
        if bytes[i] == b'0' && bytes[i + 1] == b'x' {
            let hex_start = i + 2;
            let mut hex_end = hex_start;
            while hex_end < bytes.len() && (bytes[hex_end] as char).is_ascii_hexdigit() {
                hex_end += 1;
            }

            let hex_len = hex_end - hex_start;
            // Selectors are 1-8 hex chars (1 to 4 bytes)
            if hex_len >= 1 && hex_len <= 8 {
                let hex_str = &condition[hex_start..hex_end];
                if let Ok(val) = u64::from_str_radix(hex_str, 16) {
                    if is_plausible_selector(val)
                        && !is_common_non_selector_constant(val)
                    {
                        return Some(val);
                    }
                }
            }

            i = hex_end;
        } else {
            i += 1;
        }
    }

    None
}

/// Check if a value is a common EVM constant that is NOT a function selector.
fn is_common_non_selector_constant(val: u64) -> bool {
    matches!(
        val,
        0xe0  // 224 - used in SHR(224) for selector extraction
        | 0x100  // 256
        | 0x1f   // 31 - used in various bit operations
        | 0x20   // 32 - word size
        | 0x04   // 4 - selector size
        | 0x01   // 1
        | 0x02   // 2
        | 0xff   // 255
        | 0xffffffff // max uint32, unlikely to be a real selector
    )
}

/// Detect the number of buckets in a hash bucket dispatch.
///
/// For MOD: `sig MOD n_buckets` -> returns n_buckets
/// For AND: `sig AND (n_buckets-1)` -> returns n_buckets (must be power of 2)
fn detect_bucket_count(opcode: u8, inputs: &[U256]) -> Option<u64> {
    for input in inputs {
        let val: u64 = (*input).try_into().unwrap_or(0);

        if opcode == MOD {
            // MOD n_buckets: reasonable range is 2-256
            if val > 1 && val <= 256 {
                return Some(val);
            }
        } else if opcode == AND {
            // AND (n_buckets - 1): mask must be all 1s in low bits (power of 2 minus 1)
            // e.g., AND 0x0f = 16 buckets, AND 0x07 = 8 buckets
            if val > 0 && (val + 1).is_power_of_two() && (val + 1) <= 256 {
                return Some(val + 1);
            }
        }
    }

    None
}

/// Check if a given offset in the bytecode is a valid JUMPDEST instruction.
fn is_valid_jumpdest(vm: &VM, offset: u128) -> bool {
    let offset = offset as usize;
    if offset < vm.bytecode.len() {
        vm.bytecode[offset] == JUMPDEST
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::Address;

    fn create_test_vm(bytecode: Vec<u8>) -> VM {
        VM::new(
            &bytecode,
            &[],
            Address::default(),
            Address::default(),
            Address::default(),
            0,
            u128::MAX,
        )
    }

    #[test]
    fn test_is_plausible_selector() {
        assert!(is_plausible_selector(0x12345678));
        assert!(is_plausible_selector(0x00000001));
        assert!(is_plausible_selector(0xFFFFFFFF));
        assert!(!is_plausible_selector(0));
        assert!(!is_plausible_selector(0x100000000));
    }

    #[test]
    fn test_is_common_non_selector_constant() {
        assert!(is_common_non_selector_constant(0xe0));
        assert!(is_common_non_selector_constant(0x20));
        assert!(!is_common_non_selector_constant(0x12345678));
    }

    #[test]
    fn test_detect_bucket_count_mod() {
        // MOD with 8 buckets
        let inputs = vec![U256::from(0u64), U256::from(8u64)];
        assert_eq!(detect_bucket_count(MOD, &inputs), Some(8));

        // MOD with value too large
        let inputs = vec![U256::from(0u64), U256::from(1000u64)];
        assert_eq!(detect_bucket_count(MOD, &inputs), None);
    }

    #[test]
    fn test_detect_bucket_count_and() {
        // AND with 0x0f = 16 buckets
        let inputs = vec![U256::from(0u64), U256::from(0x0fu64)];
        assert_eq!(detect_bucket_count(AND, &inputs), Some(16));

        // AND with 0x07 = 8 buckets
        let inputs = vec![U256::from(0u64), U256::from(0x07u64)];
        assert_eq!(detect_bucket_count(AND, &inputs), Some(8));

        // AND with non-power-of-2 mask
        let inputs = vec![U256::from(0u64), U256::from(0x06u64)];
        assert_eq!(detect_bucket_count(AND, &inputs), None);
    }

    #[test]
    fn test_extract_selector_from_solidified() {
        assert_eq!(
            extract_selector_from_solidified("0x12345678 == msg.data[0x00] >> 0xe0"),
            Some(0x12345678)
        );
        assert_eq!(
            extract_selector_from_solidified("msg.data[0x00] >> 0xe0 == 0xabcdef01"),
            Some(0xabcdef01)
        );
        // Should not extract common constants
        assert_eq!(extract_selector_from_solidified("msg.data[0x00] >> 0xe0"), None);
    }

    #[test]
    fn test_is_valid_jumpdest() {
        let bytecode = vec![0x00, 0x5b, 0x00, 0x5b]; // STOP, JUMPDEST, STOP, JUMPDEST
        let vm = create_test_vm(bytecode);
        assert!(!is_valid_jumpdest(&vm, 0)); // STOP
        assert!(is_valid_jumpdest(&vm, 1)); // JUMPDEST
        assert!(!is_valid_jumpdest(&vm, 2)); // STOP
        assert!(is_valid_jumpdest(&vm, 3)); // JUMPDEST
        assert!(!is_valid_jumpdest(&vm, 100)); // out of bounds
    }
}
