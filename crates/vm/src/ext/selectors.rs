use hashbrown::{HashMap, HashSet};
use std::{
    sync::{Arc, Mutex},
    time::Instant,
};

use eyre::Result;
use heimdall_common::{
    ether::{
        compiler::{detect_compiler, Compiler},
        signatures::{ResolveSelector, ResolvedFunction},
    },
    utils::strings::decode_hex,
};
use tokio::task;
use tracing::{debug, error, info, trace, warn};

use crate::core::vm::VM;

/// Finds and resolves function selectors from disassembled bytecode
///
/// This function analyzes disassembled EVM bytecode to extract function selectors
/// and optionally resolves them to human-readable function signatures.
///
/// # Arguments
/// * `disassembled_bytecode` - The disassembled EVM bytecode to analyze
/// * `skip_resolving` - If true, skip the process of resolving selectors to function signatures
/// * `evm` - The VM instance to use for analysis
///
/// # Returns
/// * A Result containing a tuple with:
///   - A HashMap mapping selector strings to their instruction offsets
///   - A HashMap mapping selector strings to their resolved function information
pub async fn get_resolved_selectors(
    disassembled_bytecode: &str,
    skip_resolving: &bool,
    evm: &VM,
) -> Result<(HashMap<String, u128>, HashMap<String, Vec<ResolvedFunction>>)> {
    let selectors = find_function_selectors(evm, disassembled_bytecode);

    let mut resolved_selectors = HashMap::new();
    if !skip_resolving {
        resolved_selectors =
            resolve_selectors::<ResolvedFunction>(selectors.keys().cloned().collect()).await;

        trace!(
            "resolved {} possible functions from {} detected selectors.",
            resolved_selectors.len(),
            selectors.len()
        );
    } else {
        trace!("found {} possible function selectors.", selectors.len());
    }

    Ok((selectors, resolved_selectors))
}

/// Find all function selectors in the given EVM assembly.
///
/// Detects the compiler used to compile the contract and uses the appropriate
/// strategy for selector discovery:
/// - Solidity (solc): Uses PUSH4 pattern matching in disassembled bytecode
/// - Vyper: Uses CALLDATA flow tracing to detect selectors through the O(1)
///   bucket-based dispatcher
/// - Unknown: Tries both methods and merges results
pub fn find_function_selectors(evm: &VM, assembly: &str) -> HashMap<String, u128> {
    let (compiler, version) = detect_compiler(&evm.bytecode);
    debug!("selector detection using compiler: {} {}", compiler, version);

    match compiler {
        Compiler::Solc | Compiler::Proxy => find_solidity_selectors(evm, assembly),
        Compiler::Vyper => find_vyper_selectors(evm),
        Compiler::Unknown => {
            // try solidity first (most common), then vyper, merge results
            let mut selectors = find_solidity_selectors(evm, assembly);
            if selectors.is_empty() {
                debug!("no selectors found with solidity strategy, trying vyper");
                selectors = find_vyper_selectors(evm);
            }
            selectors
        }
    }
}

