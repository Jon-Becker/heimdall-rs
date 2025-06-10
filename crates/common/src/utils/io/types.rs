use alloy_dyn_abi::DynSolValue;
use colored::Colorize;

use crate::utils::{hex::ToLowerHex, strings::encode_hex};

/// A helper function used by the decode module to pretty format decoded tokens.
pub fn display(inputs: Vec<DynSolValue>, prefix: &str) -> Vec<String> {
    let mut output = Vec::new();
    let prefix = prefix.to_string();

    for input in inputs {
        match input {
            DynSolValue::Address(val) => {
                output.push(format!("{prefix}{} {}", "address".blue(), val))
            }
            DynSolValue::Int(val, _) => {
                output.push(format!("{prefix}{} {}", "int    ".blue(), val))
            }
            DynSolValue::Uint(val, _) => {
                output.push(format!("{prefix}{} {}", "uint   ".blue(), val))
            }
            DynSolValue::String(val) => output.push(format!("{prefix}{} {val}", "string ".blue())),
            DynSolValue::Bool(val) => {
                if val {
                    output.push(format!("{prefix}{} true", "bool   ".blue()));
                } else {
                    output.push(format!("{prefix}{} false", "bool   ".blue()));
                }
            }
            DynSolValue::FixedBytes(val, _) => {
                output.push(format!("{prefix}{} {}", "bytes  ".blue(), val));
            }
            DynSolValue::Bytes(val) => {
                // chunk val into 32-byte chunks
                let bytes = val.chunks(32).map(encode_hex).collect::<Vec<String>>();

                for (i, byte) in bytes.iter().enumerate() {
                    if i == 0 {
                        output.push(format!("{prefix}{} 0x{}", "bytes  ".blue(), byte));
                    } else {
                        output.push(format!("{prefix}{}   {}", "       ".blue(), byte));
                    }
                }
            }
            DynSolValue::FixedArray(val) | DynSolValue::Array(val) => {
                if val.is_empty() {
                    output.push(format!("{prefix}[]"));
                } else {
                    output.push(format!("{prefix}["));
                    output.extend(display(val.to_vec(), &format!("{prefix}   ")));
                    output.push(format!("{prefix}]"));
                }
            }
            DynSolValue::Tuple(val) => {
                if val.is_empty() {
                    output.push(format!("{prefix}()"));
                } else {
                    output.push(format!("{prefix}("));
                    output.extend(display(val.to_vec(), &format!("{prefix}   ")));
                    output.push(format!("{prefix})"));
                }
            }
            _ => unreachable!(),
        };
    }

    output
}

/// Trait for converting values to parameterized strings and type information.
///
/// This trait is used primarily for displaying and serializing function parameters
/// in a readable format when presenting decoded contract data.
pub trait Parameterize {
    /// Converts the value to a parameterized string representation.
    ///
    /// # Returns
    ///
    /// * `String` - The string representation of the parameter value
    fn parameterize(&self) -> String;

    /// Returns the type name of the parameter as a string.
    ///
    /// # Returns
    ///
    /// * `String` - The type name (e.g., "uint256", "address", etc.)
    fn to_type(&self) -> String;
}

/// A helper function used by the decode module to pretty format decoded tokens.
///
/// ```
/// use heimdall_common::utils::io::types::Parameterize;
/// use alloy_dyn_abi::DynSolValue;
/// use alloy::primitives::Address;
///
/// let output = DynSolValue::Address(Address::ZERO).parameterize();
/// assert_eq!(output, "address: 0x0000000000000000000000000000000000000000".to_string());
/// ```
impl Parameterize for DynSolValue {
    fn parameterize(&self) -> String {
        match self {
            DynSolValue::Address(val) => format!("address: {val}"),
            DynSolValue::Int(val, _) => format!("int: {val}"),
            DynSolValue::Uint(val, _) => format!("uint: {val}"),
            DynSolValue::String(val) => format!("string: {val}"),
            DynSolValue::Bool(val) => format!("bool: {val}"),
            DynSolValue::Bytes(val) => format!("bytes: 0x{}", val.to_lower_hex()),
            DynSolValue::FixedBytes(val, size) => {
                format!("bytes{}: 0x{}", size, &val.to_string()[(64 - size * 2) + 2..])
            }
            DynSolValue::Array(val) => {
                // get type of array
                let array_type = match val.first() {
                    Some(token) => token.to_type(),
                    None => String::new(),
                };

                // parametrize all array elements, remove their `type: ` prefix, and join them
                let elements = val
                    .iter()
                    .map(|token| token.parameterize().replace(&format!("{array_type}: "), ""))
                    .collect::<Vec<String>>()
                    .join(", ");

                // return array type and elements
                format!("{array_type}[]: [{elements}]")
            }
            DynSolValue::FixedArray(val) => {
                // get type of array
                let array_type = match val.first() {
                    Some(token) => token.to_type(),
                    None => String::new(),
                };

                // parametrize all array elements, remove their `type: ` prefix, and join them
                let elements = val
                    .iter()
                    .map(|token| token.parameterize().replace(&format!("{array_type}: "), ""))
                    .collect::<Vec<String>>()
                    .join(", ");

                // return array type and elements
                format!("{}[{}]: [{}]", array_type, val.len(), elements)
            }
            DynSolValue::Tuple(val) => {
                // return tuple type and elements
                format!(
                    "({})",
                    val.iter()
                        .map(|token| token.parameterize())
                        .collect::<Vec<String>>()
                        .join(", ")
                )
            }
            _ => unreachable!(),
        }
    }

