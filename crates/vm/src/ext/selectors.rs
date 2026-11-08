use hashbrown::{HashMap, HashSet};
use std::time::Instant;

use eyre::Result;
use heimdall_common::{
    ether::signatures::{ResolveSelector, ResolvedFunction},
    utils::strings::{decode_hex, encode_hex},
};
use tokio::task;
use tracing::{debug, error, info, trace, warn};

use crate::core::{opcodes, program::Program, vm::VM};

/// Finds selectors in disassembled bytecode and optionally resolves their signatures.
///
/// # Arguments
/// * `disassembled_bytecode` - The disassembled bytecode to analyze
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

/// Find function selectors from the supplied EVM assembly.
///
/// This compatibility entry point honors the provided assembly. For analysis of the entire
/// bytecode without a formatting dependency, use [`find_function_selectors_in_bytecode`].
pub fn find_function_selectors(evm: &VM, assembly: &str) -> HashMap<String, u128> {
    let candidates = assembly.lines().filter_map(|line| {
        let mut parts = line.split_whitespace();
        parts.next()?;
        (parts.next()? == "PUSH4").then(|| parts.next()).flatten().map(str::to_owned)
    });
    resolve_candidates(evm, candidates)
}

/// Find function selectors using the VM's decoded bytecode.
pub fn find_function_selectors_in_bytecode(evm: &VM) -> HashMap<String, u128> {
    let program = Program::decode(&evm.bytecode, evm.hardfork);
    let candidates = program
        .instructions
        .iter()
        .filter(|instruction| instruction.opcode == opcodes::PUSH4 && !instruction.truncated)
        .map(|instruction| encode_hex(&instruction.immediate));
    resolve_candidates(evm, candidates)
}

fn resolve_candidates(evm: &VM, candidates: impl Iterator<Item = String>) -> HashMap<String, u128> {
    let mut function_selectors = HashMap::new();
    let mut handled_selectors = HashSet::new();
    for selector in candidates {
        if !handled_selectors.insert(selector.clone()) {
            continue;
        }
        let entry_point = resolve_entry_point(&mut evm.clone(), &selector);
        if entry_point != 0 {
            trace!("found function selector {} at entry point {}", selector, entry_point);
            function_selectors.insert(selector, entry_point);
        }
    }
    info!("discovered {} function selectors", function_selectors.len());
    function_selectors
}

/// Resolve a selector's function entry point, or return zero if it cannot be found.
///
/// Dispatch execution is limited to 100,000 instructions to bound looping bytecode.
// TODO: update resolve_entry_point logic to support vyper
pub fn resolve_entry_point(vm: &mut VM, selector: &str) -> u128 {
    resolve_entry_point_with_limit(vm, selector, 100_000)
}

fn resolve_entry_point_with_limit(vm: &mut VM, selector: &str, step_limit: usize) -> u128 {
    vm.calldata = decode_hex(selector).expect("Failed to decode selector.");
    let mut steps = 0;
    while vm.bytecode.len() >= vm.instruction as usize && steps < step_limit {
        steps += 1;
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
            }
        }

        if vm.exitcode != 255 || !vm.returndata.is_empty() {
            break;
        }
    }

    if steps == step_limit && vm.bytecode.len() >= vm.instruction as usize && vm.exitcode == 255 {
        warn!("selector dispatcher exceeded the {} instruction limit", step_limit);
    }

    0
}

/// Resolve a list of selectors to their function signatures.
pub async fn resolve_selectors<T>(selectors: Vec<String>) -> HashMap<String, Vec<T>>
where
    T: ResolveSelector + Send + Clone + 'static, {
    if selectors.is_empty() {
        return HashMap::new();
    }

    let start_time = Instant::now();
    let selector_count = selectors.len();
    let tasks = selectors
        .into_iter()
        .map(|selector| {
            task::spawn(async move {
                T::resolve(&selector).await.ok().flatten().map(|functions| (selector, functions))
            })
        })
        .collect::<Vec<_>>();

    let mut signatures = HashMap::new();
    for task in tasks {
        match task.await {
            Ok(Some((selector, functions))) => {
                signatures.insert(selector, functions);
            }
            Ok(None) => {}
            Err(error) => error!("failed to resolve selector: {:?}", error),
        }
    }
    if signatures.is_empty() {
        warn!("failed to resolve any signatures from {} selectors", selector_count);
    }
    info!("resolved {} signatures from {} selectors", signatures.len(), selector_count);
    debug!("signature resolution took {:?}", start_time.elapsed());
    signatures
}

#[cfg(test)]
mod tests {
    use std::{sync::mpsc, time::Duration};

    use alloy::primitives::Address;

    use super::{
        find_function_selectors, find_function_selectors_in_bytecode, resolve_entry_point,
        resolve_entry_point_with_limit, VM,
    };

    #[test]
    fn assembly_api_honors_filtered_input() {
        let bytecode = heimdall_common::utils::strings::decode_hex(include_str!(
            "../../../core/tests/testdata/decompile/multi_selector.hex"
        ))
        .unwrap();
        let vm = VM::new(&bytecode, &[], Address::ZERO, Address::ZERO, Address::ZERO, 0, u128::MAX);
        let all = find_function_selectors_in_bytecode(&vm);
        let filtered = find_function_selectors(&vm, "0 PUSH4 11111101\n");
        assert_eq!(all.len(), 3);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered.get("11111101"), all.get("11111101"));
        assert!(find_function_selectors(&vm, "0 PUSH4").is_empty());
    }

    #[test]
    fn selector_resolution_allows_finite_dispatch_loops() {
        let code = [
            0x60, 3, 0x5b, 0x60, 1, 0x90, 0x03, 0x80, 0x60, 2, 0x57, 0x50, 0x60, 0, 0x35, 0x60,
            0xe0, 0x1c, 0x63, 0xde, 0xad, 0xbe, 0xef, 0x14, 0x60, 0x1c, 0x57, 0, 0x5b, 0,
        ];
        let mut vm = VM::new(&code, &[], Address::ZERO, Address::ZERO, Address::ZERO, 0, u128::MAX);
        assert_eq!(resolve_entry_point(&mut vm, "deadbeef"), 0x1c);
    }

    #[test]
    fn selector_resolution_terminates_on_unconditional_dispatch_loop() {
        let (sender, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            let mut vm = VM::new(
                &[0x5b, 0x60, 0, 0x56],
                &[],
                Address::ZERO,
                Address::ZERO,
                Address::ZERO,
                0,
                u128::MAX,
            );
            let _ = sender.send(resolve_entry_point_with_limit(&mut vm, "deadbeef", 32));
        });
        assert_eq!(receiver.recv_timeout(Duration::from_secs(1)).unwrap(), 0);
    }
}
