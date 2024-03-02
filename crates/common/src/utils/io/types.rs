use colored::Colorize;
use ethers::abi::Token;

use crate::utils::hex::ToLowerHex;

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

pub trait Parameterize {
    fn parameterize(&self) -> String;
    fn to_type(&self) -> String;
}

/// A helper function used by the decode module to pretty format decoded tokens.
///
/// ```
/// use ethers::abi::Token;
/// use ethers::types::Address;
/// use heimdall_common::utils::io::types::Parameterize;
///
/// let output = Token::Address(Address::zero()).parameterize();
/// assert_eq!(output, "address: 0x0000000000000000000000000000000000000000".to_string());
/// ```
impl Parameterize for Token {
    fn parameterize(&self) -> String {
        match self {
            Token::Address(val) => format!("address: {}", val.to_lower_hex()),
            Token::Int(val) => format!("int: {}", val),
            Token::Uint(val) => format!("uint: {}", val),
            Token::String(val) => format!("string: {}", val),
            Token::Bool(val) => format!("bool: {}", val),
            Token::Bytes(val) => format!("bytes: 0x{}", val.to_lower_hex()),
            Token::FixedBytes(val) => format!("bytes{}: 0x{}", val.len(), val.to_lower_hex()),
            Token::Array(val) => {
                // get type of array
                let array_type = match val.first() {
                    Some(token) => token.to_type(),
                    None => String::new(),
                };

                // parametrize all array elements, remove their `type: ` prefix, and join them
                let elements = val
                    .iter()
                    .map(|token| token.parameterize().replace(&format!("{}: ", array_type), ""))
                    .collect::<Vec<String>>()
                    .join(", ");

                // return array type and elements
                format!("{}[]: [{}]", array_type, elements)
            }
            Token::FixedArray(val) => {
                // get type of array
                let array_type = match val.first() {
                    Some(token) => token.to_type(),
                    None => String::new(),
                };

                // parametrize all array elements, remove their `type: ` prefix, and join them
                let elements = val
                    .iter()
                    .map(|token| token.parameterize().replace(&format!("{}: ", array_type), ""))
                    .collect::<Vec<String>>()
                    .join(", ");

                // return array type and elements
                format!("{}[{}]: [{}]", array_type, val.len(), elements)
            }
            Token::Tuple(val) => {
                // return tuple type and elements
                format!(
                    "({})",
                    val.iter()
                        .map(|token| token.parameterize())
                        .collect::<Vec<String>>()
                        .join(", ")
                )
            }
        }
    }

    fn to_type(&self) -> String {
        match self {
            Token::Address(_) => "address".to_string(),
            Token::Int(_) => "int".to_string(),
            Token::Uint(_) => "uint".to_string(),
            Token::String(_) => "string".to_string(),
            Token::Bool(_) => "bool".to_string(),
            Token::Bytes(_) => "bytes".to_string(),
            Token::FixedBytes(val) => format!("bytes{}", val.len()),
            Token::Array(val) => {
                // get type of array
                let array_type = match val.first() {
                    Some(token) => token.to_type(),
                    None => String::new(),
                };
                format!("{}[]", array_type)
            }
            Token::FixedArray(val) => {
                // get type of array
                let array_type = match val.first() {
                    Some(token) => token.to_type(),
                    None => String::new(),
                };
                format!("{}[{}]", array_type, val.len())
            }
            Token::Tuple(val) => {
                // get all internal types
                let types =
                    val.iter().map(|token| token.to_type()).collect::<Vec<String>>().join(", ");

                // return tuple type
                format!("({})", types)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use ethers::types::Address;

    use super::*;

    #[test]
    fn test_parameterize_address() {
        let output = Token::Address(Address::zero()).parameterize();
        assert_eq!(output, "address: 0x0000000000000000000000000000000000000000".to_string());
    }

    #[test]
    fn test_parameterize_int() {
        let output = Token::Int(1.into()).parameterize();
        assert_eq!(output, "int: 1".to_string());
    }

    #[test]
    fn test_parameterize_uint() {
        let output = Token::Uint(1.into()).parameterize();
        assert_eq!(output, "uint: 1".to_string());
    }

    #[test]
    fn test_parameterize_string() {
        let output = Token::String("test".to_string()).parameterize();
        assert_eq!(output, "string: test".to_string());
    }

    #[test]
    fn test_parameterize_bool() {
        let output = Token::Bool(true).parameterize();
        assert_eq!(output, "bool: true".to_string());
    }

    #[test]
    fn test_parameterize_bytes() {
        let output = Token::Bytes(vec![0x01, 0x02, 0x03]).parameterize();
        assert_eq!(output, "bytes: 0x010203".to_string());
    }

    #[test]
    fn test_parameterize_fixed_bytes() {
        let output = Token::FixedBytes(vec![0x01, 0x02, 0x03]).parameterize();
        assert_eq!(output, "bytes3: 0x010203".to_string());
    }

    #[test]
    fn test_parameterize_array() {
        let output =
            Token::Array(vec![Token::Uint(1.into()), Token::Uint(2.into())]).parameterize();
        assert_eq!(output, "uint[]: [1, 2]".to_string());
    }

    #[test]
    fn test_parameterize_fixed_array() {
        let output =
            Token::FixedArray(vec![Token::Uint(1.into()), Token::Uint(2.into())]).parameterize();
        assert_eq!(output, "uint[2]: [1, 2]".to_string());
    }

    #[test]
    fn test_parameterize_tuple() {
        let output =
            Token::Tuple(vec![Token::Uint(1.into()), Token::Uint(2.into()), Token::Uint(3.into())])
                .parameterize();
        assert_eq!(output, "(uint: 1, uint: 2, uint: 3)".to_string());
    }

    #[test]
    fn test_parameterize_nested_array() {
        let output = Token::Array(vec![
            Token::Array(vec![Token::Uint(1.into()), Token::Uint(2.into())]),
            Token::Array(vec![Token::Uint(3.into()), Token::Uint(4.into())]),
        ])
        .parameterize();
        assert_eq!(output, "uint[][]: [[1, 2], [3, 4]]".to_string());
    }
}
