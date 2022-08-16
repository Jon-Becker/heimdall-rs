use std::{collections::HashMap, str::FromStr};

use ethers::{
    abi::AbiEncode,
    prelude::{
        rand::{self, Rng},
        U256,
    },
};
use heimdall_common::{
    ether::{
        evm::{vm::{VM, State}},
        signatures::{resolve_signature, ResolvedFunction},
    },
    io::logging::TraceFactory,
};

use super::Function;

#[derive(Clone, Debug)]
pub struct VMTrace {
    pub instruction: u128,
    pub operations: Vec<State>,
    pub children: Vec<VMTrace>,
    pub depth: usize,
}

// Find all function selectors in the given EVM.
pub fn find_function_selectors(evm: &VM, assembly: String) -> Vec<String> {
    let mut function_selectors = Vec::new();

    let mut vm = evm.clone();

    // find a selector not present in the assembly
    let selector;
    loop {
        let num = rand::thread_rng().gen_range(286331153..2147483647);
        if !vm
            .bytecode
            .contains(&format!("63{}", num.encode_hex()[58..].to_string()))
        {
            selector = num.encode_hex()[58..].to_string();
            break;
        }
    }

    // execute the EVM call to find the dispatcher revert
    let dispatcher_revert = vm.call(selector, 0).instruction - 1;

    // search through assembly for PUSH4 instructions up until the dispatcher revert
    let assembly: Vec<String> = assembly
        .split("\n")
        .map(|line| line.trim().to_string())
        .collect();
    for line in assembly.iter() {
        let instruction_args: Vec<String> = line.split(" ").map(|arg| arg.to_string()).collect();
        let program_counter: u128 = instruction_args[0].clone().parse().unwrap();
        let instruction = instruction_args[1].clone();

        if program_counter < dispatcher_revert {
            if instruction == "PUSH4" {
                let function_selector = instruction_args[2].clone();
                function_selectors.push(function_selector);
            }
        } else {
            break;
        }
    }
    function_selectors.sort();
    function_selectors.dedup();
    function_selectors
}

