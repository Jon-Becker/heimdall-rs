use hashbrown::{HashMap, HashSet};
use std::{
    sync::{Arc, Mutex},
    time::Instant,
};

use eyre::Result;
use heimdall_common::{
    ether::signatures::{ResolveSelector, ResolvedFunction},
    utils::strings::decode_hex,
};
use tokio::task;
use tracing::{debug, error, info, trace, warn};

use crate::core::vm::VM;

/// Enhanced selector detection that supports multiple compiler patterns
#[derive(Debug)]
pub struct EnhancedSelectorDetector {
    vm: VM,
    selectors: HashMap<String, u128>,
    /// Set of selectors that have already been processed
    pub handled_selectors: HashSet<String>,
}

impl EnhancedSelectorDetector {
    /// Create a new enhanced selector detector with the given VM
    pub fn new(vm: VM) -> Self {
        Self { vm, selectors: HashMap::new(), handled_selectors: HashSet::new() }
    }

    /// Find all function selectors using pattern matching similar to evmole
    pub fn find_selectors(&mut self, assembly: &str) -> HashMap<String, u128> {
        trace!("Starting selector detection on {} bytes of assembly", assembly.len());

        // First pass: Look for PUSH4 patterns (Solidity)
        self.find_push4_selectors(assembly);
        trace!("After PUSH4 pass: found {} selectors", self.handled_selectors.len());

        // Second pass: Look for signature comparison patterns
        self.find_comparison_selectors(assembly);
        trace!("After comparison pass: found {} selectors", self.handled_selectors.len());

        // Third pass: Look for Vyper-specific patterns
        self.find_vyper_selectors(assembly);
        trace!("After Vyper pass: found {} selectors", self.handled_selectors.len());

        self.selectors.clone()
    }

    /// Find selectors using PUSH4 instructions (Solidity pattern)
    fn find_push4_selectors(&mut self, assembly: &str) {
        let lines: Vec<String> = assembly.split('\n').map(|line| line.trim().to_string()).collect();

        for line in lines.iter() {
            let parts: Vec<String> = line.split_whitespace().map(|s| s.to_string()).collect();

            if parts.len() >= 3 && parts[1] == "PUSH4" {
                // Handle both formats: with and without 0x prefix
                let selector = if parts[2].starts_with("0x") {
                    parts[2].clone()
                } else {
                    format!("0x{}", parts[2])
                };

                if self.handled_selectors.contains(&selector) {
                    continue;
                }

                trace!("Found PUSH4 selector: {}", selector);
                self.handled_selectors.insert(selector.clone());

                // Find the entry point for this selector
                if let Some(entry_point) = self.resolve_entry_point(&selector) {
                    self.selectors.insert(selector.clone(), entry_point);
                }
            }
        }
    }

    /// Find selectors using comparison patterns (XOR, EQ, SUB)
    fn find_comparison_selectors(&mut self, assembly: &str) {
        let lines: Vec<String> = assembly.split('\n').map(|line| line.trim().to_string()).collect();
        let mut i = 0;

        while i < lines.len() {
            let parts: Vec<String> =
                lines[i].split_whitespace().map(|arg| arg.to_string()).collect();

            if parts.len() >= 2 {
                let opcode = &parts[1];

                // Look for signature comparison patterns
                if matches!(opcode.as_str(), "XOR" | "EQ" | "SUB") {
                    // For comparison operations, look for PUSH4 before the operation
                    // This handles patterns like: PUSH4 selector, XOR/EQ/SUB
                    if let Some(selector) = self.extract_selector_from_comparison(&lines, i) {
                        if !self.handled_selectors.contains(&selector) {
                            trace!("Found selector via {} comparison: {}", opcode, selector);
                            self.handled_selectors.insert(selector.clone());

                            if let Some(entry_point) = self.resolve_entry_point(&selector) {
                                self.selectors.insert(selector, entry_point);
                            }
                        }
                    }
                }
            }
            i += 1;
        }
    }

