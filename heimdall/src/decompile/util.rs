use std::{collections::HashMap, str::FromStr};

use ethers::{
    prelude::{
        U256,
    },
};
use heimdall_common::{
    ether::{
        evm::{
            log::Log,
            opcodes::WrappedOpcode,
            vm::{State, VM}
        }, signatures::{ResolvedFunction, ResolvedError, ResolvedLog},
    },
    io::logging::{TraceFactory},
};

use crate::decompile::constants::LOOP_DETECTION_REGEX;

#[derive(Clone, Debug)]
pub struct Function {
    // the function's 4byte selector
    pub selector: String,

    // the function's entry point in the code.
    // the entry point is the instruction the dispatcher JUMPs to when called.
    pub entry_point: u64,

    // argument structure:
    //   - key : slot operations of the argument.
    //   - value : tuple of ({slot: U256, mask: usize}, potential_types)
    pub arguments: HashMap<usize, (CalldataFrame, Vec<String>)>,

    // storage structure:
    //   - key : slot of the argument. I.E: slot 0 is CALLDATALOAD(4).
    //   - value : tuple of ({value: U256, operation: WrappedOpcode})
    pub storage: HashMap<U256, StorageFrame>,

    // memory structure:
    //   - key : slot of the argument. I.E: slot 0 is CALLDATALOAD(4).
    //   - value : tuple of ({value: U256, operation: WrappedOpcode})
    pub memory: HashMap<U256, StorageFrame>,

    // returns the return type for the function.
    pub returns: Option<String>,

    // holds function logic to be written to the output solidity file.
    pub logic: Vec<String>,

    // holds all emitted events. used to generate solidity event definitions
    // as well as ABI specifications.
    pub events: HashMap<String, (Option<ResolvedLog>, Log)>,

    // holds all found custom errors used to generate solidity error definitions
    // as well as ABI specifications.
    pub errors: HashMap<String, Option<ResolvedError>>,

    // stores the matched resolved function for this Functon
    pub resolved_function: Option<ResolvedFunction>,

    // modifiers
    pub pure: bool,
    pub view: bool,
    pub payable: bool,
}

#[derive(Clone, Debug)]
pub struct StorageFrame {
    pub value: U256,
    pub operations: WrappedOpcode,
}

#[derive(Clone, Debug)]
pub struct CalldataFrame {
    pub slot: usize,
    pub operation: String,
    pub mask_size: usize,
    pub heuristics: Vec<String>,
}

impl Function {
    // get a specific memory slot

    pub fn get_memory_range(&self, _offset: U256, _size: U256) -> Vec<StorageFrame> {
        let mut memory_slice: Vec<StorageFrame> = Vec::new();

        // Safely convert U256 to usize
        let mut offset: usize = match _offset.try_into() {
            Ok(x) => x,
            Err(_) => 0,
        };
        let mut size: usize = match _size.try_into() {
            Ok(x) => x,
            Err(_) => 0,
        };

        // get the memory range
        while size > 0 {
            match self.memory.get(&U256::from(offset)) {
                Some(memory) => {
                    memory_slice.push(memory.clone());
                }
                None => {}
            }
            offset += 32;
            size = size.saturating_sub(32);
        }

        memory_slice
    }
}

#[derive(Clone, Debug)]
pub struct VMTrace {
    pub instruction: u128,
    pub operations: Vec<State>,
    pub children: Vec<VMTrace>,
    pub loop_detected: bool,
    pub depth: usize,
}

// returns the compiler version used to compile the contract.
// for example: (solc, 0.8.10) or (vyper, 0.2.16)
pub fn detect_compiler(bytecode: String) -> (String, String) {
    
    let mut compiler = "unknown".to_string();
    let mut version = "unknown".to_string();

    // perfom prefix check for rough version matching
    if bytecode.starts_with("363d3d373d3d3d363d73") {
        compiler = "proxy".to_string();
        version = "minimal".to_string();
    }
    else if bytecode.starts_with("366000600037611000600036600073") {
        compiler = "proxy".to_string();
        version = "vyper".to_string();
    }
    else if bytecode.starts_with("6004361015") {
        compiler = "vyper".to_string();
        version = "0.2.0-0.2.4,0.2.11-0.3.3".to_string();
    }
    else if bytecode.starts_with("341561000a") {
        compiler = "vyper".to_string();
        version = "0.2.5-0.2.8".to_string();
    }
    else if bytecode.starts_with("731bf797") {
        compiler = "solc".to_string();
        version = "0.4.10-0.4.24".to_string();
    }
    else if bytecode.starts_with("6080604052") {
        compiler = "solc".to_string();
        version = "0.4.22+".to_string();
    }
    else if bytecode.starts_with("6060604052") {
        compiler = "solc".to_string();
        version = "0.4.11-0.4.21".to_string();
    }
    else if bytecode.contains("7679706572") {
        compiler = "vyper".to_string();
    }
    else if bytecode.contains("736f6c63") {
        compiler = "solc".to_string();
    }

    // perform metadata check
    if compiler == "solc" {
        let compiler_version = bytecode.split("736f6c6343").collect::<Vec<&str>>();
        
        if compiler_version.len() > 1 {
            match compiler_version[1].get(0..6) {
                Some(encoded_version) => {
                    let version_array = encoded_version.chars()
                        .collect::<Vec<char>>()
                        .chunks(2)
                        .map(|c| c.iter().collect::<String>())
                        .collect::<Vec<String>>();

                    version = String::new();
                    for version_part in version_array {
                        version.push_str(&format!("{}.", u8::from_str_radix(&version_part, 16).unwrap()));
                    }
                },
                None => {},
            }
        }
    }
    else if compiler == "vyper" {
        let compiler_version = bytecode.split("767970657283").collect::<Vec<&str>>();
        
        if compiler_version.len() > 1 {
            match compiler_version[1].get(0..6) {
                Some(encoded_version) => {
                    let version_array = encoded_version.chars()
                        .collect::<Vec<char>>()
                        .chunks(2)
                        .map(|c| c.iter().collect::<String>())
                        .collect::<Vec<String>>();

                    version = String::new();
                    for version_part in version_array {
                        version.push_str(&format!("{}.", u8::from_str_radix(&version_part, 16).unwrap()));
                    }
                },
                None => {},
            }
        }
    }


    (compiler, version.trim_end_matches(".").to_string())
}

