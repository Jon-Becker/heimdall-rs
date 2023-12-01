use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
    time::Duration,
};

use indicatif::ProgressBar;
use tokio::task;

use crate::utils::{io::logging::Logger, strings::decode_hex};

use super::{evm::core::vm::VM, signatures::{ResolveSelector, ResolvedFunction}};

// Find all function selectors and all the data associated to this function, represented by
// [`ResolvedFunction`]
pub async fn get_resolved_selectors(
    disassembled_bytecode: &str,
    skip_resolving: &bool,
    evm: &VM,
    shortened_target: &str,
) -> Result<
    (HashMap<String, u128>, HashMap<String, Vec<ResolvedFunction>>),
    Box<dyn std::error::Error>,
> {
    let logger = Logger::default();
    let selectors = find_function_selectors(evm, &disassembled_bytecode);

    let mut resolved_selectors = HashMap::new();
    if !skip_resolving {
        resolved_selectors =
            resolve_selectors::<ResolvedFunction>(selectors.keys().cloned().collect()).await;

        // if resolved selectors are empty, we can't perform symbolic execution
        if resolved_selectors.is_empty() {
            logger.error(&format!(
                "failed to resolve any function selectors from '{shortened_target}' .",
            ));
        }

        logger.info(&format!(
            "resolved {} possible functions from {} detected selectors.",
            resolved_selectors.len(),
            selectors.len()
        ));
    } else {
        logger.info(&format!("found {} possible function selectors.", selectors.len()));
    }

    logger.info(&format!("performing symbolic execution on '{shortened_target}' ."));

    Ok((selectors, resolved_selectors))
}

/// find all function selectors in the given EVM assembly.
pub fn find_function_selectors(evm: &VM, assembly: &str) -> HashMap<String, u128> {
    let mut function_selectors = HashMap::new();
    let mut handled_selectors = HashSet::new();

    // get a new logger
    let logger = Logger::default();

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
                    continue
                }

                logger.debug_max(&format!(
                    "optimistically assuming instruction {} {} {} is a function selector",
                    instruction_args[0], instruction_args[1], instruction_args[2]
                ));

                // add the function selector to the handled selectors
                handled_selectors.insert(function_selector.clone());

                // get the function's entry point
                let function_entry_point =
                    match resolve_entry_point(&evm.clone(), &function_selector) {
                        0 => continue,
                        x => x,
                    };

                logger.debug_max(&format!(
                    "found function selector {} at entry point {}",
                    function_selector, function_entry_point
                ));

                function_selectors.insert(function_selector, function_entry_point);
            }
        }
    }
    function_selectors
}

/// resolve a selector's function entry point from the EVM bytecode
pub fn resolve_entry_point(evm: &VM, selector: &str) -> u128 {
    let mut vm = evm.clone();
    let mut handled_jumps = HashSet::new();

    // execute the EVM call to find the entry point for the given selector
    vm.calldata = decode_hex(selector).expect("Failed to decode selector.");
    while vm.bytecode.len() >= vm.instruction as usize {
        let call = vm.step();

        // if the opcode is an JUMPI and it matched the selector, the next jumpi is the entry point
        if call.last_instruction.opcode == 0x57 {
            let jump_condition = call.last_instruction.input_operations[1].solidify();
            let jump_taken = call.last_instruction.inputs[1].try_into().unwrap_or(1);

            if jump_condition.contains(selector) &&
                jump_condition.contains("msg.data[0]") &&
                jump_condition.contains(" == ") &&
                jump_taken == 1
            {
                return call.last_instruction.inputs[0].try_into().unwrap_or(0)
            } else if jump_taken == 1 {
                // if handled_jumps contains the jumpi, we have already handled this jump.
                // loops aren't supported in the dispatcher, so we can just return 0
                if handled_jumps.contains(&call.last_instruction.inputs[0].try_into().unwrap_or(0))
                {
                    return 0
                } else {
                    handled_jumps.insert(call.last_instruction.inputs[0].try_into().unwrap_or(0));
                }
            }
        }

        if vm.exitcode != 255 || !vm.returndata.is_empty() {
            break
        }
    }

    0
}

/// Resolve a list of selectors to their function signatures.
pub async fn resolve_selectors<T>(selectors: Vec<String>) -> HashMap<String, Vec<T>>
where
    T: ResolveSelector + Send + Clone + 'static, {
    // get a new logger
    let logger = Logger::default();

    let resolved_functions: Arc<Mutex<HashMap<String, Vec<T>>>> =
        Arc::new(Mutex::new(HashMap::new()));

    let resolve_progress: Arc<Mutex<ProgressBar>> =
        Arc::new(Mutex::new(ProgressBar::new_spinner()));

    let mut threads = Vec::new();

    resolve_progress
        .lock()
        .expect("Could not obtain lock on resolve_progress.")
        .enable_steady_tick(Duration::from_millis(100));
    resolve_progress
        .lock()
        .expect("Could not obtain lock on resolve_progress.")
        .set_style(logger.info_spinner());
    resolve_progress
        .lock()
        .expect("Could not obtain lock on resolve_progress.")
        .set_message("resolving selectors");

    for selector in selectors {
        let function_clone = resolved_functions.clone();
        let resolve_progress = resolve_progress.clone();

        // create a new thread for each selector
        threads.push(task::spawn(async move {
            if let Some(function) = T::resolve(&selector).await {
                let mut _resolved_functions =
                    function_clone.lock().expect("Could not obtain lock on function_clone.");
                let mut _resolve_progress =
                    resolve_progress.lock().expect("Could not obtain lock on resolve_progress.");
                _resolve_progress
                    .set_message(format!("resolved {} selectors", _resolved_functions.len()));
                _resolved_functions.insert(selector, function);
            }
        }));
    }

    // wait for all threads to finish
    for thread in threads {
        if let Err(e) = thread.await {
            // Handle error
            eprintln!("Task failed: {:?}", e);
        }
    }

    resolve_progress.lock().unwrap().finish_and_clear();

    let x =
        resolved_functions.lock().expect("Could not obtain lock on resolved_functions.").clone();
    x
}