    /// Find Vyper-specific selector patterns
    fn find_vyper_selectors(&mut self, assembly: &str) {
        let lines: Vec<String> = assembly.split('\n').map(|line| line.trim().to_string()).collect();
        let mut i = 0;

        while i < lines.len() {
            let parts: Vec<String> =
                lines[i].split_whitespace().map(|arg| arg.to_string()).collect();

            if parts.len() >= 2 {
                let opcode = &parts[1];

                // Vyper uses MOD or AND for hash table dispatch
                if matches!(opcode.as_str(), "MOD" | "AND") &&
                    self.is_vyper_dispatch_pattern(&lines, i)
                {
                    // Extract selectors from Vyper's dispatch table
                    let selectors = self.extract_vyper_selectors(&lines, i);
                    for selector in selectors {
                        if !self.handled_selectors.contains(&selector) {
                            trace!("Found Vyper selector: {}", selector);
                            self.handled_selectors.insert(selector.clone());

                            if let Some(entry_point) = self.resolve_entry_point(&selector) {
                                self.selectors.insert(selector, entry_point);
                            }
                        }
                    }
                }

                // Also look for MUL/SHR patterns used by Vyper
                if (opcode == "MUL" || opcode == "SHR") &&
                    self.is_vyper_signature_pattern(&lines, i)
                {
                    if let Some(selector) = self.extract_selector_from_vyper_pattern(&lines, i) {
                        if !self.handled_selectors.contains(&selector) {
                            trace!("Found Vyper selector via MUL/SHR: {}", selector);
                            self.handled_selectors.insert(selector.clone());

                            if let Some(entry_point) = self.resolve_entry_point(&selector) {
                                self.selectors.insert(selector, entry_point);
                            }
                        }
                    }
                }
            }
            i += 1;
        }
    }

    /// Check if the current position is a signature comparison
    fn is_signature_comparison(&self, lines: &[String], pos: usize) -> bool {
        // Look for patterns that involve calldata[0:4] comparisons
        if pos > 0 && pos < lines.len() - 1 {
            // Check previous instructions for calldata loading
            for line in lines.iter().take(pos).skip(pos.saturating_sub(5)) {
                if line.contains("CALLDATALOAD") ||
                    line.contains("CALLDATACOPY") ||
                    (line.contains("PUSH1") && line.contains("0x00"))
                {
                    return true;
                }
            }
        }
        false
    }

    /// Extract selector from a comparison operation
    fn extract_selector_from_comparison(&self, lines: &[String], pos: usize) -> Option<String> {
        // Look backwards for a PUSH4 or similar that contains the selector
        for line in lines.iter().take(pos).skip(pos.saturating_sub(10)) {
            let parts: Vec<String> = line.split_whitespace().map(|s| s.to_string()).collect();
            if parts.len() >= 3 && parts[1] == "PUSH4" {
                // Handle both formats: with and without 0x prefix
                let selector = if parts[2].starts_with("0x") {
                    parts[2].clone()
                } else {
                    format!("0x{}", parts[2])
                };
                return Some(selector);
            }
        }
        None
    }

    /// Check if this is a Vyper dispatch pattern
    fn is_vyper_dispatch_pattern(&self, lines: &[String], pos: usize) -> bool {
        // Vyper uses patterns like: sig MOD n_buckets or sig AND (n_buckets-1)
        // Look for CALLDATALOAD or SHR before the MOD/AND operation
        for line in lines.iter().take(pos).skip(pos.saturating_sub(5)) {
            if line.contains("CALLDATALOAD") || line.contains("SHR") {
                // Found signature loading, now check if this leads to dispatch
                // Vyper typically follows with EQ comparison and JUMPI
                return true;
            }
        }
        false
    }

