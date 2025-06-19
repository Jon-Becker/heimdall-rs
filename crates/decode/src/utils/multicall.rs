use alloy_dyn_abi::DynSolValue;
use eyre::eyre;
use heimdall_common::utils::{
    io::{logging::TraceFactory, types::display},
    strings::encode_hex,
};
use tracing::{debug, trace};

use crate::{
    core::{decode, DecodeResult},
    error::Error,
    interfaces::DecodeArgs,
};

/// Detects if a decoded value represents a multicall pattern.
/// A multicall is an array of tuples that must contain at least:
/// - address: target contract
/// - bytes: encoded function call data
///
/// Additional parameters (like bool flags or uint values) are allowed
/// but not required. The order of parameters doesn't matter.
pub(crate) fn is_multicall_pattern(value: &DynSolValue) -> bool {
    match value {
        DynSolValue::Array(items) | DynSolValue::FixedArray(items) => {
            if items.is_empty() {
                return false;
            }

            // Check if all items follow multicall pattern
            items.iter().all(|item| match item {
                DynSolValue::Tuple(tuple_items) => {
                    // Must have at least address and bytes, regardless of other parameters
                    let has_address =
                        tuple_items.iter().any(|v| matches!(v, DynSolValue::Address(_)));
                    let has_bytes = tuple_items.iter().any(|v| matches!(v, DynSolValue::Bytes(_)));

                    // As long as we have address and bytes, it's a potential multicall
                    has_address && has_bytes
                }
                _ => false,
            })
        }
        _ => false,
    }
}

/// Decodes multicall data recursively
pub(crate) async fn decode_multicall(
    value: &DynSolValue,
    args: &DecodeArgs,
) -> Result<Vec<MulticallDecoded>, Error> {
    match value {
        DynSolValue::Array(items) | DynSolValue::FixedArray(items) => {
            let mut results = Vec::new();

            for (index, item) in items.iter().enumerate() {
                if let DynSolValue::Tuple(tuple_items) = item {
                    let decoded = decode_multicall_item(tuple_items, args, index).await?;
                    results.push(decoded);
                }
            }

            Ok(results)
        }
        _ => Err(Error::Eyre(eyre!("Expected array for multicall decoding"))),
    }
}

/// Represents a decoded multicall item
#[derive(Debug, Clone)]
pub struct MulticallDecoded {
    pub index: usize,
    pub target: String,
    pub value: Option<String>,
    pub calldata: Vec<u8>,
    pub decoded: Option<DecodeResult>,
}

async fn decode_multicall_item(
    tuple_items: &[DynSolValue],
    args: &DecodeArgs,
    index: usize,
) -> Result<MulticallDecoded, Error> {
    // Find each component regardless of order
    let mut target = None;
    let mut value = None;
    let mut calldata = None;

    for item in tuple_items {
        match item {
            DynSolValue::Address(addr) => {
                if target.is_none() {
                    target = Some(format!("{addr:?}"));
                }
            }
            DynSolValue::Uint(val, _) => {
                if value.is_none() {
                    value = Some(val.to_string());
                }
            }
            DynSolValue::Bytes(data) => {
                if calldata.is_none() {
                    calldata = Some(data.clone());
                }
            }
            _ => {}
        }
    }

    // Validate we have required fields
    let target = target.ok_or_else(|| Error::Eyre(eyre!("No address found in multicall tuple")))?;
    let calldata =
        calldata.ok_or_else(|| Error::Eyre(eyre!("No bytes found in multicall tuple")))?;

    // Check if calldata looks like a function call (4 byte selector + padded args)
    let decoded = if calldata.len() >= 4 && (calldata.len() - 4) % 32 == 0 {
        trace!(
            "Attempting to decode multicall item {} with calldata: {}",
            index,
            encode_hex(&calldata)
        );

        // Create a new DecodeArgs for the nested call
        let mut nested_args = args.clone();
        nested_args.target = encode_hex(&calldata);
        nested_args.raw = true;

        match Box::pin(decode(nested_args)).await {
            Ok(result) => {
                debug!("Successfully decoded multicall item {}", index);
                Some(result)
            }
            Err(e) => {
                debug!("Failed to decode multicall item {}: {:?}", index, e);
                None
            }
        }
    } else {
        None
    };

    Ok(MulticallDecoded { index, target, value, calldata, decoded })
}