/// Find function selectors using Solidity's PUSH4 pattern matching in disassembled bytecode.
///
/// Searches through assembly for PUSH4 instructions, optimistically assuming they are function
/// selectors, then resolves each selector's entry point via symbolic execution.
fn find_solidity_selectors(evm: &VM, assembly: &str) -> HashMap<String, u128> {
    let mut function_selectors = HashMap::new();
    let mut handled_selectors = HashSet::new();

    // search through assembly for PUSH4 instructions, optimistically assuming that
    // they are function selectors
    let assembly: Vec<String> = assembly.split('\n').map(|line| line.trim().to_string()).collect();
    for line in assembly.iter() {
        let instruction_args: Vec<String> = line.split(' ').map(|arg| arg.to_string()).collect();

        if instruction_args.len() >= 2 {
            let instruction = instruction_args[1].clone();

            if &instruction == "PUSH4" {
                let function_selector = instruction_args[2].clone();

                // check if this function selector has already been handled
                if handled_selectors.contains(&function_selector) {
                    continue;
                }

                trace!(
                    "optimistically assuming instruction {} {} {} is a function selector",
                    instruction_args[0],
                    instruction_args[1],
                    instruction_args[2]
                );

                // add the function selector to the handled selectors
                handled_selectors.insert(function_selector.clone());

                // get the function's entry point
                let function_entry_point =
                    match resolve_solidity_entry_point(&mut evm.clone(), &function_selector) {
                        0 => continue,
                        x => x,
                    };

                trace!(
                    "found function selector {} at entry point {}",
                    function_selector,
                    function_entry_point
                );

                function_selectors.insert(function_selector, function_entry_point);
            }
        }
    }

    info!("discovered {} function selectors via solidity strategy", function_selectors.len());
    function_selectors
}

/// Find function selectors by tracing CALLDATA flow through Vyper's O(1) bucket-based dispatcher.
///
/// Vyper compilers use a hash-based dispatcher: the 4-byte selector extracted from calldata is
/// used to compute a bucket index (typically `selector % n_buckets`), which is then used to jump
/// to a bucket containing one or more selector comparisons. This function traces the bytecode
/// execution to discover selectors by:
///
/// 1. Executing the bytecode with symbolic calldata to observe dispatcher behavior
/// 2. Detecting when the bytecode performs CALLDATALOAD at offset 0 followed by SHR (224 bits)
///    to extract the 4-byte selector
/// 3. Monitoring JUMPI instructions that perform EQ comparisons against known selector values
/// 4. Recording both the selectors and their corresponding function entry points
fn find_vyper_selectors(evm: &VM) -> HashMap<String, u128> {
    let mut function_selectors = HashMap::new();

    // execute VM with a dummy selector to trace the dispatcher structure.
    // Vyper's dispatcher compares the extracted selector against hardcoded values,
    // so we can observe the comparisons by tracing execution.
    let mut vm = evm.clone();
    // set calldata to a known 4-byte selector padded to 32 bytes so CALLDATALOAD(0) works
    vm.calldata = vec![0x00; 36];

    let mut max_steps: u32 = 10000;

    while vm.bytecode.len() >= vm.instruction as usize && max_steps > 0 {
        max_steps -= 1;
        let call = match vm.step() {
            Ok(call) => call,
            Err(_) => break,
        };

        // look for JUMPI instructions (0x57) where the condition involves an EQ comparison
        // with a selector value. In Vyper dispatchers, the pattern is:
        //   CALLDATALOAD(0) >> 0xe0  (extract selector)
        //   EQ(extracted_selector, hardcoded_selector)
        //   JUMPI(destination, eq_result)
        if call.last_instruction.opcode == 0x57 {
            let jump_condition = call.last_instruction.input_operations[1].solidify();

            // check if this is a selector comparison: the condition should contain
            // msg.data (indicating CALLDATALOAD), and an equality check
            if jump_condition.contains("msg.data") && jump_condition.contains(" == ") {
                // extract the selector value from the comparison
                if let Some(selector) = extract_selector_from_condition(&jump_condition) {
                    let entry_point: u128 =
                        call.last_instruction.inputs[0].try_into().unwrap_or(0);
                    if entry_point != 0 {
                        trace!(
                            "vyper dispatcher: found selector {} at entry point {}",
                            selector,
                            entry_point
                        );
                        function_selectors.insert(selector, entry_point);
                    }
                }
            }
        }

        if vm.exitcode != 255 || !vm.returndata.is_empty() {
            break;
        }
    }

    // if the initial pass with zeroed calldata didn't find selectors (e.g. the dispatcher
    // jumps to a specific bucket and only reveals selectors in that bucket), try additional
    // probe values to cover more buckets
    if function_selectors.is_empty() {
        trace!("vyper first pass found no selectors, probing with additional calldata values");
        // try a few different probe values to trigger different dispatcher paths
        let probes: Vec<[u8; 4]> = vec![
            [0xFF, 0xFF, 0xFF, 0xFF],
            [0x01, 0x00, 0x00, 0x00],
            [0xA9, 0x05, 0x9C, 0xBB], // transfer(address,uint256)
            [0x70, 0xA0, 0x82, 0x31], // balanceOf(address)
        ];

        for probe in probes {
            let mut vm = evm.clone();
            let mut calldata = probe.to_vec();
            calldata.resize(36, 0);
            vm.calldata = calldata;

            let mut steps: u32 = 10000;
            while vm.bytecode.len() >= vm.instruction as usize && steps > 0 {
                steps -= 1;
                let call = match vm.step() {
                    Ok(call) => call,
                    Err(_) => break,
                };

                if call.last_instruction.opcode == 0x57 {
                    let jump_condition = call.last_instruction.input_operations[1].solidify();
                    if jump_condition.contains("msg.data") && jump_condition.contains(" == ") {
                        if let Some(selector) = extract_selector_from_condition(&jump_condition) {
                            let entry_point: u128 =
                                call.last_instruction.inputs[0].try_into().unwrap_or(0);
                            if entry_point != 0 && !function_selectors.contains_key(&selector) {
                                trace!(
                                    "vyper dispatcher (probe): found selector {} at entry point {}",
                                    selector,
                                    entry_point
                                );
                                function_selectors.insert(selector, entry_point);
                            }
                        }
                    }
                }

                if vm.exitcode != 255 || !vm.returndata.is_empty() {
                    break;
                }
            }
        }
    }

    info!("discovered {} function selectors via vyper strategy", function_selectors.len());
    function_selectors
}