    /// Extract selectors from Vyper's dispatch pattern
    fn extract_vyper_selectors(&self, lines: &[String], pos: usize) -> Vec<String> {
        let mut selectors = Vec::new();

        // Look for selector values in the surrounding code
        // Vyper often has selectors after the MOD/AND operation
        let start = pos;
        let end = std::cmp::min(pos + 30, lines.len());

        for line in lines.iter().take(end).skip(start) {
            let parts: Vec<String> = line.split_whitespace().map(|s| s.to_string()).collect();
            if parts.len() >= 3 && parts[1] == "PUSH4" {
                // Handle both formats: with and without 0x prefix
                let selector = if parts[2].starts_with("0x") {
                    parts[2].clone()
                } else {
                    format!("0x{}", parts[2])
                };
                selectors.push(selector);
            }
        }

        selectors
    }

    /// Check if this is a Vyper signature pattern (MUL/SHR)
    fn is_vyper_signature_pattern(&self, lines: &[String], pos: usize) -> bool {
        // Check for signature manipulation patterns
        if pos > 0 {
            let prev_line = &lines[pos - 1];
            if prev_line.contains("CALLDATALOAD") || prev_line.contains("DUP") {
                return true;
            }
        }
        false
    }

    /// Extract selector from Vyper MUL/SHR pattern
    fn extract_selector_from_vyper_pattern(&self, lines: &[String], pos: usize) -> Option<String> {
        // Similar to extract_selector_from_comparison but for Vyper patterns
        self.extract_selector_from_comparison(lines, pos)
    }

    /// Resolve the entry point for a given selector
    fn resolve_entry_point(&mut self, selector: &str) -> Option<u128> {
        let mut handled_jumps = HashSet::new();

        // Execute the VM to find the entry point
        self.vm.reset();
        self.vm.calldata = decode_hex(selector).ok()?;

        while self.vm.bytecode.len() >= self.vm.instruction as usize {
            let call = match self.vm.step() {
                Ok(call) => call,
                Err(_) => break,
            };

            // Check for JUMPI that matches our selector
            if call.last_instruction.opcode == 0x57 {
                let jump_condition = call.last_instruction.input_operations[1].solidify();
                let jump_taken = call.last_instruction.inputs[1].try_into().unwrap_or(1);

                if jump_condition.contains(selector) &&
                    jump_condition.contains("msg.data[0]") &&
                    jump_condition.contains(" == ") &&
                    jump_taken == 1
                {
                    return Some(call.last_instruction.inputs[0].try_into().unwrap_or(0));
                } else if jump_taken == 1 {
                    // Check for loops
                    let jump_dest = call.last_instruction.inputs[0].try_into().unwrap_or(0);
                    if handled_jumps.contains(&jump_dest) {
                        return None;
                    }
                    handled_jumps.insert(jump_dest);
                }
            }

            // Also check for Vyper-style entry points
            if call.last_instruction.opcode == 0x56 {
                // JUMP
                // For Vyper, the entry point might be a direct jump
                let jump_dest = call.last_instruction.inputs[0].try_into().unwrap_or(0);
                if jump_dest > 0 && !handled_jumps.contains(&jump_dest) {
                    // Verify this is a valid entry point
                    if self.is_valid_entry_point(jump_dest as usize) {
                        return Some(jump_dest);
                    }
                }
            }

            if self.vm.exitcode != 255 || !self.vm.returndata.is_empty() {
                break;
            }
        }

        None
    }

    /// Check if a given address is a valid function entry point
    fn is_valid_entry_point(&self, address: usize) -> bool {
        // Check if the address points to a JUMPDEST
        if address < self.vm.bytecode.len() {
            return self.vm.bytecode[address] == 0x5b; // JUMPDEST opcode
        }
        false
    }
}