// resolve a list of function selectors to their possible signatures
pub fn resolve_function_selectors(
    selectors: Vec<String>,
) -> HashMap<String, Vec<ResolvedFunction>> {
    let mut resolved_functions: HashMap<String, Vec<ResolvedFunction>> = HashMap::new();

    for selector in selectors {
        match resolve_signature(&selector) {
            Some(function) => {
                resolved_functions.insert(selector, function);
            }
            None => continue,
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
    while vm.bytecode.len() >= (vm.instruction * 2 + 2) as usize {
        let call = vm.step();

        // if the opcode is an EQ and it matched the selector, the next jumpi is the entry point
        if call.last_instruction.opcode == "14"
            && call.last_instruction.inputs[0].eq(&U256::from_str(&selector.clone()).unwrap())
            && call.last_instruction.outputs[0].eq(&U256::from_str("1").unwrap())
        {
            flag_next_jumpi = true;
        }

        // if we are flagging the next jumpi, and the opcode is a JUMPI, we have found the entry point
        if flag_next_jumpi && call.last_instruction.opcode == "57" {
            // it's safe to convert here because we know max bytecode length is ~25kb, way less than 2^64
            function_entry_point = call.last_instruction.inputs[0].as_u64();
            break;
        }

        if vm.exitcode != 255 || vm.returndata.len() as usize > 0 {
            break;
        }
    }

    function_entry_point
}

// build a map of function jump possibilities from the EVM bytecode
pub fn map_selector(
    evm: &VM,
    trace: &TraceFactory,
    trace_parent: u32,
    selector: String,
    entry_point: u64,
) -> (VMTrace, Vec<u128>) {
    let mut vm = evm.clone();
    vm.calldata = selector.clone();

    // step through the bytecode until we reach the entry point
    while (vm.bytecode.len() >= (vm.instruction * 2 + 2) as usize)
        && (vm.instruction <= entry_point.into())
    {
        vm.step();

        // this shouldn't be necessary, but it's safer to have it
        if vm.exitcode != 255 || vm.returndata.len() as usize > 0 {
            break;
        }
    }

    // the VM is at the function entry point, begin tracing
    let mut handled_jumpdests = Vec::new();
    (
        recursive_map(&vm.clone(), trace, trace_parent, &mut handled_jumpdests),
        handled_jumpdests,
    )
}

pub fn recursive_map(
    evm: &VM,
    trace: &TraceFactory,
    trace_parent: u32,
    handled_jumpdests: &mut Vec<u128>,
) -> VMTrace {
    let mut vm = evm.clone();

    // create a new VMTrace object
    let mut vm_trace = VMTrace {
        instruction: vm.instruction,
        operations: Vec::new(),
        children: Vec::new(),
        depth: 0,
    };

    // step through the bytecode until we find a JUMPI instruction
    while vm.bytecode.len() >= (vm.instruction * 2 + 2) as usize {
        let state = vm.step();
        vm_trace.operations.push(state.clone());

        // if we encounter a JUMPI, create children taking both paths and break
        if state.last_instruction.opcode == "57" {
            vm_trace.depth += 1;

            // we need to create a trace for the path that wasn't taken.
            if state.last_instruction.inputs[1] == U256::from(0) {
                // the jump was not taken, create a trace for the jump path
                // only jump if we haven't already traced this destination
                // TODO: mark as a loop?
                if !(handled_jumpdests.contains(&(state.last_instruction.inputs[0].as_u128() + 1)))
                {
                    let mut trace_vm = vm.clone();
                    trace_vm.instruction = state.last_instruction.inputs[0].as_u128() + 1;
                    handled_jumpdests.push(trace_vm.instruction.clone());
                    vm_trace.children.push(recursive_map(
                        &trace_vm,
                        trace,
                        trace_parent,
                        handled_jumpdests,
                    ));
                } else {
                    break;
                }

                // push the current path onto the stack
                vm_trace.children.push(recursive_map(
                    &vm.clone(),
                    trace,
                    trace_parent,
                    handled_jumpdests,
                ));
            } else {
                // the jump was taken, create a trace for the fallthrough path
                // only jump if we haven't already traced this destination
                if !(handled_jumpdests.contains(&(state.last_instruction.instruction + 1))) {
                    let mut trace_vm = vm.clone();
                    trace_vm.instruction = state.last_instruction.instruction + 1;
                    handled_jumpdests.push(trace_vm.instruction.clone());
                    vm_trace.children.push(recursive_map(
                        &trace_vm,
                        trace,
                        trace_parent,
                        handled_jumpdests,
                    ));
                } else {
                    break;
                }

                // push the current path onto the stack
                vm_trace.children.push(recursive_map(
                    &vm.clone(),
                    trace,
                    trace_parent,
                    handled_jumpdests,
                ));
            }
        }

        if vm.exitcode != 255 || vm.returndata.len() as usize > 0 {
            break;
        }
    }

    vm_trace
}

impl VMTrace {
    // converts a VMTrace to a Funciton
    pub fn analyze(
        &self,
        function: Function,
        trace: &mut TraceFactory,
        trace_parent: u32,
    ) -> Function {
        // make a clone of the recursed analysis function
        let mut function = function.clone();

        // perform analysis on the operations of the current VMTrace branch
        for operation in &self.operations { 
            let opcode_name = operation.last_instruction.opcode_details.clone().unwrap().name;

            // if the instruction is a state-accessing instruction, the function is no longer pure
            if function.pure
                && vec![
                    "BALANCE",
                    "ORIGIN",
                    "CALLER",
                    "GASPRICE",
                    "EXTCODESIZE",
                    "EXTCODECOPY",
                    "BLOCKHASH",
                    "COINBASE",
                    "TIMESTAMP",
                    "NUMBER",
                    "DIFFICULTY",
                    "GASLIMIT",
                    "CHAINID",
                    "SELFBALANCE",
                    "BASEFEE",
                    "SLOAD",
                    "SSTORE",
                    "CREATE",
                    "SELFDESTRUCT",
                    "CALL",
                    "CALLCODE",
                    "DELEGATECALL",
                    "STATICCALL",
                    "CREATE2",
                ]
                .contains(&opcode_name.as_str())
            {
                function.pure = false;
                trace.add_info(
                    trace_parent,
                    operation.last_instruction.instruction.try_into().unwrap(),
                    format!(
                        "instruction {} ({}) indicates an non-pure function.",
                        operation.last_instruction.instruction, opcode_name
                    ),
                );
            }

            // if the instruction is a state-setting instruction, the function is no longer a view
            if function.view
                && vec![
                    "SSTORE",
                    "CREATE",
                    "SELFDESTRUCT",
                    "CALL",
                    "CALLCODE",
                    "DELEGATECALL",
                    "STATICCALL",
                    "CREATE2",
                ]
                .contains(&opcode_name.as_str())
            {
                function.view = false;
                trace.add_info(
                    trace_parent,
                    operation.last_instruction.instruction.try_into().unwrap(),
                    format!(
                        "instruction {} ({}) indicates a non-view function.",
                        operation.last_instruction.instruction, opcode_name
                    ),
                );
            }

            // add the sstore to the function's storage map
            if opcode_name == "SSTORE" {
                let key = operation.last_instruction.inputs[0];
                let value = operation.last_instruction.inputs[1];
                function.storage.insert(key, value);
            }
        }

        // recurse into the children of the VMTrace map
        for child in &self.children {
            function = child.analyze(function, trace, trace_parent);
        }

        function
    }
}