/// Extract a 4-byte function selector from a solidified comparison condition.
///
/// The condition string typically looks like one of:
/// - `msg.data[0x00] >> 0xe0 == 0xa9059cbb`
/// - `0xa9059cbb == msg.data[0x00] >> 0xe0`
/// - Other variations involving msg.data comparisons
///
/// Returns the selector as a hex string (e.g., "0xa9059cbb") or None if not found.
fn extract_selector_from_condition(condition: &str) -> Option<String> {
    // split on " == " to get the two sides of the comparison
    let parts: Vec<&str> = condition.split(" == ").collect();
    if parts.len() != 2 {
        return None;
    }

    // one side should contain msg.data (the extracted selector), the other should be
    // the hardcoded selector value
    let (selector_side, _data_side) = if parts[0].contains("msg.data") {
        (parts[1].trim(), parts[0].trim())
    } else if parts[1].contains("msg.data") {
        (parts[0].trim(), parts[1].trim())
    } else {
        return None;
    };

    // the selector side should be a hex value like 0xNNNNNNNN
    // it may have extra whitespace or nested expressions, so try to extract the hex value
    let selector_str = selector_side.trim();

    // handle case where selector is a simple hex value
    if selector_str.starts_with("0x") && selector_str.len() <= 10 {
        // validate it looks like a 4-byte selector (up to 8 hex chars after 0x)
        let hex_part = &selector_str[2..];
        if hex_part.chars().all(|c| c.is_ascii_hexdigit()) && !hex_part.is_empty() {
            // normalize to 8 hex chars with leading zeros
            let normalized = format!("0x{:0>8}", hex_part);
            return Some(normalized);
        }
    }

    // handle case where the selector is embedded in a more complex expression
    // look for a hex pattern in the string
    for word in selector_str.split_whitespace() {
        if word.starts_with("0x") {
            let hex_part = &word[2..];
            if hex_part.len() <= 8 && hex_part.chars().all(|c| c.is_ascii_hexdigit()) {
                let normalized = format!("0x{:0>8}", hex_part);
                return Some(normalized);
            }
        }
    }

    None
}

