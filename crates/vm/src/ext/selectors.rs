use hashbrown::{HashMap, HashSet};

use heimdall_common::utils::strings::decode_hex;
use tracing::{info, trace};

use crate::core::vm::VM;

/// find all function selectors in the given EVM assembly.
// TODO: update get_resolved_selectors logic to support vyper, huff
pub fn find_function_selectors(evm: &VM, assembly: &str) -> HashMap<String, u128> {
    let mut function_selectors = HashMap::new();
    let mut handled_selectors = HashSet::new();

    // search through assembly for PUSHN (where N <= 4) instructions, optimistically assuming that
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

/// resolve a selector's function entry point from the EVM bytecode
// TODO: update resolve_entry_point logic to support vyper
fn resolve_entry_point(vm: &mut VM, selector: &str) -> u128 {
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
