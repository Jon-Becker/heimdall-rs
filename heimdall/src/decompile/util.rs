use std::{collections::HashMap, str::FromStr};

use ethers::{prelude::{rand::{self, Rng}, U256}, abi::AbiEncode};
use heimdall_common::{
    ether::{evm::vm::VM, signatures::{ResolvedFunction, resolve_signature}}
};


// Find all function selectors in the given EVM.
pub fn find_function_selectors(evm: &VM, assembly: String) -> Vec<String> {
    let mut function_selectors = Vec::new();

    let mut vm = evm.clone();

    // find a selector not present in the assembly
    let selector;
    loop {
        let num = rand::thread_rng().gen_range(286331153..2147483647);
        if !vm.bytecode.contains(&format!("63{}", num.encode_hex()[58..].to_string())) {
            selector = num.encode_hex()[58..].to_string();
            break;
        }
    }

    // execute the EVM call to find the dispatcher revert
    let dispatcher_revert = vm.call(selector, 0).instruction - 1;

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
    function_selectors.sort();
    function_selectors.dedup();
    function_selectors
}


// resolve a list of function selectors to their possible signatures
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


// resolve a selector's function entry point from the EVM bytecode
pub fn resolve_entry_point(evm: &VM, selector: String) -> u64 {
    let mut vm = evm.clone();
    let mut flag_next_jumpi = false;
    let mut function_entry_point = 0;
    
    // execute the EVM call to find the entry point for the given selector
    vm.calldata = selector.clone();
    while vm.bytecode.len() >= (vm.instruction*2+2) as usize {
        let call = vm.step();

        // if the opcode is an EQ and it matched the selector, the next jumpi is the entry point
        if call.last_instruction.opcode == "14" && 
           call.last_instruction.inputs[0].eq(&U256::from_str(&selector.clone()).unwrap()) {
            
            flag_next_jumpi = true;
        }

        // if we are flagging the next jumpi, and the opcode is a JUMPI, we have found the entry point
        if flag_next_jumpi && call.last_instruction.opcode == "57" {

            // it's safe to convert here because we know max bytecode length is ~25kb, way less than 2^64
            function_entry_point = call.last_instruction.inputs[0].as_u64();
            break;
        }

        if vm.exitcode != 255 || vm.returndata.len() as usize > 0 {
            break
        }
    }

    function_entry_point
}
