use std::collections::HashMap;

use heimdall_common::{
    ether::{evm::vm::VM, signatures::{ResolvedFunction, resolve_signature}}
};


// Find all function selectors in the given EVM.
pub fn find_function_selectors(evm: &VM, assembly: String) -> Vec<String> {
    
    let mut function_selectors = Vec::new();

    // execute an EVM call with empty calldata to find the dispatcher revert
    let mut vm = evm.clone();
    vm.execute();
    let dispatcher_revert = vm.instruction - 1;

    // search through assembly for PUSH4 instructions up until the dispatcher revert
    let assembly: Vec<String> = assembly.split("\n").map(|line| line.trim().to_string()).collect();
    for line in assembly.iter() {
        let instruction_args: Vec<String> = line.split(" ").map(|arg| arg.to_string()).collect();
        let program_counter: u128 = instruction_args[0].clone().parse().unwrap();
        let instruction = instruction_args[1].clone();

        if program_counter < dispatcher_revert {
            if instruction == "PUSH4" {
                let function_selector = instruction_args[2].clone();
                function_selectors.push(function_selector);
            }
        }
        else {
            break;
        }
    }

    function_selectors
}

pub fn resolve_function_selectors(selectors: Vec<String>) -> HashMap<String, Vec<ResolvedFunction>> {
    
    let mut resolved_functions: HashMap<String, Vec<ResolvedFunction>> = HashMap::new();

    for selector in selectors {
        match resolve_signature(&selector) {
            Some(function) => {
                resolved_functions.insert(selector, function);
            },
            None => continue
        }
    }

    resolved_functions
}