use colored::Colorize;
use ethers::abi::{AbiEncode, ParamType, Token};

use crate::{constants::TYPE_CAST_REGEX, utils::strings::find_balanced_encapsulator};

use super::vm::Instruction;

// decode a string into an ethereum type
pub fn parse_function_parameters(function_signature: &str) -> Option<Vec<ParamType>> {
    // remove the function name from the signature, only keep the parameters
    let (start, end, valid) = find_balanced_encapsulator(function_signature, ('(', ')'));
    if !valid {
        return None
    }

    let function_inputs = function_signature[start + 1..end - 1].to_string();

    // get inputs from the string
    extract_types_from_string(&function_inputs)
}

// helper function for extracting types from a string
fn extract_types_from_string(string: &str) -> Option<Vec<ParamType>> {
    let mut types = Vec::new();

    // if string is empty, return None
    if string.is_empty() {
        return None
    }

    // if the string contains a tuple we cant simply split on commas
    if ['(', ')'].iter().any(|c| string.contains(*c)) {
        // check if first type is a tuple
        if is_first_type_tuple(string) {
            // get balanced encapsulator
            let (tuple_start, tuple_end, valid) = find_balanced_encapsulator(string, ('(', ')'));
            if !valid {
                return None
            }

            // extract the tuple
            let tuple_types = string[tuple_start + 1..tuple_end - 1].to_string();

            // remove the tuple from the string
            let mut string = string[tuple_end..].to_string();

            // if string is not empty, split on commas and check if tuple is an array
            let mut is_array = false;
            let mut array_size: Option<usize> = None;
            if !string.is_empty() {
                let split = string.splitn(2, ',').collect::<Vec<&str>>()[0];

                is_array = split.ends_with(']');

                // get array size, or none if []
                if is_array {
                    let (start, end, valid) = find_balanced_encapsulator(split, ('[', ']'));
                    if !valid {
                        return None
                    }

                    let size = split[start + 1..end - 1].to_string();
                    array_size = match size.parse::<usize>() {
                        Ok(size) => Some(size),
                        Err(_) => None,
                    };
                }
            }

            if is_array {
                // if the string doesnt contain a comma, this is the last type
                if string.contains(',') {
                    // remove the array from the string by splitting on the first comma and taking
                    // the second half
                    string = string.splitn(2, ',').collect::<Vec<&str>>()[1].to_string();
                } else {
                    // set string to empty string
                    string = "".to_string();
                }

                if array_size.is_some() {
                    // recursively call this function to extract the tuple types
                    let inner_types = extract_types_from_string(&tuple_types);

                    types.push(ParamType::FixedArray(
                        Box::new(ParamType::Tuple(inner_types.unwrap())),
                        array_size.unwrap(),
                    ))
                } else {
                    // recursively call this function to extract the tuple types
                    let inner_types = extract_types_from_string(&tuple_types);

                    types.push(ParamType::Array(Box::new(ParamType::Tuple(inner_types.unwrap()))))
                }
            } else {
                // recursively call this function to extract the tuple types
                let inner_types = extract_types_from_string(&tuple_types);

                types.push(ParamType::Tuple(inner_types.unwrap()));
            }

            // recursively call this function to extract the remaining types
            match extract_types_from_string(&string) {
                Some(mut remaining_types) => {
                    types.append(&mut remaining_types);
                }
                None => {}
            }
        } else {
            // first type is not a tuple, so we can extract it
            let string_parts = string.splitn(2, ',').collect::<Vec<&str>>();

            // convert the string type to a ParamType
            if string_parts[0].is_empty() {
                // the first type is empty, so we can just recursively call this function to extract
                // the remaining types
                match extract_types_from_string(string_parts[1]) {
                    Some(mut remaining_types) => {
                        types.append(&mut remaining_types);
                    }
                    None => {}
                }
            } else {
                let param_type = to_type(string_parts[0]);
                types.push(param_type);

                // remove the first type from the string
                let string = string[string_parts[0].len() + 1..].to_string();

                // recursively call this function to extract the remaining types
                match extract_types_from_string(&string) {
                    Some(mut remaining_types) => {
                        types.append(&mut remaining_types);
                    }
                    None => {}
                }
            }
        }
    } else {
        // split on commas
        let split = string.split(',').collect::<Vec<&str>>();

        // iterate over the split string and convert each type to a ParamType
        for string_type in split {
            if string_type.is_empty() {
                continue
            }

            let param_type = to_type(string_type);
            types.push(param_type);
        }
    }

    match types.len() {
        0 => None,
        _ => Some(types),
    }
}

fn is_first_type_tuple(string: &str) -> bool {
    // split by first comma
    let split = string.splitn(2, ',').collect::<Vec<&str>>();

    // if the first element starts with a (, it is a tuple
    split[0].starts_with('(')
}

fn to_type(string: &str) -> ParamType {
    let is_array = string.ends_with(']');

    // get size of array
    let array_size = if is_array {
        let (start, end, valid) = find_balanced_encapsulator(string, ('[', ']'));
        if !valid {
            return ParamType::Bytes
        }

        let size = string[start + 1..end - 1].to_string();
        match size.parse::<usize>() {
            Ok(size) => Some(size),
            Err(_) => None,
        }
    } else {
        None
    };

    // if array, remove the [..] from the string
    let string = if is_array { string.splitn(2, '[').collect::<Vec<&str>>()[0] } else { string };

    let arg_type = match string {
        "address" => ParamType::Address,
        "bool" => ParamType::Bool,
        "string" => ParamType::String,
        "bytes" => ParamType::Bytes,
        _ => {
            if string.starts_with("uint") {
                let size = string[4..].parse::<usize>().unwrap_or(256);
                ParamType::Uint(size)
            } else if string.starts_with("int") {
                let size = string[3..].parse::<usize>().unwrap_or(256);
                ParamType::Int(size)
            } else if string.starts_with("bytes") {
                let size = string[5..].parse::<usize>().unwrap();
                ParamType::FixedBytes(size)
            } else {
                panic!("Invalid type: '{}'", string);
            }
        }
    };

    if is_array {
        if let Some(size) = array_size {
            ParamType::FixedArray(Box::new(arg_type), size)
        } else {
            ParamType::Array(Box::new(arg_type))
        }
    } else {
        arg_type
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
            crate::ether::evm::core::opcodes::WrappedInput::Raw(_) => continue,
            crate::ether::evm::core::opcodes::WrappedInput::Opcode(opcode) => {
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

pub fn find_cast(line: &str) -> (usize, usize, Option<String>) {
    // find the start of the cast
    match TYPE_CAST_REGEX.find(line).expect("Failed to find type cast.") {
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