/// Resolve a selector's function entry point from the EVM bytecode.
///
/// Detects the compiler type and uses the appropriate resolution strategy:
/// - Solidity: Looks for JUMPI with direct selector comparison in the condition
/// - Vyper: Traces execution to find the selector match within O(1) bucket dispatcher
/// - Unknown: Tries Solidity first, then falls back to Vyper
pub fn resolve_entry_point(vm: &mut VM, selector: &str) -> u128 {
    let (compiler, _version) = detect_compiler(&vm.bytecode);

    match compiler {
        Compiler::Solc | Compiler::Proxy => resolve_solidity_entry_point(vm, selector),
        Compiler::Vyper => resolve_vyper_entry_point(vm, selector),
        Compiler::Unknown => {
            // try solidity approach first (most common)
            let entry = resolve_solidity_entry_point(&mut vm.clone(), selector);
            if entry != 0 {
                return entry;
            }
            // fallback to vyper
            resolve_vyper_entry_point(vm, selector)
        }
    }
}

/// Resolve a selector's entry point using Solidity's dispatcher pattern.
///
/// Executes the VM with the given selector as calldata and looks for JUMPI
/// instructions where the condition contains a direct EQ comparison between
/// msg.data[0] and the selector value.
fn resolve_solidity_entry_point(vm: &mut VM, selector: &str) -> u128 {
    let mut handled_jumps = HashSet::new();

    // execute the EVM call to find the entry point for the given selector
    vm.calldata = decode_hex(selector).expect("Failed to decode selector.");
    while vm.bytecode.len() >= vm.instruction as usize {
        let call = match vm.step() {
            Ok(call) => call,
            Err(_) => break, // the call failed, so we can't resolve the selector
        };

        // if the opcode is an JUMPI and it matched the selector, the next jumpi is the entry point
        if call.last_instruction.opcode == 0x57 {
            let jump_condition = call.last_instruction.input_operations[1].solidify();
            let jump_taken = call.last_instruction.inputs[1].try_into().unwrap_or(1);

            if jump_condition.contains(selector) &&
                jump_condition.contains("msg.data[0]") &&
                jump_condition.contains(" == ") &&
                jump_taken == 1
            {
                return call.last_instruction.inputs[0].try_into().unwrap_or(0);
            } else if jump_taken == 1 {
                // if handled_jumps contains the jumpi, we have already handled this jump.
                // loops aren't supported in the dispatcher, so we can just return 0
                if handled_jumps.contains(&call.last_instruction.inputs[0].try_into().unwrap_or(0))
                {
                    return 0;
                } else {
                    handled_jumps.insert(call.last_instruction.inputs[0].try_into().unwrap_or(0));
                }
            }
        }

        if vm.exitcode != 255 || !vm.returndata.is_empty() {
            break;
        }
    }

    0
}

