use colored::Colorize;
use ethers::abi::{AbiEncode, ParamType, Token};

use crate::{constants::TYPE_CAST_REGEX, utils::strings::find_balanced_encapsulator};

use super::vm::Instruction;

/// Parse function parameters [`ParamType`]s from a function signature.
///
/// ```
/// use heimdall_common::ether::evm::core::types::parse_function_parameters;
/// use ethers::abi::ParamType;
///
/// let function_signature = "foo(uint256,uint256)";
/// let function_parameters = parse_function_parameters(function_signature).unwrap();
///
/// assert_eq!(function_parameters, vec![ParamType::Uint(256), ParamType::Uint(256)]);
/// ```
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

/// Helper function for extracting types from a string. Used by [`parse_function_parameters`],
/// typically after entering a nested tuple or similar.
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

                if let Some(array_size) = array_size {
                    // recursively call this function to extract the tuple types
                    let inner_types = extract_types_from_string(&tuple_types);

                    types.push(ParamType::FixedArray(
                        Box::new(ParamType::Tuple(inner_types.unwrap())),
                        array_size,
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
            if let Some(mut remaining_types) = extract_types_from_string(&string) {
                types.append(&mut remaining_types);
            }
        } else {
            // first type is not a tuple, so we can extract it
            let string_parts = string.splitn(2, ',').collect::<Vec<&str>>();

            // convert the string type to a ParamType
            if string_parts[0].is_empty() {
                // the first type is empty, so we can just recursively call this function to extract
                // the remaining types
                if let Some(mut remaining_types) = extract_types_from_string(string_parts[1]) {
                    types.append(&mut remaining_types);
                }
            } else {
                let param_type = to_type(string_parts[0]);
                types.push(param_type);

                // remove the first type from the string
                let string = string[string_parts[0].len() + 1..].to_string();

                // recursively call this function to extract the remaining types
                if let Some(mut remaining_types) = extract_types_from_string(&string) {
                    types.append(&mut remaining_types);
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

/// A helper function used by [`extract_types_from_string`] to check if the first type in a string
/// is a tuple.
fn is_first_type_tuple(string: &str) -> bool {
    // split by first comma
    let split = string.splitn(2, ',').collect::<Vec<&str>>();

    // if the first element starts with a (, it is a tuple
    split[0].starts_with('(')
}

/// A helper function used by [`extract_types_from_string`] that converts a string type to a
/// ParamType. For example, "address" will be converted to [`ParamType::Address`].
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
            if let Some(stripped) = string.strip_prefix("uint") {
                let size = stripped.parse::<usize>().unwrap_or(256);
                ParamType::Uint(size)
            } else if let Some(stripped) = string.strip_prefix("int") {
                let size = stripped.parse::<usize>().unwrap_or(256);
                ParamType::Int(size)
            } else if let Some(stripped) = string.strip_prefix("bytes") {
                let size = stripped.parse::<usize>().unwrap();
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

/// A helper function used by the decode module to pretty format decoded tokens.
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

/// Convert a bitwise masking operation to a tuple containing: \
/// 1. The size of the type being masked \
/// 2. Potential types that the type being masked could be.
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

/// Given a byte size, return a tuple containing: \
/// 1. The byte size \
/// 2. Potential types that the byte size could be.
///
/// ```
/// use heimdall_common::ether::evm::core::types::byte_size_to_type;
///
/// let (byte_size, potential_types) = byte_size_to_type(1);
/// assert_eq!(byte_size, 1);
/// assert_eq!(potential_types, vec!["bool".to_string(), "uint8".to_string(), "bytes1".to_string(), "int8".to_string()]);
/// ```
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

/// Given a string (typically a line of decompiled source code), extract a type cast if one exists.
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

#[cfg(test)]
mod tests {
    use ethers::abi::ParamType;

    use crate::ether::evm::core::types::parse_function_parameters;

    #[test]
    fn test_simple_signature() {
        let solidity_type = "test(uint256)".to_string();
        let param_type = parse_function_parameters(&solidity_type);
        assert_eq!(param_type, Some(vec![ParamType::Uint(256)]));
    }

    #[test]
    fn test_multiple_signature() {
        let solidity_type = "test(uint256,string)".to_string();
        let param_type = parse_function_parameters(&solidity_type);
        assert_eq!(param_type, Some(vec![ParamType::Uint(256), ParamType::String]));
    }

    #[test]
    fn test_array_signature() {
        let solidity_type = "test(uint256,string[],uint256)";
        let param_type = parse_function_parameters(solidity_type);
        assert_eq!(
            param_type,
            Some(vec![
                ParamType::Uint(256),
                ParamType::Array(Box::new(ParamType::String)),
                ParamType::Uint(256)
            ])
        );
    }

    #[test]
    fn test_array_fixed_signature() {
        let solidity_type = "test(uint256,string[2],uint256)";
        let param_type = parse_function_parameters(solidity_type);
        assert_eq!(
            param_type,
            Some(vec![
                ParamType::Uint(256),
                ParamType::FixedArray(Box::new(ParamType::String), 2),
                ParamType::Uint(256)
            ])
        );
    }

    #[test]
    fn test_complex_signature() {
        let solidity_type =
            "test(uint256,string,(address,address,uint24,address,uint256,uint256,uint256,uint160))";
        let param_type = parse_function_parameters(solidity_type);
        assert_eq!(
            param_type,
            Some(vec![
                ParamType::Uint(256),
                ParamType::String,
                ParamType::Tuple(vec![
                    ParamType::Address,
                    ParamType::Address,
                    ParamType::Uint(24),
                    ParamType::Address,
                    ParamType::Uint(256),
                    ParamType::Uint(256),
                    ParamType::Uint(256),
                    ParamType::Uint(160)
                ])
            ])
        );
    }

    #[test]
    fn test_tuple_signature() {
        let solidity_type =
            "exactInputSingle((address,address,uint24,address,uint256,uint256,uint256,uint160))";
        let param_type = parse_function_parameters(solidity_type);
        assert_eq!(
            param_type,
            Some(vec![ParamType::Tuple(vec![
                ParamType::Address,
                ParamType::Address,
                ParamType::Uint(24),
                ParamType::Address,
                ParamType::Uint(256),
                ParamType::Uint(256),
                ParamType::Uint(256),
                ParamType::Uint(160)
            ])])
        );
    }

    #[test]
    fn test_tuple_array_signature() {
        let solidity_type =
            "exactInputSingle((address,address,uint24,address,uint256,uint256,uint256,uint160)[])";
        let param_type = parse_function_parameters(solidity_type);
        assert_eq!(
            param_type,
            Some(vec![ParamType::Array(Box::new(ParamType::Tuple(vec![
                ParamType::Address,
                ParamType::Address,
                ParamType::Uint(24),
                ParamType::Address,
                ParamType::Uint(256),
                ParamType::Uint(256),
                ParamType::Uint(256),
                ParamType::Uint(160)
            ])))])
        );
    }

    #[test]
    fn test_tuple_fixedarray_signature() {
        let solidity_type =
            "exactInputSingle((address,address,uint24,address,uint256,uint256,uint256,uint160)[2])";
        let param_type = parse_function_parameters(solidity_type);
        assert_eq!(
            param_type,
            Some(vec![ParamType::FixedArray(
                Box::new(ParamType::Tuple(vec![
                    ParamType::Address,
                    ParamType::Address,
                    ParamType::Uint(24),
                    ParamType::Address,
                    ParamType::Uint(256),
                    ParamType::Uint(256),
                    ParamType::Uint(256),
                    ParamType::Uint(160)
                ])),
                2
            )])
        );
    }

    #[test]
    fn test_nested_tuple_signature() {
        let solidity_type = "exactInputSingle((address,address,uint24,address,uint256,(uint256,uint256)[],uint160))";
        let param_type = parse_function_parameters(solidity_type);
        assert_eq!(
            param_type,
            Some(vec![ParamType::Tuple(vec![
                ParamType::Address,
                ParamType::Address,
                ParamType::Uint(24),
                ParamType::Address,
                ParamType::Uint(256),
                ParamType::Array(Box::new(ParamType::Tuple(vec![
                    ParamType::Uint(256),
                    ParamType::Uint(256)
                ]))),
                ParamType::Uint(160)
            ])])
        );
    }

    #[test]
    fn test_seaport_fulfill_advanced_order() {
        let solidity_type = "fulfillAdvancedOrder(((address,address,(uint8,address,uint256,uint256,uint256)[],(uint8,address,uint256,uint256,uint256,address)[],uint8,uint256,uint256,bytes32,uint256,bytes32,uint256),uint120,uint120,bytes,bytes),(uint256,uint8,uint256,uint256,bytes32[])[],bytes32,address)";
        let param_type = parse_function_parameters(solidity_type);
        assert_eq!(
            param_type,
            Some(vec![
                ParamType::Tuple(vec![
                    ParamType::Tuple(vec![
                        ParamType::Address,
                        ParamType::Address,
                        ParamType::Array(Box::new(ParamType::Tuple(vec![
                            ParamType::Uint(8),
                            ParamType::Address,
                            ParamType::Uint(256),
                            ParamType::Uint(256),
                            ParamType::Uint(256)
                        ]))),
                        ParamType::Array(Box::new(ParamType::Tuple(vec![
                            ParamType::Uint(8),
                            ParamType::Address,
                            ParamType::Uint(256),
                            ParamType::Uint(256),
                            ParamType::Uint(256),
                            ParamType::Address
                        ]))),
                        ParamType::Uint(8),
                        ParamType::Uint(256),
                        ParamType::Uint(256),
                        ParamType::FixedBytes(32),
                        ParamType::Uint(256),
                        ParamType::FixedBytes(32),
                        ParamType::Uint(256)
                    ]),
                    ParamType::Uint(120),
                    ParamType::Uint(120),
                    ParamType::Bytes,
                    ParamType::Bytes
                ]),
                ParamType::Array(Box::new(ParamType::Tuple(vec![
                    ParamType::Uint(256),
                    ParamType::Uint(8),
                    ParamType::Uint(256),
                    ParamType::Uint(256),
                    ParamType::Array(Box::new(ParamType::FixedBytes(32)))
                ]))),
                ParamType::FixedBytes(32),
                ParamType::Address
            ])
        );
    }
}
