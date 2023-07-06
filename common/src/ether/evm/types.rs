use colored::Colorize;
use ethers::abi::{AbiEncode, ParamType, Token};

use crate::{
    constants::TYPE_CAST_REGEX,
    utils::strings::{find_balanced_encapsulator, replace_last},
};

use super::vm::Instruction;

// decode a string into an ethereum type
pub fn parse_function_parameters(function_signature: String) -> Option<Vec<ParamType>> {
    let mut function_inputs = Vec::new();

    // get only the function input body, removing the name and input wrapping parentheses
    let string_inputs = match function_signature.split_once('(') {
        Some((_, inputs)) => replace_last(inputs.to_string(), ")", ""),
        None => replace_last(function_signature, ")", ""),
    };

    // split into individual inputs
    let temp_inputs: Vec<String> = string_inputs.split(',').map(|s| s.to_string()).collect();
    let mut inputs: Vec<String> = Vec::new();

    // if the input contains complex types, rejoin them. for nested types, this function will
    // recurse.
    if string_inputs.contains('(') {
        let mut tuple_depth = 0;
        let mut complex_input: Vec<String> = Vec::new();

        for input in temp_inputs {
            if input.contains('(') {
                tuple_depth += 1;
            }

            if tuple_depth > 0 {
                complex_input.push(input.to_string());
            } else {
                inputs.push(input.to_string());
            }

            if input.contains(')') {
                tuple_depth -= 1;

                if tuple_depth == 0 {
                    inputs.push(complex_input.join(","));
                    complex_input = Vec::new();
                }
            }
        }
    } else {
        inputs = temp_inputs;
    }

    // parse each input into an ethereum type, recusing if necessary
    for solidity_type in inputs {
        if solidity_type == "address" {
            function_inputs.push(ParamType::Address);
            continue
        }
        if solidity_type == "bytes" {
            function_inputs.push(ParamType::Bytes);
            continue
        }
        if solidity_type == "bool" {
            function_inputs.push(ParamType::Bool);
            continue
        }
        if solidity_type == "string" {
            function_inputs.push(ParamType::String);
            continue
        }
        if solidity_type.starts_with('(') && !solidity_type.ends_with(']') {
            let complex_inputs = match parse_function_parameters(solidity_type.clone()) {
                Some(inputs) => inputs,
                None => continue,
            };
            function_inputs.push(ParamType::Tuple(complex_inputs));
            continue
        }
        if solidity_type.ends_with("[]") {
            let array_type = match parse_function_parameters(
                solidity_type[..solidity_type.len() - 2].to_string(),
            ) {
                Some(types_) => types_,
                None => continue,
            };

            if array_type.len() == 1 {
                function_inputs.push(ParamType::Array(Box::new(array_type[0].clone())));
            } else {
                function_inputs.push(ParamType::Array(Box::new(ParamType::Tuple(array_type))));
            }
            continue
        }
        if solidity_type.ends_with(']') {
            let size = match solidity_type.split('[').nth(1) {
                Some(size) => match size.replace(']', "").parse::<usize>() {
                    Ok(size) => size,
                    Err(_) => continue,
                },
                None => continue,
            };
            let array_type = match parse_function_parameters(
                solidity_type[..solidity_type.len() - (2 + size.to_string().len())].to_string(),
            ) {
                Some(types_) => types_,
                None => continue,
            };

            if array_type.len() == 1 {
                function_inputs.push(ParamType::FixedArray(Box::new(array_type[0].clone()), size));
            } else {
                function_inputs
                    .push(ParamType::FixedArray(Box::new(ParamType::Tuple(array_type)), size));
            }
            continue
        }
        if solidity_type.starts_with("int") {
            let size = solidity_type.replace("int", "").parse::<usize>().unwrap_or(256);
            function_inputs.push(ParamType::Int(size));
            continue
        }
        if solidity_type.starts_with("uint") {
            let size = solidity_type.replace("uint", "").parse::<usize>().unwrap_or(256);

            function_inputs.push(ParamType::Uint(size));
            continue
        }
        if solidity_type.starts_with("bytes") {
            let size = solidity_type.replace("bytes", "").parse::<usize>().unwrap_or(32);

            function_inputs.push(ParamType::FixedBytes(size));
            continue
        }
    }

    match function_inputs.len() {
        0 => None,
        _ => Some(function_inputs),
    }
}