    fn to_type(&self) -> String {
        match self {
            DynSolValue::Address(_) => "address".to_string(),
            DynSolValue::Int(..) => "int".to_string(),
            DynSolValue::Uint(..) => "uint".to_string(),
            DynSolValue::String(_) => "string".to_string(),
            DynSolValue::Bool(_) => "bool".to_string(),
            DynSolValue::Bytes(_) => "bytes".to_string(),
            DynSolValue::FixedBytes(_, size) => format!("bytes{size}"),
            DynSolValue::Array(val) => {
                // get type of array
                let array_type = match val.first() {
                    Some(token) => token.to_type(),
                    None => String::new(),
                };
                format!("{array_type}[]")
            }
            DynSolValue::FixedArray(val) => {
                // get type of array
                let array_type = match val.first() {
                    Some(token) => token.to_type(),
                    None => String::new(),
                };
                format!("{}[{}]", array_type, val.len())
            }
            DynSolValue::Tuple(val) => {
                // get all internal types
                let types =
                    val.iter().map(|token| token.to_type()).collect::<Vec<String>>().join(", ");

                // return tuple type
                format!("({types})")
            }
            _ => unreachable!(),
        }
    }
}

#[cfg(test)]
mod tests {

    use alloy::primitives::{Address, FixedBytes};

    use super::*;

    #[test]
    fn test_parameterize_address() {
        let output = DynSolValue::Address(Address::ZERO).parameterize();
        assert_eq!(output, "address: 0x0000000000000000000000000000000000000000".to_string());
    }

    #[test]
    fn test_parameterize_int() {
        let output = DynSolValue::Int(1.try_into().expect("invalid"), 256).parameterize();
        assert_eq!(output, "int: 1".to_string());
    }

    #[test]
    fn test_parameterize_uint() {
        let output = DynSolValue::Uint(1.try_into().expect("invalid"), 256).parameterize();
        assert_eq!(output, "uint: 1".to_string());
    }

    #[test]
    fn test_parameterize_string() {
        let output = DynSolValue::String("test".to_string()).parameterize();
        assert_eq!(output, "string: test".to_string());
    }

    #[test]
    fn test_parameterize_bool() {
        let output = DynSolValue::Bool(true).parameterize();
        assert_eq!(output, "bool: true".to_string());
    }

    #[test]
    fn test_parameterize_bytes() {
        let output = DynSolValue::Bytes(vec![0x01, 0x02, 0x03]).parameterize();
        assert_eq!(output, "bytes: 0x010203".to_string());
    }

    #[test]
    fn test_parameterize_fixed_bytes() {
        let output = DynSolValue::FixedBytes(
            FixedBytes([
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                0x00, 0x01, 0x02, 0x03,
            ]),
            3,
        )
        .parameterize();
        assert_eq!(output, "bytes3: 0x010203".to_string());
    }

    #[test]
    fn test_parameterize_array() {
        let output = DynSolValue::Array(vec![
            DynSolValue::Uint(1.try_into().expect("invalid"), 256),
            DynSolValue::Uint(2.try_into().expect("invalid"), 256),
        ])
        .parameterize();
        assert_eq!(output, "uint[]: [1, 2]".to_string());
    }

    #[test]
    fn test_parameterize_fixed_array() {
        let output = DynSolValue::FixedArray(vec![
            DynSolValue::Uint(1.try_into().expect("invalid"), 256),
            DynSolValue::Uint(2.try_into().expect("invalid"), 256),
        ])
        .parameterize();
        assert_eq!(output, "uint[2]: [1, 2]".to_string());
    }

    #[test]
    fn test_parameterize_tuple() {
        let output = DynSolValue::Tuple(vec![
            DynSolValue::Uint(1.try_into().expect("invalid"), 256),
            DynSolValue::Uint(2.try_into().expect("invalid"), 256),
            DynSolValue::Uint(3.try_into().expect("invalid"), 256),
        ])
        .parameterize();
        assert_eq!(output, "(uint: 1, uint: 2, uint: 3)".to_string());
    }

    #[test]
    fn test_parameterize_nested_array() {
        let output = DynSolValue::Array(vec![
            DynSolValue::Array(vec![
                DynSolValue::Uint(1.try_into().expect("invalid"), 256),
                DynSolValue::Uint(2.try_into().expect("invalid"), 256),
            ]),
            DynSolValue::Array(vec![
                DynSolValue::Uint(3.try_into().expect("invalid"), 256),
                DynSolValue::Uint(4.try_into().expect("invalid"), 256),
            ]),
        ])
        .parameterize();
        assert_eq!(output, "uint[][]: [[1, 2], [3, 4]]".to_string());
    }
}