/// Resolve a selector's entry point using Vyper's O(1) bucket-based dispatcher pattern.
///
/// Vyper's dispatcher works by:
/// 1. Extracting the 4-byte selector via CALLDATALOAD(0) >> 224
/// 2. Computing a bucket index (selector % n_buckets)
/// 3. Jumping to the bucket
/// 4. Linear probing within the bucket, comparing against each stored selector
/// 5. Jumping to the function when a match is found
///
/// This function executes the VM with the given selector and looks for the JUMPI
/// that has an EQ condition matching the selector, indicating the function entry point.
/// It handles both gas-optimized (sparse table) and code-size optimized (perfect hash) variants.
fn resolve_vyper_entry_point(vm: &mut VM, selector: &str) -> u128 {
    let mut handled_jumps = HashSet::new();

    // set calldata to the selector padded to 36 bytes
    vm.calldata = decode_hex(selector).expect("Failed to decode selector.");
    // pad to at least 36 bytes so CALLDATALOAD(0) and further reads work
    vm.calldata.resize(36, 0);

    let mut max_steps: u32 = 10000;

    while vm.bytecode.len() >= vm.instruction as usize && max_steps > 0 {
        max_steps -= 1;
        let call = match vm.step() {
            Ok(call) => call,
            Err(_) => break,
        };

        // look for JUMPI with selector comparison
        if call.last_instruction.opcode == 0x57 {
            let jump_condition = call.last_instruction.input_operations[1].solidify();
            let jump_taken: u128 = call.last_instruction.inputs[1].try_into().unwrap_or(0);

            // in vyper's dispatcher, the selector comparison appears as:
            // EQ(calldataload(0) >> 224, hardcoded_selector) or similar pattern
            // the solidified form contains "msg.data" and " == " with the selector value
            if jump_condition.contains("msg.data") && jump_condition.contains(" == ") {
                // check if the comparison involves our selector
                if jump_condition.contains(selector) && jump_taken == 1 {
                    return call.last_instruction.inputs[0].try_into().unwrap_or(0);
                }
            }

            // track jumps to detect loops (which shouldn't occur in dispatchers)
            if jump_taken == 1 {
                let dest: u128 = call.last_instruction.inputs[0].try_into().unwrap_or(0);
                if handled_jumps.contains(&dest) {
                    return 0;
                }
                handled_jumps.insert(dest);
            }
        }

        if vm.exitcode != 255 || !vm.returndata.is_empty() {
            break;
        }
    }

    0
}

