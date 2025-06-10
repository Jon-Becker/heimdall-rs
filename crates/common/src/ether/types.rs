//! Helper functions for parsing and converting Solidity types to Rust types.

use alloy_dyn_abi::{DynSolType, DynSolValue};
use alloy_json_abi::Param;
use serde_json::{Map, Number, Value};
use std::collections::VecDeque;

use crate::utils::strings::find_balanced_encapsulator;
use eyre::Result;

/// Enum representing the padding of a type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Padding {
    /// The value is left-padded. I.e. 0x0000...1234
    Left,
    /// The value is right-padded. I.e. 0x1234...0000
    Right,
    /// The value is not padded, or the padding is unknown.
    None,
}

/// Parse function parameters [`DynSolType`]s from a function signature.
///
/// ```
/// use heimdall_common::ether::types::parse_function_parameters;
/// use alloy_dyn_abi::DynSolType;
///
/// let function_signature = "foo(uint256,uint256)";
/// let function_parameters = parse_function_parameters(function_signature)
///     .expect("failed to parse function parameters");
///
/// assert_eq!(function_parameters, vec![DynSolType::Uint(256), DynSolType::Uint(256)]);
/// ```
pub fn parse_function_parameters(function_signature: &str) -> Result<Vec<DynSolType>> {
    // remove the function name from the signature, only keep the parameters
    let param_range = find_balanced_encapsulator(function_signature, ('(', ')'))?;

    let function_inputs = function_signature[param_range].to_string();

    // get inputs from the string
    extract_types_from_string(&function_inputs)
}

