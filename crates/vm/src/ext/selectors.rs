use hashbrown::{HashMap, HashSet};
use std::{
    sync::{Arc, Mutex},
    time::Instant,
};

use alloy::primitives::U256;
use eyre::Result;
use heimdall_common::{
    ether::signatures::{ResolveSelector, ResolvedFunction},
    utils::strings::decode_hex,
};
use tokio::task;
use tracing::{debug, error, info, trace, warn};

use crate::core::{
    opcodes::{self, WrappedInput, WrappedOpcode},
    vm::VM,
};

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

/// Find selectors whose calldata projection controls a dispatcher branch.
pub fn find_function_selectors(evm: &VM, assembly: &str) -> HashMap<String, u128> {
    let instructions: Vec<Vec<&str>> =
        assembly.lines().map(|line| line.split_whitespace().collect()).collect();
    let mut candidates: HashSet<String> = HashSet::from(["00000000".to_string()]);
    for (index, instruction) in instructions.iter().enumerate() {
        let Some(opcode) = instruction.get(1) else { continue };
        let short_comparison = matches!(*opcode, "PUSH1" | "PUSH2" | "PUSH3") &&
            instructions
                .get(index + 1)
                .and_then(|next| next.get(1))
                .is_some_and(|next| matches!(*next, "EQ" | "SUB" | "XOR"));
        if *opcode == "PUSH4" || short_comparison {
            if let Some(value) = instruction
                .get(2)
                .and_then(|value| u32::from_str_radix(value.trim_start_matches("0x"), 16).ok())
            {
                candidates.insert(format!("{value:08x}"));
            }
        }
    }
    let mut candidates: Vec<_> = candidates.into_iter().collect();
    candidates.sort();
    let mut selectors = HashMap::new();
    for selector in candidates {
        let entry = resolve_entry_point(&mut evm.clone(), &selector);
        if entry != 0 {
            selectors.insert(selector, entry);
        }
    }
    info!("discovered {} function selectors in assembly", selectors.len());
    selectors
}

fn is_selector_projection(input: &WrappedInput, calldata_words: &HashSet<WrappedOpcode>) -> bool {
    let WrappedInput::Opcode(operation) = input else { return false };
    match (operation.opcode, operation.inputs.as_slice()) {
        (opcodes::SHR, [shift, WrappedInput::Opcode(word)]) => {
            shift.pushed_value() == Some(U256::from(224)) && calldata_words.contains(word.as_ref())
        }
        (opcodes::DIV, [WrappedInput::Opcode(word), divisor]) => {
            divisor.pushed_value() == Some(U256::from(1) << 224) &&
                calldata_words.contains(word.as_ref())
        }
        (opcodes::AND, [left, right]) => {
            (left.pushed_value() == Some(U256::from(u32::MAX)) &&
                is_selector_projection(right, calldata_words)) ||
                (right.pushed_value() == Some(U256::from(u32::MAX)) &&
                    is_selector_projection(left, calldata_words))
        }
        _ => false,
    }
}

fn selector_branch_condition(
    operation: &WrappedOpcode,
    selector: U256,
    calldata_words: &HashSet<WrappedOpcode>,
) -> Option<bool> {
    if operation.depth() > 64 {
        return None;
    }
    match (operation.opcode, operation.inputs.as_slice()) {
        (opcodes::EQ | opcodes::SUB | opcodes::XOR, [left, right]) => {
            let matches = (left.pushed_value() == Some(selector) &&
                is_selector_projection(right, calldata_words)) ||
                (right.pushed_value() == Some(selector) &&
                    is_selector_projection(left, calldata_words));
            matches.then_some(operation.opcode == opcodes::EQ)
        }
        (opcodes::ISZERO, [input])
            if selector.is_zero() && is_selector_projection(input, calldata_words) =>
        {
            Some(true)
        }
        (opcodes::ISZERO, [WrappedInput::Opcode(inner)]) => {
            selector_branch_condition(inner, selector, calldata_words).map(|condition| !condition)
        }
        _ => None,
    }
}

/// Resolve both taken and fallthrough selector branches without matching rendered source text.
pub fn resolve_entry_point(vm: &mut VM, selector: &str) -> u128 {
    let Ok(calldata) = decode_hex(selector) else { return 0 };
    if calldata.len() != 4 {
        return 0;
    }
    let selector = U256::from_be_slice(&calldata);
    vm.calldata = calldata;
    let mut calldata_words = HashSet::new();
    let mut handled_jumps = HashSet::new();
    for _ in 0..vm.bytecode.len().saturating_mul(4).min(100_000) {
        let Ok(state) = vm.step() else { break };
        let instruction = state.last_instruction;
        if instruction.opcode == opcodes::CALLDATALOAD &&
            instruction.inputs.first() == Some(&U256::ZERO)
        {
            calldata_words.extend(instruction.output_operations.iter().cloned());
        }
        if instruction.opcode == opcodes::JUMPI {
            if !handled_jumps.insert(instruction.instruction) {
                break;
            }
            if let Some(condition) = instruction.input_operations.get(1).and_then(|operation| {
                selector_branch_condition(operation, selector, &calldata_words)
            }) {
                if instruction.inputs.get(1).is_some_and(|value| value.is_zero() != condition) &&
                    vm.exitcode == 255
                {
                    return vm.instruction.saturating_sub(1);
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