/// Resolve a list of selectors to their function signatures.
pub async fn resolve_selectors<T>(selectors: Vec<String>) -> HashMap<String, Vec<T>>
where
    T: ResolveSelector + Send + Clone + 'static, {
    // short-circuit if there are no selectors
    if selectors.is_empty() {
        return HashMap::new();
    }

    let resolved_functions: Arc<Mutex<HashMap<String, Vec<T>>>> =
        Arc::new(Mutex::new(HashMap::new()));
    let mut threads = Vec::new();
    let start_time = Instant::now();
    let selector_count = selectors.len();

    for selector in selectors {
        let function_clone = resolved_functions.clone();

        // create a new thread for each selector
        threads.push(task::spawn(async move {
            if let Ok(Some(function)) = T::resolve(&selector).await {
                let mut _resolved_functions =
                    function_clone.lock().expect("Could not obtain lock on function_clone.");
                _resolved_functions.insert(selector, function);
            }
        }));
    }

    // wait for all threads to finish
    for thread in threads {
        if let Err(e) = thread.await {
            // Handle error
            error!("failed to resolve selector: {:?}", e);
        }
    }

    let signatures =
        resolved_functions.lock().expect("failed to obtain lock on resolved_functions.").clone();
    if signatures.is_empty() {
        warn!("failed to resolve any signatures from {} selectors", selector_count);
    }
    info!("resolved {} signatures from {} selectors", signatures.len(), selector_count);
    debug!("signature resolution took {:?}", start_time.elapsed());
    signatures
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::Address;

    /// Construct a Vyper-style dispatcher bytecode with known selectors.
    /// Contains 3 selectors: transfer (0xa9059cbb), balanceOf (0x70a08231), approve (0x095ea7b3)
    /// with entry points at 0x3b, 0x3d, and 0x3f respectively.
    /// Includes Vyper CBOR metadata suffix so compiler detection identifies it as Vyper.
    fn vyper_test_bytecode() -> Vec<u8> {
        vec![
            // Vyper prefix: PUSH1 4, CALLDATASIZE, LT, ISZERO, PUSH2 0x000e, JUMPI
            0x60, 0x04, 0x36, 0x10, 0x15, 0x61, 0x00, 0x0e, 0x57,
            // revert path for short calldata
            0x60, 0x00, 0x60, 0x00, 0xfd,
            // JUMPDEST: dispatcher start
            0x5b,
            // extract selector: PUSH1 0, CALLDATALOAD, PUSH1 0xe0, SHR
            0x60, 0x00, 0x35, 0x60, 0xe0, 0x1c,
            // compare with transfer (0xa9059cbb)
            0x80, 0x63, 0xa9, 0x05, 0x9c, 0xbb, 0x14, 0x61, 0x00, 0x3b, 0x57,
            // compare with balanceOf (0x70a08231)
            0x80, 0x63, 0x70, 0xa0, 0x82, 0x31, 0x14, 0x61, 0x00, 0x3d, 0x57,
            // compare with approve (0x095ea7b3)
            0x80, 0x63, 0x09, 0x5e, 0xa7, 0xb3, 0x14, 0x61, 0x00, 0x3f, 0x57,
            // fallthrough: revert
            0x60, 0x00, 0x60, 0x00, 0xfd,
            // entry points
            0x5b, 0x00, // transfer @ 0x3b
            0x5b, 0x00, // balanceOf @ 0x3d
            0x5b, 0x00, // approve @ 0x3f
            // Vyper CBOR metadata: "vyper" + 0x83 + version (0.3.10)
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

    // --- extract_selector_from_condition tests ---

    #[test]
    fn test_extract_selector_simple_rhs() {
        let condition = "msg.data[0x00] >> 0xe0 == 0xa9059cbb";
        let result = extract_selector_from_condition(condition);
        assert_eq!(result, Some("0xa9059cbb".to_string()));
    }

    #[test]
    fn test_extract_selector_simple_lhs() {
        let condition = "0xa9059cbb == msg.data[0x00] >> 0xe0";
        let result = extract_selector_from_condition(condition);
        assert_eq!(result, Some("0xa9059cbb".to_string()));
    }

    #[test]
    fn test_extract_selector_short_hex() {
        // selector with leading zeros stripped (e.g., 0x95ea7b3 instead of 0x095ea7b3)
        let condition = "msg.data[0x00] >> 0xe0 == 0x95ea7b3";
        let result = extract_selector_from_condition(condition);
        assert_eq!(result, Some("0x095ea7b3".to_string()));
    }

    #[test]
    fn test_extract_selector_no_match() {
        let condition = "0x01 > 0x02";
        let result = extract_selector_from_condition(condition);
        assert_eq!(result, None);
    }

    #[test]
    fn test_extract_selector_no_msg_data() {
        let condition = "0xa9059cbb == 0xdeadbeef";
        let result = extract_selector_from_condition(condition);
        assert_eq!(result, None);
    }

    #[test]
    fn test_extract_selector_nested_eq() {
        // multiple == signs: should return None
        let condition = "msg.data[0x00] == 0x01 == 0x02";
        let result = extract_selector_from_condition(condition);
        assert_eq!(result, None);
    }

    // --- Vyper selector detection tests ---

    #[test]
    fn test_find_vyper_selectors_discovers_all_selectors() {
        let bytecode = vyper_test_bytecode();
        let evm = create_vm(&bytecode);

        let selectors = find_vyper_selectors(&evm);

        assert!(
            selectors.contains_key("0xa9059cbb"),
            "should detect transfer selector 0xa9059cbb, found: {:?}",
            selectors.keys().collect::<Vec<_>>()
        );
        assert!(
            selectors.contains_key("0x70a08231"),
            "should detect balanceOf selector 0x70a08231, found: {:?}",
            selectors.keys().collect::<Vec<_>>()
        );
        assert!(
            selectors.contains_key("0x095ea7b3"),
            "should detect approve selector 0x095ea7b3, found: {:?}",
            selectors.keys().collect::<Vec<_>>()
        );
        assert_eq!(selectors.len(), 3, "should find exactly 3 selectors");
    }

    #[test]
    fn test_find_vyper_selectors_correct_entry_points() {
        let bytecode = vyper_test_bytecode();
        let evm = create_vm(&bytecode);

        let selectors = find_vyper_selectors(&evm);

        // entry points at 0x3b, 0x3d, 0x3f
        assert_eq!(selectors.get("0xa9059cbb"), Some(&0x3b_u128), "transfer entry point");
        assert_eq!(selectors.get("0x70a08231"), Some(&0x3d_u128), "balanceOf entry point");
        assert_eq!(selectors.get("0x095ea7b3"), Some(&0x3f_u128), "approve entry point");
    }

    #[test]
    fn test_resolve_vyper_entry_point_transfer() {
        let bytecode = vyper_test_bytecode();
        let mut vm = create_vm(&bytecode);

        let entry = resolve_vyper_entry_point(&mut vm, "0xa9059cbb");
        assert_eq!(entry, 0x3b, "transfer entry point should be 0x3b");
    }

    #[test]
    fn test_resolve_vyper_entry_point_balance_of() {
        let bytecode = vyper_test_bytecode();
        let mut vm = create_vm(&bytecode);

        let entry = resolve_vyper_entry_point(&mut vm, "0x70a08231");
        assert_eq!(entry, 0x3d, "balanceOf entry point should be 0x3d");
    }

    #[test]
    fn test_resolve_vyper_entry_point_approve() {
        let bytecode = vyper_test_bytecode();
        let mut vm = create_vm(&bytecode);

        let entry = resolve_vyper_entry_point(&mut vm, "0x095ea7b3");
        assert_eq!(entry, 0x3f, "approve entry point should be 0x3f");
    }

    #[test]
    fn test_resolve_vyper_entry_point_unknown_selector() {
        let bytecode = vyper_test_bytecode();
        let mut vm = create_vm(&bytecode);

        let entry = resolve_vyper_entry_point(&mut vm, "0xdeadbeef");
        assert_eq!(entry, 0, "unknown selector should return entry point 0");
    }

    #[test]
    fn test_find_function_selectors_vyper_compiler_detection() {
        // the bytecode includes vyper CBOR metadata, so compiler detection
        // should route to the vyper strategy
        let bytecode = vyper_test_bytecode();
        let evm = create_vm(&bytecode);

        let (compiler, _) = heimdall_common::ether::compiler::detect_compiler(&bytecode);
        assert_eq!(compiler, Compiler::Vyper, "should detect Vyper compiler");

        // find_function_selectors should use vyper path and find all selectors
        let selectors = find_function_selectors(&evm, "");

        assert_eq!(selectors.len(), 3, "should find 3 selectors via vyper path");
        assert!(selectors.contains_key("0xa9059cbb"), "should find transfer");
        assert!(selectors.contains_key("0x70a08231"), "should find balanceOf");
        assert!(selectors.contains_key("0x095ea7b3"), "should find approve");
    }

    #[test]
    fn test_resolve_entry_point_with_vyper_compiler() {
        let bytecode = vyper_test_bytecode();
        let mut vm = create_vm(&bytecode);

        // resolve_entry_point should detect Vyper and use vyper resolution
        let entry = resolve_entry_point(&mut vm, "0xa9059cbb");
        assert_eq!(entry, 0x3b, "should resolve transfer entry point via vyper path");
    }

    #[test]
    fn test_find_vyper_selectors_empty_bytecode() {
        // empty bytecode should return no selectors
        let evm = create_vm(&[0x00]);
        let selectors = find_vyper_selectors(&evm);
        assert!(selectors.is_empty(), "empty bytecode should yield no selectors");
    }

    #[test]
    fn test_find_vyper_selectors_no_dispatcher() {
        // bytecode with no dispatcher (just STOP) should return no selectors
        let bytecode = vec![0x00]; // STOP
        let evm = create_vm(&bytecode);
        let selectors = find_vyper_selectors(&evm);
        assert!(selectors.is_empty(), "STOP-only bytecode should yield no selectors");
    }
}
