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
/// Supports both Solidity-style and Vyper-style selector dispatch patterns:
/// - **Solidity**: Uses `PUSH4 <selector>` followed by `EQ` + `JUMPI` (jump-if-equal)
/// - **Vyper sparse** (`_selector_section_sparse`): Uses `PUSH4 <selector>` followed by `XOR` +
///   `JUMPI` (skip-if-not-equal) or `SUB` + `JUMPI`
/// - **Vyper dense** (`_selector_section_dense`): Uses `MOD`/`AND` for bucket selection, then
///   `EQ`/`XOR` comparisons within buckets
///
/// In all cases, PUSH4 instructions are scanned from the disassembly, and the VM is used
/// to symbolically execute the dispatcher to find each selector's entry point.
pub fn find_function_selectors(evm: &VM, assembly: &str) -> HashMap<String, u128> {
    let mut function_selectors = HashMap::new();
    let mut handled_selectors = HashSet::new();

    // search through assembly for PUSH4 instructions, optimistically assuming that
    // they are function selectors. This works for both Solidity and Vyper compilers,
    // as both use PUSH4 to push 4-byte function selectors onto the stack.
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

                // get the function's entry point using symbolic execution.
                // resolve_entry_point handles both Solidity (EQ+JUMPI) and
                // Vyper (XOR+JUMPI, SUB+JUMPI, ISZERO+EQ+JUMPI) dispatch patterns.
                let function_entry_point =
                    match resolve_entry_point(&mut evm.clone(), &function_selector) {
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

    info!("discovered {} function selectors in assembly", function_selectors.len());
    function_selectors
}

/// Resolve a selector's function entry point from the EVM bytecode.
///
/// Supports multiple dispatch patterns used by different compilers:
///
/// ## Solidity pattern (EQ + JUMPI, jump-if-equal)
/// ```text
/// PUSH4 <selector>
/// CALLDATALOAD(0) >> 0xe0
/// EQ                        ; 1 if selector matches
/// PUSH2 <function_target>
/// JUMPI                     ; jump to function if EQ=1
/// ```
///
/// ## Vyper sparse pattern (`_selector_section_sparse`, XOR/SUB + JUMPI, skip-if-not-equal)
/// ```text
/// PUSH4 <selector>
/// DUP2                      ; copy extracted calldata selector
/// XOR                       ; 0 if selector matches
/// PUSH2 <skip_target>
/// JUMPI                     ; skip to next check if XOR!=0 (not matching)
/// ; ... function body starts here when selector matches ...
/// ```
///
/// ## Vyper dense pattern (`_selector_section_dense`, with bucket selection)
/// ```text
/// MOD/AND                   ; compute bucket index
/// ; ... jump table to bucket ...
/// ; in bucket:
/// PUSH4 <selector>
/// DUP2
/// EQ
/// ISZERO                    ; 1 if NOT matching
/// PUSH2 <skip_target>
/// JUMPI                     ; skip if not matching
/// ; ... function body starts here when selector matches ...
/// ```
pub fn resolve_entry_point(vm: &mut VM, selector: &str) -> u128 {
    let mut handled_jumps = HashSet::new();

    // execute the EVM call to find the entry point for the given selector
    vm.calldata = decode_hex(selector).expect("Failed to decode selector.");
    while vm.bytecode.len() >= vm.instruction as usize {
        let call = match vm.step() {
            Ok(call) => call,
            Err(_) => break, // the call failed, so we can't resolve the selector
        };

        // check JUMPI instructions for selector dispatch patterns
        if call.last_instruction.opcode == 0x57 {
            let jump_condition = call.last_instruction.input_operations[1].solidify();
            let jump_taken: u128 = call.last_instruction.inputs[1].try_into().unwrap_or(1);

            // Solidity pattern: EQ + JUMPI (jump-if-equal)
            // When the selector matches, EQ returns 1, JUMPI takes the jump.
            // The jump target (inputs[0]) is the function entry point.
            if jump_condition.contains(selector) &&
                jump_condition.contains("msg.data[0]") &&
                jump_condition.contains(" == ") &&
                !jump_condition.contains('!') &&
                jump_taken == 1
            {
                return call.last_instruction.inputs[0].try_into().unwrap_or(0);
            }

            // Vyper pattern: XOR + JUMPI (skip-if-not-equal)
            // Vyper's _selector_section_sparse uses XOR(selector, calldata_selector).
            // XOR returns 0 when selectors match, so JUMPI does NOT jump.
            // The entry point is the instruction right after JUMPI (vm.instruction).
            if jump_condition.contains(selector) &&
                jump_condition.contains("msg.data[0]") &&
                jump_condition.contains(" ^ ") &&
                jump_taken == 0
            {
                trace!(
                    "vyper XOR dispatch: selector {} matched, entry point at {}",
                    selector,
                    vm.instruction
                );
                return vm.instruction;
            }

            // Vyper pattern: SUB + JUMPI (skip-if-not-equal)
            // Some Vyper versions use SUB instead of XOR for equality checking.
            // SUB returns 0 when selectors are equal, so JUMPI does NOT jump.
            if jump_condition.contains(selector) &&
                jump_condition.contains("msg.data[0]") &&
                jump_condition.contains(" - ") &&
                jump_taken == 0
            {
                trace!(
                    "vyper SUB dispatch: selector {} matched, entry point at {}",
                    selector,
                    vm.instruction
                );
                return vm.instruction;
            }

            // Vyper dense pattern: ISZERO(EQ(...)) + JUMPI (skip-if-not-equal)
            // In Vyper's _selector_section_dense, the comparison may be negated:
            // ISZERO(EQ(selector, calldata_selector)) = 1 when NOT matching.
            // JUMPI skips to next bucket/selector. When matching, ISZERO gives 0,
            // JUMPI does NOT jump, and execution falls through to the function body.
            if jump_condition.contains(selector) &&
                jump_condition.contains("msg.data[0]") &&
                jump_condition.contains(" == ") &&
                jump_condition.contains('!') &&
                jump_taken == 0
            {
                trace!(
                    "vyper ISZERO+EQ dispatch: selector {} matched, entry point at {}",
                    selector,
                    vm.instruction
                );
                return vm.instruction;
            }

            // Handle non-matching jumps (for loop detection)
            if jump_taken == 1 {
                // if handled_jumps contains the jumpi, we have already handled this jump.
                // loops aren't supported in the dispatcher, so we can just return 0
                let jump_target: u128 = call.last_instruction.inputs[0].try_into().unwrap_or(0);
                if handled_jumps.contains(&jump_target) {
                    return 0;
                } else {
                    handled_jumps.insert(jump_target);
                }
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