// find all function selectors in the given EVM.
pub fn find_function_selectors(assembly: String) -> Vec<String> {
    let mut function_selectors = Vec::new();

    // search through assembly for PUSH4 instructions, optimistically assuming that they are function selectors
    let assembly: Vec<String> = assembly
        .split("\n")
        .map(|line| line.trim().to_string())
        .collect();
    for line in assembly.iter() {
        let instruction_args: Vec<String> = line.split(" ").map(|arg| arg.to_string()).collect();

        if instruction_args.len() >= 2 {
            let instruction = instruction_args[1].clone();

            if instruction == "PUSH4" {
                let function_selector = instruction_args[2].clone();
                function_selectors.push(function_selector);
            }
        }
    }
    function_selectors.sort();
    function_selectors.dedup();
    function_selectors
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
) -> (VMTrace, Vec<String>) {
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
        recursive_map(
            &vm.clone(),
            trace,
            trace_parent,
            &mut handled_jumpdests,
            &mut String::new()
        ),
        handled_jumpdests,
    )
}

pub fn recursive_map(
    evm: &VM,
    trace: &TraceFactory,
    trace_parent: u32,
    handled_jumpdests: &mut Vec<String>,
    path: &mut String,
) -> VMTrace {
    let mut vm = evm.clone();

    // create a new VMTrace object
    let mut vm_trace = VMTrace {
        instruction: vm.instruction,
        operations: Vec::new(),
        children: Vec::new(),
        loop_detected: false,
        depth: 0,
    };

    // step through the bytecode until we find a JUMPI instruction
    while vm.bytecode.len() >= (vm.instruction * 2 + 2) as usize {
        let state = vm.step();
        vm_trace.operations.push(state.clone());

        // if we encounter a JUMPI, create children taking both paths and break
        if state.last_instruction.opcode == "57" {
            vm_trace.depth += 1;

            path.push_str(&format!("{}->{};", state.last_instruction.instruction, state.last_instruction.inputs[0]));

            // we need to create a trace for the path that wasn't taken.
            if state.last_instruction.inputs[1] == U256::from(0) {

                // break out of loops
                if LOOP_DETECTION_REGEX.is_match(&path).unwrap() {
                    vm_trace.loop_detected = true;
                    break;
                }

                handled_jumpdests.push(format!("{}@{}", vm_trace.depth, state.last_instruction.instruction));

                // push a new vm trace to the children
                let mut trace_vm = vm.clone();
                trace_vm.instruction = state.last_instruction.inputs[0].as_u128() + 1;
                vm_trace.children.push(recursive_map(
                    &trace_vm,
                    trace,
                    trace_parent,
                    handled_jumpdests,
                    &mut path.clone()
                ));

                // push the current path onto the stack
                vm_trace.children.push(recursive_map(
                    &vm.clone(),
                    trace,
                    trace_parent,
                    handled_jumpdests,
                    &mut path.clone()
                ));
                break;
            } else {

                // break out of loops
                if LOOP_DETECTION_REGEX.is_match(&path).unwrap() {
                    vm_trace.loop_detected = true;
                    break;
                }

                handled_jumpdests.push(format!("{}@{}", vm_trace.depth, state.last_instruction.instruction));

                // push a new vm trace to the children
                let mut trace_vm = vm.clone();
                trace_vm.instruction = state.last_instruction.instruction + 1;
                vm_trace.children.push(recursive_map(
                    &trace_vm,
                    trace,
                    trace_parent,
                    handled_jumpdests,
                    &mut path.clone()
                ));

                // push the current path onto the stack
                vm_trace.children.push(recursive_map(
                    &vm.clone(),
                    trace,
                    trace_parent,
                    handled_jumpdests,
                    &mut path.clone()
                ));
                break;
            }
        }

        if vm.exitcode != 255 || vm.returndata.len() > 0 {
            break;
        }
    }

    vm_trace
}