// returns a vec of beautified types for a given vec of tokens
pub fn display(inputs: Vec<Token>, prefix: &str) -> Vec<String> {
    let mut output = Vec::new();
    let prefix = prefix.to_string();

    for input in inputs {
        match input {
            Token::Address(_) => output.push(format!("{prefix}{} 0x{input}", "address".blue())),
            Token::Int(val) => output.push(format!("{prefix}{} {}", "int    ".blue(), val)),
            Token::Uint(val) => output.push(format!("{prefix}{} {}", "uint   ".blue(), val)),
            Token::String(val) => output.push(format!("{prefix}{} {val}", "string ".blue())),
            Token::Bool(val) => {
                if val {
                    output.push(format!("{prefix}{} true", "bool   ".blue()));
                } else {
                    output.push(format!("{prefix}{} false", "bool   ".blue()));
                }
            }
            Token::FixedBytes(_) | Token::Bytes(_) => {
                let bytes = input
                    .to_string()
                    .chars()
                    .collect::<Vec<char>>()
                    .chunks(64)
                    .map(|c| c.iter().collect::<String>())
                    .collect::<Vec<String>>();

                for (i, byte) in bytes.iter().enumerate() {
                    if i == 0 {
                        output.push(format!("{prefix}{} 0x{}", "bytes  ".blue(), byte));
                    } else {
                        output.push(format!("{prefix}{}   {}", "       ".blue(), byte));
                    }
                }
            }
            Token::FixedArray(val) | Token::Array(val) => {
                if val.is_empty() {
                    output.push(format!("{prefix}[]"));
                } else {
                    output.push(format!("{prefix}["));
                    output.extend(display(val.to_vec(), &format!("{prefix}   ")));
                    output.push(format!("{prefix}]"));
                }
            }
            Token::Tuple(val) => {
                if val.is_empty() {
                    output.push(format!("{prefix}()"));
                } else {
                    output.push(format!("{prefix}("));
                    output.extend(display(val.to_vec(), &format!("{prefix}   ")));
                    output.push(format!("{prefix})"));
                }
            }
        };
    }

    output
}

// converts a bit mask into it's potential types
pub fn convert_bitmask(instruction: Instruction) -> (usize, Vec<String>) {
    let mask = instruction.output_operations[0].clone();

    // use 32 as the default size, as it is the default word size in the EVM
    let mut type_byte_size = 32;

    // determine which input contains the bitmask
    for (i, input) in mask.inputs.iter().enumerate() {
        match input {
            crate::ether::evm::opcodes::WrappedInput::Raw(_) => continue,
            crate::ether::evm::opcodes::WrappedInput::Opcode(opcode) => {
                if !(opcode.opcode.name == "CALLDATALOAD" || opcode.opcode.name == "CALLDATACOPY") {
                    if mask.opcode.name == "AND" {
                        type_byte_size = instruction.inputs[i].encode_hex().matches("ff").count();
                    } else if mask.opcode.name == "OR" {
                        type_byte_size = instruction.inputs[i].encode_hex().matches("00").count();
                    }
                }
            }
        };
    }

    // determine the solidity type based on the resulting size of the masked data
    byte_size_to_type(type_byte_size)
}

pub fn byte_size_to_type(byte_size: usize) -> (usize, Vec<String>) {
    let mut potential_types = Vec::new();

    match byte_size {
        1 => potential_types.push("bool".to_string()),
        20 => potential_types.push("address".to_string()),
        _ => {}
    }

    // push arbitrary types to the array
    potential_types.push(format!("uint{}", byte_size * 8));
    potential_types.push(format!("bytes{byte_size}"));
    potential_types.push(format!("int{}", byte_size * 8));

    // return list of potential type castings, sorted by likelihood descending
    (byte_size, potential_types)
}

pub fn find_cast(line: String) -> (usize, usize, Option<String>) {
    // find the start of the cast
    match TYPE_CAST_REGEX.find(&line).unwrap() {
        Some(m) => {
            let start = m.start();
            let end = m.end() - 1;
            let cast_type = line[start..].split('(').collect::<Vec<&str>>()[0].to_string();

            // find where the cast ends
            let (a, b, _) = find_balanced_encapsulator(&line[end..], ('(', ')'));
            (end + a, end + b, Some(cast_type))
        }
        None => (0, 0, None),
    }
}