/// Helper function for extracting types from a string. Used by [`parse_function_parameters`],
/// typically after entering a nested tuple or similar.
fn extract_types_from_string(string: &str) -> Result<Vec<DynSolType>> {
    let mut types = Vec::new();
    if string.is_empty() {
        return Ok(types);
    }

    // if the string contains a tuple we cant simply split on commas
    if ['(', ')'].iter().any(|c| string.contains(*c)) {
        // check if first type is a tuple
        if is_first_type_tuple(string) {
            // get balanced encapsulator
            let tuple_range = find_balanced_encapsulator(string, ('(', ')'))?;

            // extract the tuple
            let tuple_types = string[tuple_range.clone()].to_string();

            // remove the tuple from the string
            let mut string = string[tuple_range.end + 1..].to_string();

            // if string is not empty, split on commas and check if tuple is an array
            let mut is_array = false;
            let mut array_size: Option<usize> = None;
            if !string.is_empty() {
                let split = string.splitn(2, ',').collect::<Vec<&str>>()[0];

                is_array = split.ends_with(']');

                // get array size, or none if []
                if is_array {
                    let array_range = find_balanced_encapsulator(split, ('[', ']'))?;

                    let size = split[array_range].to_string();
                    array_size = size.parse::<usize>().ok();
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

                // recursively call this function to extract the tuple types
                let inner_types = extract_types_from_string(&tuple_types)?;

                if let Some(array_size) = array_size {
                    types.push(DynSolType::FixedArray(
                        Box::new(DynSolType::Tuple(inner_types)),
                        array_size,
                    ))
                } else {
                    types.push(DynSolType::Array(Box::new(DynSolType::Tuple(inner_types))))
                }
            } else {
                // recursively call this function to extract the tuple types
                let inner_types = extract_types_from_string(&tuple_types)?;

                types.push(DynSolType::Tuple(inner_types));
            }

            // recursively call this function to extract the remaining types
            types.append(&mut extract_types_from_string(&string)?);
        } else {
            // first type is not a tuple, so we can extract it
            let string_parts = string.splitn(2, ',').collect::<Vec<&str>>();

            // convert the string type to a DynSolType
            if string_parts[0].is_empty() {
                // the first type is empty, so we can just recursively call this function to extract
                // the remaining types
                types.append(&mut extract_types_from_string(string_parts[1])?);
            } else {
                let param_type = to_type(string_parts[0]);
                types.push(param_type);

                // remove the first type from the string
                let string = string[string_parts[0].len() + 1..].to_string();

                // recursively call this function to extract the remaining types
                types.append(&mut extract_types_from_string(&string)?);
            }
        }
    } else {
        // split on commas
        let split = string.split(',').collect::<Vec<&str>>();

        // iterate over the split string and convert each type to a DynSolType
        for string_type in split {
            if string_type.is_empty() {
                continue;
            }

            let param_type = to_type(string_type);
            types.push(param_type);
        }
    }

    Ok(types)
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
/// DynSolType. For example, "address" will be converted to [`DynSolType::Address`].
pub fn to_type(string: &str) -> DynSolType {
    let is_array = string.ends_with(']');
    let mut array_size: VecDeque<Option<usize>> = VecDeque::new();
    let mut string = string.to_string();

    // while string contains a [..]
    while string.ends_with(']') {
        let array_range = match find_balanced_encapsulator(&string, ('[', ']')) {
            Ok(range) => range,
            Err(_) => return DynSolType::Bytes, // default to bytes if invalid
        };

        let size = string[array_range].to_string();

        array_size.push_back(size.parse::<usize>().ok());

        string = string.replacen(&format!("[{}]", &size), "", 1);
    }

    let arg_type = match string.as_str().replace("memory", "").trim() {
        "address" => DynSolType::Address,
        "bool" => DynSolType::Bool,
        "string" => DynSolType::String,
        "bytes" => DynSolType::Bytes,
        _ => {
            if let Some(stripped) = string.strip_prefix("uint") {
                let size = stripped.parse::<usize>().unwrap_or(256);
                DynSolType::Uint(size)
            } else if let Some(stripped) = string.strip_prefix("int") {
                let size = stripped.parse::<usize>().unwrap_or(256);
                DynSolType::Int(size)
            } else if let Some(stripped) = string.strip_prefix("bytes") {
                let size = stripped.parse::<usize>().unwrap_or(32);
                DynSolType::FixedBytes(size)
            } else {
                // default to bytes if invalid
                DynSolType::Bytes
            }
        }
    };

    if is_array {
        let mut arg_type = arg_type;

        // while array_size is not empty
        while !array_size.is_empty() {
            // pop off first element of array_size
            if let Some(size) =
                array_size.pop_front().expect("impossible case: failed to pop from array_size")
            {
                arg_type = DynSolType::FixedArray(Box::new(arg_type), size);
            } else {
                arg_type = DynSolType::Array(Box::new(arg_type));
            }
        }

        return arg_type;
    }

    arg_type
}

/// Convert a given DynSolType to its abi-safe "type" string representation
pub fn to_abi_string(param_type: &DynSolType) -> String {
    match param_type {
        DynSolType::Array(inner) => format!("{}[]", to_abi_string(inner)),
        DynSolType::FixedArray(inner, size) => format!("{}[{}]", to_abi_string(inner), size),
        DynSolType::Tuple(_) => "tuple".to_string(),
        _ => param_type.to_string(),
    }
}

/// Convert a given DynSolType to its abi-safe "components"
pub fn to_components(param_type: &DynSolType) -> Vec<Param> {
    match param_type {
        DynSolType::Array(inner) | DynSolType::FixedArray(inner, _) => to_components(inner),
        DynSolType::Tuple(params) => params
            .iter()
            .enumerate()
            .map(|(i, p)| Param {
                ty: to_abi_string(p),
                name: format!("component{i}"),
                components: to_components(p),
                internal_type: None,
            })
            .collect::<Vec<_>>(),
        _ => vec![],
    }
}

/// an extension on DynSolValue which allows serialization to a string
pub trait DynSolValueExt {
    /// Serialize the value to a serde_json::Value
    fn serialize(&self) -> Value;
}

impl DynSolValueExt for DynSolValue {
    fn serialize(&self) -> Value {
        match self {
            DynSolValue::Address(addr) => Value::String(addr.to_string()),
            DynSolValue::Bool(b) => Value::Bool(*b),
            DynSolValue::String(s) => Value::String(s.to_owned()),
            DynSolValue::Bytes(b) => {
                Value::Array(b.iter().map(|b| Value::Number(Number::from(*b))).collect())
            }
            DynSolValue::Uint(u, _) => Value::String(u.to_string()),
            DynSolValue::Int(i, _) => Value::String(i.to_string()),
            DynSolValue::FixedBytes(b, _) => {
                Value::Array(b.iter().map(|b| Value::Number(Number::from(*b))).collect())
            }
            DynSolValue::Array(arr) => Value::Array(arr.iter().map(|v| v.serialize()).collect()),
            DynSolValue::FixedArray(arr) => {
                Value::Array(arr.iter().map(|v| v.serialize()).collect())
            }
            DynSolValue::Tuple(t) => {
                let mut map = Map::new();
                for (i, v) in t.iter().enumerate() {
                    map.insert(format!("component{i}"), v.serialize());
                }
                Value::Object(map)
            }
            _ => Value::Null,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_signature() {
        let solidity_type = "test(uint256)".to_string();
        let param_type =
            parse_function_parameters(&solidity_type).expect("failed to parse function parameters");
        assert_eq!(param_type, vec![DynSolType::Uint(256)]);
    }

    #[test]
    fn test_multiple_signature() {
        let solidity_type = "test(uint256,string)".to_string();
        let param_type =
            parse_function_parameters(&solidity_type).expect("failed to parse function parameters");
        assert_eq!(param_type, vec![DynSolType::Uint(256), DynSolType::String]);
    }

    #[test]
    fn test_array_signature() {
        let solidity_type = "test(uint256,string[],uint256)";
        let param_type =
            parse_function_parameters(solidity_type).expect("failed to parse function parameters");
        assert_eq!(
            param_type,
            vec![
                DynSolType::Uint(256),
                DynSolType::Array(Box::new(DynSolType::String)),
                DynSolType::Uint(256)
            ]
        );
    }

    #[test]
    fn test_array_fixed_signature() {
        let solidity_type = "test(uint256,string[2],uint256)";
        let param_type =
            parse_function_parameters(solidity_type).expect("failed to parse function parameters");
        assert_eq!(
            param_type,
            vec![
                DynSolType::Uint(256),
                DynSolType::FixedArray(Box::new(DynSolType::String), 2),
                DynSolType::Uint(256)
            ]
        );
    }

    #[test]
    fn test_complex_signature() {
        let solidity_type =
            "test(uint256,string,(address,address,uint24,address,uint256,uint256,uint256,uint160))";
        let param_type =
            parse_function_parameters(solidity_type).expect("failed to parse function parameters");
        assert_eq!(
            param_type,
            vec![
                DynSolType::Uint(256),
                DynSolType::String,
                DynSolType::Tuple(vec![
                    DynSolType::Address,
                    DynSolType::Address,
                    DynSolType::Uint(24),
                    DynSolType::Address,
                    DynSolType::Uint(256),
                    DynSolType::Uint(256),
                    DynSolType::Uint(256),
                    DynSolType::Uint(160)
                ])
            ]
        );
    }

    #[test]
    fn test_tuple_signature() {
        let solidity_type =
            "exactInputSingle((address,address,uint24,address,uint256,uint256,uint256,uint160))";
        let param_type =
            parse_function_parameters(solidity_type).expect("failed to parse function parameters");
        assert_eq!(
            param_type,
            vec![DynSolType::Tuple(vec![
                DynSolType::Address,
                DynSolType::Address,
                DynSolType::Uint(24),
                DynSolType::Address,
                DynSolType::Uint(256),
                DynSolType::Uint(256),
                DynSolType::Uint(256),
                DynSolType::Uint(160)
            ])]
        );
    }

    #[test]
    fn test_tuple_array_signature() {
        let solidity_type =
            "exactInputSingle((address,address,uint24,address,uint256,uint256,uint256,uint160)[])";
        let param_type =
            parse_function_parameters(solidity_type).expect("failed to parse function parameters");
        assert_eq!(
            param_type,
            vec![DynSolType::Array(Box::new(DynSolType::Tuple(vec![
                DynSolType::Address,
                DynSolType::Address,
                DynSolType::Uint(24),
                DynSolType::Address,
                DynSolType::Uint(256),
                DynSolType::Uint(256),
                DynSolType::Uint(256),
                DynSolType::Uint(160)
            ])))]
        );
    }

    #[test]
    fn test_tuple_fixedarray_signature() {
        let solidity_type =
            "exactInputSingle((address,address,uint24,address,uint256,uint256,uint256,uint160)[2])";
        let param_type =
            parse_function_parameters(solidity_type).expect("failed to parse function parameters");
        assert_eq!(
            param_type,
            vec![DynSolType::FixedArray(
                Box::new(DynSolType::Tuple(vec![
                    DynSolType::Address,
                    DynSolType::Address,
                    DynSolType::Uint(24),
                    DynSolType::Address,
                    DynSolType::Uint(256),
                    DynSolType::Uint(256),
                    DynSolType::Uint(256),
                    DynSolType::Uint(160)
                ])),
                2
            )]
        );
    }

    #[test]
    fn test_nested_tuple_signature() {
        let solidity_type = "exactInputSingle((address,address,uint24,address,uint256,(uint256,uint256)[],uint160))";
        let param_type =
            parse_function_parameters(solidity_type).expect("failed to parse function parameters");
        assert_eq!(
            param_type,
            vec![DynSolType::Tuple(vec![
                DynSolType::Address,
                DynSolType::Address,
                DynSolType::Uint(24),
                DynSolType::Address,
                DynSolType::Uint(256),
                DynSolType::Array(Box::new(DynSolType::Tuple(vec![
                    DynSolType::Uint(256),
                    DynSolType::Uint(256)
                ]))),
                DynSolType::Uint(160)
            ])]
        );
    }

    #[test]
    fn test_seaport_fulfill_advanced_order() {
        let solidity_type = "fulfillAdvancedOrder(((address,address,(uint8,address,uint256,uint256,uint256)[],(uint8,address,uint256,uint256,uint256,address)[],uint8,uint256,uint256,bytes32,uint256,bytes32,uint256),uint120,uint120,bytes,bytes),(uint256,uint8,uint256,uint256,bytes32[])[],bytes32,address)";
        let param_type =
            parse_function_parameters(solidity_type).expect("failed to parse function parameters");
        assert_eq!(
            param_type,
            vec![
                DynSolType::Tuple(vec![
                    DynSolType::Tuple(vec![
                        DynSolType::Address,
                        DynSolType::Address,
                        DynSolType::Array(Box::new(DynSolType::Tuple(vec![
                            DynSolType::Uint(8),
                            DynSolType::Address,
                            DynSolType::Uint(256),
                            DynSolType::Uint(256),
                            DynSolType::Uint(256)
                        ]))),
                        DynSolType::Array(Box::new(DynSolType::Tuple(vec![
                            DynSolType::Uint(8),
                            DynSolType::Address,
                            DynSolType::Uint(256),
                            DynSolType::Uint(256),
                            DynSolType::Uint(256),
                            DynSolType::Address
                        ]))),
                        DynSolType::Uint(8),
                        DynSolType::Uint(256),
                        DynSolType::Uint(256),
                        DynSolType::FixedBytes(32),
                        DynSolType::Uint(256),
                        DynSolType::FixedBytes(32),
                        DynSolType::Uint(256)
                    ]),
                    DynSolType::Uint(120),
                    DynSolType::Uint(120),
                    DynSolType::Bytes,
                    DynSolType::Bytes
                ]),
                DynSolType::Array(Box::new(DynSolType::Tuple(vec![
                    DynSolType::Uint(256),
                    DynSolType::Uint(8),
                    DynSolType::Uint(256),
                    DynSolType::Uint(256),
                    DynSolType::Array(Box::new(DynSolType::FixedBytes(32)))
                ]))),
                DynSolType::FixedBytes(32),
                DynSolType::Address
            ]
        );
    }

    #[test]
    fn test_to_type_address() {
        let input = "address";
        assert_eq!(super::to_type(input), DynSolType::Address);
    }

    #[test]
    fn test_to_type_bool() {
        let input = "bool";
        assert_eq!(super::to_type(input), DynSolType::Bool);
    }

    #[test]
    fn test_to_type_string() {
        let input = "string";
        assert_eq!(super::to_type(input), DynSolType::String);
    }

    #[test]
    fn test_to_type_bytes() {
        let input = "bytes";
        assert_eq!(super::to_type(input), DynSolType::Bytes);
    }

    #[test]
    fn test_to_type_uint256() {
        let input = "uint256";
        assert_eq!(super::to_type(input), DynSolType::Uint(256));
    }

    #[test]
    fn test_to_type_int() {
        let input = "int256";
        assert_eq!(super::to_type(input), DynSolType::Int(256));
    }

    #[test]
    fn test_to_type_bytes1() {
        let input = "bytes1";
        assert_eq!(super::to_type(input), DynSolType::FixedBytes(1));
    }

    #[test]
    fn test_to_type_uint() {
        let input = "uint";
        assert_eq!(super::to_type(input), DynSolType::Uint(256));
    }

    #[test]
    fn test_to_type_array() {
        let input = "uint8[]";
        assert_eq!(super::to_type(input), DynSolType::Array(Box::new(DynSolType::Uint(8))));
    }

    #[test]
    fn test_to_type_nested_array() {
        let input = "uint8[][]";
        assert_eq!(
            super::to_type(input),
            DynSolType::Array(Box::new(DynSolType::Array(Box::new(DynSolType::Uint(8)))))
        );
    }

    #[test]
    fn test_to_type_fixed_array() {
        let input = "uint8[2]";
        assert_eq!(super::to_type(input), DynSolType::FixedArray(Box::new(DynSolType::Uint(8)), 2));
    }

    #[test]
    fn test_to_type_nested_fixed_array() {
        let input = "uint8[2][2]";
        assert_eq!(
            super::to_type(input),
            DynSolType::FixedArray(
                Box::new(DynSolType::FixedArray(Box::new(DynSolType::Uint(8)), 2)),
                2
            )
        );
    }

    #[test]
    fn test_to_type_nested_fixed_array_ordering() {
        let input = "uint8[2][3][2]";
        assert_eq!(
            super::to_type(input),
            DynSolType::FixedArray(
                Box::new(DynSolType::FixedArray(
                    Box::new(DynSolType::FixedArray(Box::new(DynSolType::Uint(8)), 2)),
                    3
                )),
                2
            )
        );
    }

    #[test]
    fn test_to_abi_string_simple() {
        let input = DynSolType::String;
        assert_eq!(super::to_abi_string(&input), "string");
    }

    #[test]
    fn test_to_abi_string_array() {
        let input = DynSolType::Array(Box::new(DynSolType::Uint(8)));
        assert_eq!(super::to_abi_string(&input), "uint8[]");
    }

    #[test]
    fn test_to_abi_string_fixed_array() {
        let input = DynSolType::FixedArray(Box::new(DynSolType::Uint(8)), 2);
        assert_eq!(super::to_abi_string(&input), "uint8[2]");
    }

    #[test]
    fn test_to_abi_string_tuple() {
        let input = DynSolType::Tuple(vec![DynSolType::Uint(8), DynSolType::Uint(256)]);
        assert_eq!(super::to_abi_string(&input), "tuple");
    }

    #[test]
    fn test_to_components_simple() {
        let input = DynSolType::String;
        assert_eq!(super::to_components(&input), vec![]);
    }

    #[test]
    fn test_to_components_array() {
        let input = DynSolType::Array(Box::new(DynSolType::Uint(8)));
        assert_eq!(super::to_components(&input), vec![]);
    }

    #[test]
    fn test_to_components_tuple() {
        let input = DynSolType::Tuple(vec![DynSolType::Uint(8), DynSolType::Uint(256)]);
        assert_eq!(super::to_components(&input).len(), 2);
    }
}