/// Public API function to get enhanced selectors
pub async fn get_enhanced_selectors(
    bytecode: &[u8],
    assembly: &str,
) -> Result<HashMap<String, u128>> {
    let vm = VM::new(
        bytecode,
        &[],
        Default::default(),
        Default::default(),
        Default::default(),
        0,
        u128::MAX,
    );

    let mut detector = EnhancedSelectorDetector::new(vm);
    let selectors = detector.find_selectors(assembly);

    debug!("Enhanced detector found {} selectors", selectors.len());
    Ok(selectors)
}

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
    // Use enhanced selector detection for better accuracy
    let selectors = match get_enhanced_selectors(&evm.bytecode, disassembled_bytecode).await {
        Ok(enhanced_selectors) if !enhanced_selectors.is_empty() => enhanced_selectors,
        _ => {
            // Fallback to basic detection if enhanced fails
            warn!("Enhanced selector detection failed, using fallback");
            find_function_selectors(evm, disassembled_bytecode)
        }
    };

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

/// Legacy function selector finder - kept for backward compatibility
/// Use EnhancedSelectorDetector for better accuracy and Vyper support
pub fn find_function_selectors(evm: &VM, assembly: &str) -> HashMap<String, u128> {
    let vm_clone = evm.clone();
    let mut detector = EnhancedSelectorDetector::new(vm_clone);
    detector.find_selectors(assembly)
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

    #[test]
    fn test_solidity_push4_pattern() {
        // Test bytecode with PUSH4 selector pattern
        let assembly = r#"
            000 PUSH1 0x80
            002 PUSH1 0x40
            004 MSTORE
            005 PUSH1 0x04
            007 CALLDATASIZE
            008 LT
            009 PUSH2 0x0041
            012 JUMPI
            013 PUSH1 0x00
            015 CALLDATALOAD
            016 PUSH1 0xe0
            018 SHR
            019 DUP1
            020 PUSH4 0xa9059cbb
            025 EQ
            026 PUSH2 0x0046
            029 JUMPI
        "#;

        let bytecode = vec![0x60, 0x80]; // Simplified bytecode
        let vm = VM::new(
            &bytecode,
            &[],
            Default::default(),
            Default::default(),
            Default::default(),
            0,
            u128::MAX,
        );
        let mut detector = EnhancedSelectorDetector::new(vm);

        detector.find_push4_selectors(assembly);
        assert!(detector.handled_selectors.contains("0xa9059cbb"));
    }

    #[test]
    fn test_vyper_mod_pattern() {
        // Test Vyper MOD dispatch pattern
        let assembly = r#"
            000 PUSH1 0x00
            002 CALLDATALOAD
            003 PUSH1 0xe0
            005 SHR
            006 PUSH1 0x08
            008 MOD
            009 PUSH4 0x095ea7b3
            014 DUP2
            015 EQ
            016 PUSH2 0x0100
            019 JUMPI
        "#;

        let bytecode = vec![0x60, 0x00]; // Simplified bytecode
        let vm = VM::new(
            &bytecode,
            &[],
            Default::default(),
            Default::default(),
            Default::default(),
            0,
            u128::MAX,
        );
        let mut detector = EnhancedSelectorDetector::new(vm);

        detector.find_vyper_selectors(assembly);
        assert!(detector.handled_selectors.contains("0x095ea7b3"));
    }

    #[test]
    fn test_comparison_pattern() {
        // Test XOR/EQ comparison pattern
        let assembly = r#"
            000 PUSH1 0x00
            002 CALLDATALOAD
            003 PUSH1 0xe0
            005 SHR
            006 PUSH4 0x70a08231
            011 XOR
            012 PUSH2 0x0200
            015 JUMPI
        "#;

        let bytecode = vec![0x60, 0x00]; // Simplified bytecode
        let vm = VM::new(
            &bytecode,
            &[],
            Default::default(),
            Default::default(),
            Default::default(),
            0,
            u128::MAX,
        );
        let mut detector = EnhancedSelectorDetector::new(vm);

        detector.find_comparison_selectors(assembly);
        assert!(detector.handled_selectors.contains("0x70a08231"));
    }

    /// Test data for Solidity contract with multiple selectors
    fn get_solidity_test_assembly() -> &'static str {
        r#"
        000 PUSH1 0x80
        002 PUSH1 0x40
        004 MSTORE
        005 PUSH1 0x04
        007 CALLDATASIZE
        008 LT
        009 PUSH2 0x0056
        012 JUMPI
        013 PUSH1 0x00
        015 CALLDATALOAD
        016 PUSH1 0xe0
        018 SHR
        019 DUP1
        020 PUSH4 0x095ea7b3
        025 EQ
        026 PUSH2 0x005b
        029 JUMPI
        030 DUP1
        031 PUSH4 0x18160ddd
        036 EQ
        037 PUSH2 0x008a
        040 JUMPI
        041 DUP1
        042 PUSH4 0x23b872dd
        047 EQ
        048 PUSH2 0x00a4
        051 JUMPI
        052 DUP1
        053 PUSH4 0x70a08231
        058 EQ
        059 PUSH2 0x00e7
        062 JUMPI
        063 DUP1
        064 PUSH4 0xa9059cbb
        069 EQ
        070 PUSH2 0x011a
        073 JUMPI
        074 DUP1
        075 PUSH4 0xdd62ed3e
        080 EQ
        081 PUSH2 0x0149
        084 JUMPI
        "#
    }

    /// Test data for Vyper contract with hash table dispatch
    fn get_vyper_test_assembly() -> &'static str {
        r#"
        000 PUSH1 0x00
        002 CALLDATALOAD
        003 PUSH1 0xe0
        005 SHR
        006 DUP1
        007 PUSH1 0x07
        009 MOD
        010 DUP1
        011 PUSH1 0x00
        013 EQ
        014 ISZERO
        015 PUSH2 0x0030
        018 JUMPI
        019 DUP2
        020 PUSH4 0x06fdde03
        025 EQ
        026 PUSH2 0x0100
        029 JUMPI
        030 PUSH2 0x0030
        033 JUMP
        034 JUMPDEST
        035 DUP1
        036 PUSH1 0x01
        038 EQ
        039 ISZERO
        040 PUSH2 0x0050
        043 JUMPI
        044 DUP2
        045 PUSH4 0x095ea7b3
        050 EQ
        051 PUSH2 0x0200
        054 JUMPI
        055 PUSH2 0x0050
        058 JUMP
        059 JUMPDEST
        060 DUP1
        061 PUSH1 0x02
        063 EQ
        064 ISZERO
        065 PUSH2 0x0070
        068 JUMPI
        069 DUP2
        070 PUSH4 0x18160ddd
        075 EQ
        076 PUSH2 0x0300
        079 JUMPI
        "#
    }

    /// Test data for contract using comparison patterns (XOR/EQ/SUB)
    fn get_comparison_pattern_assembly() -> &'static str {
        r#"
        000 PUSH1 0x00
        002 CALLDATALOAD
        003 PUSH1 0xe0
        005 SHR
        006 DUP1
        007 PUSH4 0x70a08231
        012 XOR
        013 ISZERO
        014 PUSH2 0x0100
        017 JUMPI
        018 DUP1
        019 PUSH4 0xa9059cbb
        024 SUB
        025 ISZERO
        026 PUSH2 0x0200
        029 JUMPI
        030 DUP1
        031 PUSH4 0x23b872dd
        036 EQ
        037 PUSH2 0x0300
        040 JUMPI
        "#
    }

    /// Test data for Vyper contract with dense selector section
    fn get_vyper_dense_assembly() -> &'static str {
        r#"
        000 PUSH1 0x00
        002 CALLDATALOAD
        003 PUSH1 0xe0
        005 SHR
        006 DUP1
        007 PUSH4 0xffffffff
        012 AND
        013 PUSH1 0x0f
        015 AND
        016 PUSH1 0x02
        018 MUL
        019 PUSH2 0x0050
        022 ADD
        023 JUMP
        050 JUMPDEST
        051 PUSH4 0x06fdde03
        056 DUP2
        057 EQ
        058 PUSH2 0x0400
        061 JUMPI
        062 PUSH4 0x095ea7b3
        067 DUP2
        068 EQ
        069 PUSH2 0x0500
        072 JUMPI
        073 PUSH4 0x18160ddd
        078 DUP2
        079 EQ
        080 PUSH2 0x0600
        083 JUMPI
        "#
    }

    #[test]
    fn test_solidity_standard_selectors() {
        let assembly = get_solidity_test_assembly();
        let bytecode = vec![0x60, 0x80]; // Simplified bytecode for testing

        let vm = VM::new(
            &bytecode,
            &[],
            Default::default(),
            Default::default(),
            Default::default(),
            0,
            u128::MAX,
        );
        let mut detector = EnhancedSelectorDetector::new(vm);
        let _selectors = detector.find_selectors(assembly);

        // Check for standard ERC20 selectors
        assert!(detector.handled_selectors.contains("0x095ea7b3"), "Should find approve selector");
        assert!(
            detector.handled_selectors.contains("0x18160ddd"),
            "Should find totalSupply selector"
        );
        assert!(
            detector.handled_selectors.contains("0x23b872dd"),
            "Should find transferFrom selector"
        );
        assert!(
            detector.handled_selectors.contains("0x70a08231"),
            "Should find balanceOf selector"
        );
        assert!(detector.handled_selectors.contains("0xa9059cbb"), "Should find transfer selector");
        assert!(
            detector.handled_selectors.contains("0xdd62ed3e"),
            "Should find allowance selector"
        );

        assert_eq!(detector.handled_selectors.len(), 6, "Should find all 6 ERC20 selectors");
    }

    #[test]
    fn test_vyper_hash_table_dispatch() {
        let assembly = get_vyper_test_assembly();
        let bytecode = vec![0x60, 0x00]; // Simplified bytecode for testing

        let vm = VM::new(
            &bytecode,
            &[],
            Default::default(),
            Default::default(),
            Default::default(),
            0,
            u128::MAX,
        );
        let mut detector = EnhancedSelectorDetector::new(vm);
        let _selectors = detector.find_selectors(assembly);

        // Check for Vyper-dispatched selectors
        assert!(detector.handled_selectors.contains("0x06fdde03"), "Should find name selector");
        assert!(detector.handled_selectors.contains("0x095ea7b3"), "Should find approve selector");
        assert!(
            detector.handled_selectors.contains("0x18160ddd"),
            "Should find totalSupply selector"
        );

        assert!(detector.handled_selectors.len() >= 3, "Should find at least 3 selectors");
    }

    #[test]
    fn test_comparison_patterns_extended() {
        let assembly = get_comparison_pattern_assembly();
        let bytecode = vec![0x60, 0x00]; // Simplified bytecode for testing

        let vm = VM::new(
            &bytecode,
            &[],
            Default::default(),
            Default::default(),
            Default::default(),
            0,
            u128::MAX,
        );
        let mut detector = EnhancedSelectorDetector::new(vm);
        let _selectors = detector.find_selectors(assembly);

        // Check for selectors found via comparison patterns
        assert!(detector.handled_selectors.contains("0x70a08231"), "Should find balanceOf via XOR");
        assert!(detector.handled_selectors.contains("0xa9059cbb"), "Should find transfer via SUB");
        assert!(
            detector.handled_selectors.contains("0x23b872dd"),
            "Should find transferFrom via EQ"
        );

        assert_eq!(detector.handled_selectors.len(), 3, "Should find all 3 selectors");
    }

    #[test]
    fn test_vyper_dense_section() {
        let assembly = get_vyper_dense_assembly();
        let bytecode = vec![0x60, 0x00]; // Simplified bytecode for testing

        let vm = VM::new(
            &bytecode,
            &[],
            Default::default(),
            Default::default(),
            Default::default(),
            0,
            u128::MAX,
        );
        let mut detector = EnhancedSelectorDetector::new(vm);
        let _selectors = detector.find_selectors(assembly);

        // Check for selectors in dense section
        assert!(detector.handled_selectors.contains("0x06fdde03"), "Should find name selector");
        assert!(detector.handled_selectors.contains("0x095ea7b3"), "Should find approve selector");
        assert!(
            detector.handled_selectors.contains("0x18160ddd"),
            "Should find totalSupply selector"
        );

        assert!(detector.handled_selectors.len() >= 3, "Should find at least 3 selectors");
    }

    #[test]
    fn test_empty_assembly() {
        let assembly = "";
        let bytecode = vec![];

        let vm = VM::new(
            &bytecode,
            &[],
            Default::default(),
            Default::default(),
            Default::default(),
            0,
            u128::MAX,
        );
        let mut detector = EnhancedSelectorDetector::new(vm);
        let selectors = detector.find_selectors(assembly);

        assert_eq!(selectors.len(), 0, "Should find no selectors in empty assembly");
    }

    #[test]
    fn test_no_selectors() {
        let assembly = r#"
        000 PUSH1 0x80
        002 PUSH1 0x40
        004 MSTORE
        005 PUSH1 0x00
        007 DUP1
        008 REVERT
        "#;
        let bytecode = vec![0x60, 0x80];

        let vm = VM::new(
            &bytecode,
            &[],
            Default::default(),
            Default::default(),
            Default::default(),
            0,
            u128::MAX,
        );
        let mut detector = EnhancedSelectorDetector::new(vm);
        let selectors = detector.find_selectors(assembly);

        assert_eq!(selectors.len(), 0, "Should find no selectors in fallback-only contract");
    }

    #[test]
    fn test_duplicate_selectors() {
        let assembly = r#"
        000 PUSH4 0x70a08231
        005 DUP1
        006 PUSH4 0x70a08231
        011 EQ
        012 PUSH2 0x0100
        015 JUMPI
        016 PUSH4 0x70a08231
        021 XOR
        022 PUSH2 0x0200
        025 JUMPI
        "#;
        let bytecode = vec![0x60, 0x00];

        let vm = VM::new(
            &bytecode,
            &[],
            Default::default(),
            Default::default(),
            Default::default(),
            0,
            u128::MAX,
        );
        let mut detector = EnhancedSelectorDetector::new(vm);
        let _selectors = detector.find_selectors(assembly);

        // Should only have one entry for the duplicate selector
        assert_eq!(detector.handled_selectors.len(), 1, "Should deduplicate selectors");
        assert!(detector.handled_selectors.contains("0x70a08231"), "Should contain the selector");
    }

    /// Integration test with async interface
    #[tokio::test]
    async fn test_get_enhanced_selectors_async() {
        let assembly = get_solidity_test_assembly();
        // Create more complete bytecode to ensure proper VM operation
        let bytecode = vec![
            0x60, 0x80, 0x60, 0x40, 0x52, // PUSH1 0x80 PUSH1 0x40 MSTORE
            0x60, 0x04, 0x36, 0x10, // PUSH1 0x04 CALLDATASIZE LT
            0x61, 0x00, 0x56, 0x57, // PUSH2 0x0056 JUMPI
            0x60, 0x00, 0x35, // PUSH1 0x00 CALLDATALOAD
            0x60, 0xe0, 0x1c, // PUSH1 0xe0 SHR
            0x80, // DUP1
            0x63, 0x09, 0x5e, 0xa7, 0xb3, // PUSH4 0x095ea7b3
            0x14, // EQ
            0x61, 0x00, 0x5b, 0x57, // PUSH2 0x005b JUMPI
        ];

        let result = get_enhanced_selectors(&bytecode, assembly).await;
        assert!(result.is_ok(), "Should successfully get enhanced selectors");

        let selectors = result.unwrap();
        // The async function should find selectors from the PUSH4 patterns
        assert!(selectors.len() >= 1, "Should find at least one selector");
    }
}