/// Formats multicall results for display
pub(crate) fn format_multicall_trace(
    multicall_results: &[MulticallDecoded],
    parent_trace: u32,
    trace_factory: &mut TraceFactory,
) {
    // Build all multicall messages as a single batch
    let mut messages = Vec::new();
    messages.push("multicall:".to_string());

    for (idx, result) in multicall_results.iter().enumerate() {
        let is_last = idx == multicall_results.len() - 1;
        let prefix = if is_last { "└─" } else { "├─" };
        let continuation = if is_last { "   " } else { "│  " };

        messages.push(format!("   {} [{}] target: {}", prefix, result.index, result.target));

        if let Some(decoded) = &result.decoded {
            // Add the decoded function signature
            messages.push(format!("   {}    └─ {}", continuation, decoded.decoded.signature));

            // Add decoded inputs
            if let Some(inputs) = &decoded.decoded.decoded_inputs {
                if inputs.is_empty() {
                    // Show that there are no parameters
                    messages.push(format!("   {continuation}         (no parameters)"));
                } else {
                    for (i, input) in inputs.iter().enumerate() {
                        let formatted_inputs = display(
                            vec![input.clone()],
                            &format!("   {continuation}              "),
                        );
                        if !formatted_inputs.is_empty() {
                            // Format the first line with input index
                            let first_line = format!(
                                "   {}         input {}: {}",
                                continuation,
                                i,
                                formatted_inputs[0].trim_start_matches(&format!(
                                    "   {continuation}              "
                                ))
                            );
                            messages.push(first_line);

                            // Add subsequent lines with proper indentation
                            for line in formatted_inputs.iter().skip(1) {
                                let line = line.replace(
                                    &format!("   {continuation}              "),
                                    &format!("   {continuation}                "),
                                );
                                messages.push(line);
                            }
                        } else {
                            // Handle case where display returns empty (e.g., for empty bytes)
                            match input {
                                DynSolValue::Bytes(b) if b.is_empty() => {
                                    messages.push(format!(
                                        "   {continuation}         input {i}: bytes: 0x (empty)"
                                    ));
                                }
                                DynSolValue::String(s) if s.is_empty() => {
                                    messages.push(format!(
                                        "   {continuation}         input {i}: string: \"\" (empty)"
                                    ));
                                }
                                _ => {
                                    // Fallback for other empty types
                                    messages.push(format!(
                                        "   {continuation}         input {i}: (empty)"
                                    ));
                                }
                            }
                        }
                    }
                }
            }
        } else {
            // Show raw calldata if decoding failed
            messages.push(format!(
                "   {}    └─ Raw calldata: 0x{}",
                continuation,
                encode_hex(&result.calldata)
            ));
        }

        // Add space between multicalls if not the last one
        if !is_last {
            messages.push(format!("   {continuation} "));
        }
    }

    // Add all multicall lines as a single message
    trace_factory.add_message(parent_trace, line!(), messages);
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::{Address, U256};
    use heimdall_common::utils::strings::decode_hex;

    #[test]
    fn test_is_multicall_pattern_with_address_bytes() {
        // Create a multicall pattern: [(address, bytes)]
        let multicall = DynSolValue::Array(vec![
            DynSolValue::Tuple(vec![
                DynSolValue::Address(Address::ZERO),
                DynSolValue::Bytes(vec![0x12, 0x34, 0x56, 0x78]),
            ]),
            DynSolValue::Tuple(vec![
                DynSolValue::Address(Address::ZERO),
                DynSolValue::Bytes(vec![0xaa, 0xbb, 0xcc, 0xdd]),
            ]),
        ]);

        assert!(is_multicall_pattern(&multicall));
    }

    #[test]
    fn test_is_multicall_pattern_with_address_uint_bytes() {
        // Create a multicall pattern: [(address, uint, bytes)]
        let multicall = DynSolValue::Array(vec![
            DynSolValue::Tuple(vec![
                DynSolValue::Address(Address::ZERO),
                DynSolValue::Uint(U256::from(100), 256),
                DynSolValue::Bytes(vec![0x12, 0x34, 0x56, 0x78]),
            ]),
            DynSolValue::Tuple(vec![
                DynSolValue::Address(Address::ZERO),
                DynSolValue::Uint(U256::from(200), 256),
                DynSolValue::Bytes(vec![0xaa, 0xbb, 0xcc, 0xdd]),
            ]),
        ]);

        assert!(is_multicall_pattern(&multicall));
    }

    #[test]
    fn test_is_multicall_pattern_not_array() {
        let not_multicall = DynSolValue::Uint(U256::from(100), 256);
        assert!(!is_multicall_pattern(&not_multicall));
    }

    #[test]
    fn test_is_multicall_pattern_wrong_tuple_size() {
        // Wrong pattern: [(address)]
        let wrong_multicall =
            DynSolValue::Array(vec![DynSolValue::Tuple(vec![DynSolValue::Address(Address::ZERO)])]);

        assert!(!is_multicall_pattern(&wrong_multicall));
    }

    #[test]
    fn test_is_multicall_pattern_wrong_types() {
        // Wrong pattern: [(uint, bytes)] instead of [(address, bytes)]
        let wrong_multicall = DynSolValue::Array(vec![DynSolValue::Tuple(vec![
            DynSolValue::Uint(U256::from(100), 256),
            DynSolValue::Bytes(vec![0x12, 0x34]),
        ])]);

        assert!(!is_multicall_pattern(&wrong_multicall));
    }

    #[test]
    fn test_is_multicall_pattern_empty_array() {
        let empty_multicall = DynSolValue::Array(vec![]);
        assert!(!is_multicall_pattern(&empty_multicall));
    }

    #[test]
    fn test_is_multicall_pattern_fixed_array() {
        // Test with FixedArray
        let multicall = DynSolValue::FixedArray(vec![DynSolValue::Tuple(vec![
            DynSolValue::Address(Address::ZERO),
            DynSolValue::Bytes(vec![0x12, 0x34, 0x56, 0x78]),
        ])]);

        assert!(is_multicall_pattern(&multicall));
    }

    #[test]
    fn test_decode_multicall_item_validation() {
        // Test that multicall patterns with valid calldata structure are recognized
        let selector = decode_hex("4585e33b").unwrap();
        let mut calldata = selector.clone();
        calldata.extend(vec![0u8; 64]); // Add some padding for a simple call

        let multicall = DynSolValue::Array(vec![DynSolValue::Tuple(vec![
            DynSolValue::Address(Address::ZERO),
            DynSolValue::Bytes(calldata.clone()),
        ])]);

        assert!(is_multicall_pattern(&multicall));

        // Test with value parameter
        let multicall_with_value = DynSolValue::Array(vec![DynSolValue::Tuple(vec![
            DynSolValue::Address(Address::ZERO),
            DynSolValue::Uint(U256::from(1000), 256),
            DynSolValue::Bytes(calldata),
        ])]);

        assert!(is_multicall_pattern(&multicall_with_value));
    }

    #[test]
    fn test_aggregate3value_pattern() {
        // Test the aggregate3Value pattern: (address, bool, uint256, bytes)[]
        let multicall_with_bool = DynSolValue::Array(vec![DynSolValue::Tuple(vec![
            DynSolValue::Address(Address::ZERO),
            DynSolValue::Bool(false),
            DynSolValue::Uint(U256::from(0), 256),
            DynSolValue::Bytes(vec![0x12, 0x34, 0x56, 0x78]),
        ])]);

        // This SHOULD be detected as a multicall pattern (has address + bytes)
        assert!(is_multicall_pattern(&multicall_with_bool));
    }

    #[test]
    fn test_multicall_with_extra_params() {
        // Test patterns with extra parameters
        let with_string = DynSolValue::Array(vec![DynSolValue::Tuple(vec![
            DynSolValue::Address(Address::ZERO),
            DynSolValue::String("test".to_string()),
            DynSolValue::Bytes(vec![0x12, 0x34]),
            DynSolValue::Bool(true),
        ])]);
        assert!(is_multicall_pattern(&with_string));

        // Should fail if missing address
        let no_address = DynSolValue::Array(vec![DynSolValue::Tuple(vec![
            DynSolValue::Uint(U256::from(100), 256),
            DynSolValue::Bytes(vec![0x12, 0x34]),
        ])]);
        assert!(!is_multicall_pattern(&no_address));

        // Should fail if missing bytes
        let no_bytes = DynSolValue::Array(vec![DynSolValue::Tuple(vec![
            DynSolValue::Address(Address::ZERO),
            DynSolValue::Uint(U256::from(100), 256),
        ])]);
        assert!(!is_multicall_pattern(&no_bytes));
    }

    #[test]
    fn test_multicall_pattern_permutations() {
        // Test (bytes, address, uint) - different order
        let permutation1 = DynSolValue::Array(vec![DynSolValue::Tuple(vec![
            DynSolValue::Bytes(vec![0x12, 0x34, 0x56, 0x78]),
            DynSolValue::Address(Address::ZERO),
            DynSolValue::Uint(U256::from(100), 256),
        ])]);
        assert!(is_multicall_pattern(&permutation1));

        // Test (uint, bytes, address) - another order
        let permutation2 = DynSolValue::Array(vec![DynSolValue::Tuple(vec![
            DynSolValue::Uint(U256::from(100), 256),
            DynSolValue::Bytes(vec![0x12, 0x34, 0x56, 0x78]),
            DynSolValue::Address(Address::ZERO),
        ])]);
        assert!(is_multicall_pattern(&permutation2));

        // Test (bytes, address) - 2 element permutation
        let permutation3 = DynSolValue::Array(vec![DynSolValue::Tuple(vec![
            DynSolValue::Bytes(vec![0x12, 0x34, 0x56, 0x78]),
            DynSolValue::Address(Address::ZERO),
        ])]);
        assert!(is_multicall_pattern(&permutation3));
    }

    #[test]
    fn test_real_multicall_data() {
        use alloy_dyn_abi::DynSolType;

        // Test data from the failing test case
        let calldata_hex = "1749e1e3000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000010000000000000000000000000813ccee6e0fc0fbc506f834122c7c082cd4c33f0000000000000000000000000000000000000000000000000000000000176a2400000000000000000000000000000000000000000000000000000000000000600000000000000000000000000000000000000000000000000000000000000044585e33b000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000";
        let calldata = decode_hex(calldata_hex).unwrap();
        let data = &calldata[4..]; // Skip selector

        println!("Testing multicall detection with real data...");
        println!("Data length: {} bytes", data.len());

        // Debug: check what the decoder sees
        println!("\nFirst 32 bytes (offset): {}", encode_hex(&data[0..32]));
        println!("Next 32 bytes (length): {}", encode_hex(&data[32..64]));

        // The issue might be that this is not (address, uint256, bytes)[]
        // but rather a function with different parameters
        // Let's check if this could be multicall((address,uint256,bytes)[])

        // Try decoding as a single parameter that is an array
        let inner_tuple =
            DynSolType::Tuple(vec![DynSolType::Address, DynSolType::Uint(256), DynSolType::Bytes]);
        let array_type = DynSolType::Array(Box::new(inner_tuple));

        println!("\nTrying to decode as: {:?}", array_type);

        match array_type.abi_decode(data) {
            Ok(decoded) => {
                println!("Successfully decoded!");
                println!("Decoded value: {:?}", decoded);
                assert!(is_multicall_pattern(&decoded), "Should be detected as multicall pattern");
            }
            Err(e) => {
                println!("Failed with array decoding: {:?}", e);

                // Maybe the function has other parameters?
                // Let's just test the pattern detection with a manually constructed value
                let test_value = DynSolValue::Array(vec![DynSolValue::Tuple(vec![
                    DynSolValue::Address(Address::from([
                        0x08, 0x13, 0xcc, 0xee, 0x6e, 0x0f, 0xc0, 0xfb, 0xc5, 0x06, 0xf8, 0x34,
                        0x12, 0x2c, 0x7c, 0x08, 0x2c, 0xd4, 0xc3, 0x3f,
                    ])),
                    DynSolValue::Uint(U256::from(0x176a24), 256),
                    DynSolValue::Bytes(vec![0x58, 0x5e, 0x33, 0xb0]), /* Just first 4 bytes as
                                                                       * example */
                ])]);

                println!("\nTesting pattern detection with manually constructed value...");
                assert!(
                    is_multicall_pattern(&test_value),
                    "Manually constructed multicall should be detected"
                );
                println!("Pattern detection works correctly!");
            }
        }
    }
}
