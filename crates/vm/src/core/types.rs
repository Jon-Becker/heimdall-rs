use std::{collections::VecDeque, ops::Range};

use alloy::{dyn_abi::DynSolType, sol_types::SolValue};
use eyre::{eyre, Result};
use heimdall_common::{
    constants::TYPE_CAST_REGEX,
    utils::strings::{encode_hex, find_balanced_encapsulator},
};

use super::{
    opcodes::{WrappedInput, AND, CALLDATACOPY, CALLDATALOAD, OR},
    vm::Instruction,
};

/// Indicates the type of padding in a byte array
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Padding {
    /// Padding exists on the left side (higher order bytes)
    Left,

    /// Padding exists on the right side (lower order bytes)
    Right,

    /// No padding exists
    None,
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

    let arg_type = match string.as_str() {
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

/// Convert a bitwise masking operation to a tuple containing: \
/// 1. The size of the type being masked \
/// 2. Potential types that the type being masked could be.
pub fn convert_bitmask(instruction: &Instruction) -> (usize, Vec<String>) {
    let mask = &instruction.output_operations[0];

    // use 32 as the default size, as it is the default word size in the EVM
    let mut type_byte_size = 32;

    // determine which input contains the bitmask
    for (i, input) in mask.inputs.iter().enumerate() {
        match input {
            WrappedInput::Raw(_) => continue,
            WrappedInput::Opcode(opcode) => {
                if !(opcode.opcode == CALLDATALOAD || opcode.opcode == CALLDATACOPY) {
                    if mask.opcode == AND {
                        type_byte_size =
                            encode_hex(&instruction.inputs[i].abi_encode()).matches("ff").count();
                    } else if mask.opcode == OR {
                        type_byte_size =
                            encode_hex(&instruction.inputs[i].abi_encode()).matches("00").count();
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
/// use heimdall_vm::core::types::byte_size_to_type;
///
/// let (byte_size, potential_types) = byte_size_to_type(1);
/// assert_eq!(byte_size, 1);
/// assert_eq!(potential_types, vec!["bool".to_string(), "uint8".to_string(), "bytes1".to_string(), "int8".to_string()]);
/// ```
pub fn byte_size_to_type(byte_size: usize) -> (usize, Vec<String>) {
    let mut potential_types = Vec::new();

    match byte_size {
        1 => potential_types.push("bool".to_string()),
        15..=20 => potential_types.push("address".to_string()),
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
///
/// ```
/// use heimdall_vm::core::types::find_cast;
/// use alloy::dyn_abi::DynSolType;
///
/// let line = "uint256(0x000011)";
/// let (range, cast_type) = find_cast(line).expect("failed to find type cast");
/// assert_eq!(range, 8..16);
/// assert_eq!(&line[range], "0x000011");
/// assert_eq!(cast_type, DynSolType::Uint(256));
/// ```
pub fn find_cast(line: &str) -> Result<(Range<usize>, DynSolType)> {
    // find the start of the cast
    match TYPE_CAST_REGEX.find(line).expect("Failed to find type cast.") {
        Some(m) => {
            let start = m.start();
            let end = m.end() - 1;
            let cast_type = line[start..].split('(').collect::<Vec<&str>>()[0].to_string();

            // find where the cast ends
            let range = find_balanced_encapsulator(&line[end..], ('(', ')'))?;
            Ok((end + range.start..end + range.end, to_type(&cast_type)))
        }
        None => Err(eyre!("failed to find type cast")),
    }
}

/// Given a string of bytes, determine if it is left or right padded.
pub fn get_padding(bytes: &[u8]) -> Padding {
    let size = bytes.len();

    // get indices of null bytes in the decoded bytes
    let null_byte_indices = bytes
        .iter()
        .enumerate()
        .filter(|(_, byte)| **byte == 0)
        .map(|(index, _)| index)
        .collect::<Vec<usize>>();

    // we can avoid doing a full check if any of the following are true:
    // there are no null bytes OR
    // neither first nor last byte is a null byte, it is not padded
    if null_byte_indices.is_empty() ||
        null_byte_indices[0] != 0 && null_byte_indices[null_byte_indices.len() - 1] != size - 1
    {
        return Padding::None;
    }

    // the first byte is a null byte AND the last byte is not a null byte, it is left padded
    if null_byte_indices[0] == 0 && null_byte_indices[null_byte_indices.len() - 1] != size - 1 {
        return Padding::Left;
    }

    // the first byte is not a null byte AND the last byte is a null byte, it is right padded
    if null_byte_indices[0] != 0 && null_byte_indices[null_byte_indices.len() - 1] == size - 1 {
        return Padding::Right;
    }

    // get non-null byte indices
    let non_null_byte_indices = bytes
        .iter()
        .enumerate()
        .filter(|(_, byte)| **byte != 0)
        .map(|(index, _)| index)
        .collect::<Vec<usize>>();

    if non_null_byte_indices.is_empty() {
        return Padding::None;
    }

    // check if the there are more null-bytes before the first non-null byte than after the last
    // non-null byte
    let left_hand_padding =
        null_byte_indices.iter().filter(|index| **index < non_null_byte_indices[0]).count();
    let right_hand_padding = null_byte_indices
        .iter()
        .filter(|index| **index > non_null_byte_indices[non_null_byte_indices.len() - 1])
        .count();

    match left_hand_padding.cmp(&right_hand_padding) {
        std::cmp::Ordering::Greater => Padding::Left,
        std::cmp::Ordering::Less => Padding::Right,
        std::cmp::Ordering::Equal => Padding::None,
    }
}

/// Given a string of bytes, get the max padding size for the data
pub fn get_padding_size(bytes: &[u8]) -> usize {
    match get_padding(bytes) {
        Padding::Left => {
            // count number of null-bytes at the start of the data
            bytes.iter().take_while(|byte| **byte == 0).count()
        }
        Padding::Right => {
            // count number of null-bytes at the end of the data
            bytes.iter().rev().take_while(|byte| **byte == 0).count()
        }
        _ => 0,
    }
}

/// Analyzes a byte array and determines potential Solidity types that could represent it
///
/// This function examines the given word (byte array) and returns:
/// 1. The minimum size in bytes needed to store the word
/// 2. A list of possible Solidity type names that could represent the data
///
/// # Arguments
/// * `word` - The byte array to analyze
///
/// # Returns
/// * A tuple containing:
///   - The minimum size in bytes needed to store the word
///   - A vector of strings representing potential Solidity types
pub fn get_potential_types_for_word(word: &[u8]) -> (usize, Vec<String>) {
    // get padding of the word, note this is a maximum
    let padding_size = get_padding_size(word);

    // get number of bytes padded
    let data_size = word.len() - padding_size;
    byte_size_to_type(data_size)
}

#[cfg(test)]
mod tests {
    use super::*;
    use heimdall_common::{ether::types::parse_function_parameters, utils::strings::decode_hex};

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
    fn test_get_padding_no_padding() {
        // No padding, input contains no null bytes
        let input = "11".repeat(32);
        assert_eq!(get_padding(&decode_hex(&input).expect("failed to decode hex")), Padding::None);
    }

    #[test]
    fn test_get_padding_left_padding() {
        // Left padded, first byte is null
        let input = "00".repeat(31) + "11";
        assert_eq!(get_padding(&decode_hex(&input).expect("failed to decode hex")), Padding::Left);
    }

    #[test]
    fn test_get_padding_right_padding() {
        // Right padding, last byte is null
        let input = "11".to_owned() + &"00".repeat(31);
        assert_eq!(get_padding(&decode_hex(&input).expect("failed to decode hex")), Padding::Right);
    }

    #[test]
    fn test_get_padding_skewed_left_padding() {
        // Both left and right null-bytes, but still left padded
        let input = "00".repeat(30) + "1100";
        assert_eq!(get_padding(&decode_hex(&input).expect("failed to decode hex")), Padding::Left);
    }

    #[test]
    fn test_get_padding_skewed_right_padding() {
        // Both left and right null-bytes, but still right padded
        let input = "0011".to_owned() + &"00".repeat(30);
        assert_eq!(get_padding(&decode_hex(&input).expect("failed to decode hex")), Padding::Right);
    }

    #[test]
    fn test_get_padding_empty_input() {
        // Empty input should result in no padding
        let input = "";
        assert_eq!(get_padding(&decode_hex(input).expect("failed to decode hex")), Padding::None);
    }

    #[test]
    fn test_get_padding_single_byte() {
        // Single-byte input with null byte
        let input = "00";
        assert_eq!(get_padding(&decode_hex(input).expect("failed to decode hex")), Padding::None);
    }

    #[test]
    fn test_get_padding_single_byte_left_padding() {
        // Single-byte input with left padding
        let input = "0011";
        assert_eq!(get_padding(&decode_hex(input).expect("failed to decode hex")), Padding::Left);
    }

    #[test]
    fn test_get_padding_single_byte_right_padding() {
        // Single-byte input with right padding
        let input = "1100";
        assert_eq!(get_padding(&decode_hex(input).expect("failed to decode hex")), Padding::Right);
    }

    #[test]
    fn test_get_padding_single_byte_both_padding() {
        // Single-byte input with both left and right padding
        let input = "001100";
        assert_eq!(get_padding(&decode_hex(input).expect("failed to decode hex")), Padding::None);
    }

    #[test]
    fn test_get_padding_mixed_padding() {
        // Mixed padding, some null bytes in the middle
        let input = "00".repeat(10) + "1122330000332211" + &"00".repeat(10);
        assert_eq!(get_padding(&decode_hex(&input).expect("failed to decode hex")), Padding::None);
    }

    #[test]
    fn test_get_padding_mixed_padding_skewed_left() {
        // Mixed padding, some null bytes in the middle
        let input = "00".repeat(10) + "001122330000332211" + &"00".repeat(10);
        assert_eq!(get_padding(&decode_hex(&input).expect("failed to decode hex")), Padding::Left);
    }

    #[test]
    fn test_get_padding_mixed_padding_skewed_right() {
        // Mixed padding, some null bytes in the middle
        let input = "00".repeat(10) + "112233000033221100" + &"00".repeat(10);
        assert_eq!(get_padding(&decode_hex(&input).expect("failed to decode hex")), Padding::Right);
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
}
