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
/// A multicall is typically an array of tuples containing:
/// - address: target contract
/// - optional uint: value/amount (for payable multicalls)
/// - bytes: encoded function call data
pub(crate) fn is_multicall_pattern(value: &DynSolValue) -> bool {
    match value {
        DynSolValue::Array(items) | DynSolValue::FixedArray(items) => {
            if items.is_empty() {
                return false;
            }

            // Check if all items follow multicall pattern
            items.iter().all(|item| match item {
                DynSolValue::Tuple(tuple_items) => {
                    // Pattern 1: (address, bytes)
                    if tuple_items.len() == 2 {
                        matches!(&tuple_items[0], DynSolValue::Address(_)) &&
                            matches!(&tuple_items[1], DynSolValue::Bytes(_))
                    }
                    // Pattern 2: (address, uint, bytes)
                    else if tuple_items.len() == 3 {
                        matches!(&tuple_items[0], DynSolValue::Address(_)) &&
                            matches!(&tuple_items[1], DynSolValue::Uint(_, _)) &&
                            matches!(&tuple_items[2], DynSolValue::Bytes(_))
                    } else {
                        false
                    }
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
    let (target, value, calldata) = match tuple_items.len() {
        2 => {
            // (address, bytes)
            let target = match &tuple_items[0] {
                DynSolValue::Address(addr) => format!("{addr:?}"),
                _ => return Err(Error::Eyre(eyre!("Expected address in multicall tuple"))),
            };
            let calldata = match &tuple_items[1] {
                DynSolValue::Bytes(data) => data.clone(),
                _ => return Err(Error::Eyre(eyre!("Expected bytes in multicall tuple"))),
            };
            (target, None, calldata)
        }
        3 => {
            // (address, uint, bytes)
            let target = match &tuple_items[0] {
                DynSolValue::Address(addr) => format!("{addr:?}"),
                _ => return Err(Error::Eyre(eyre!("Expected address in multicall tuple"))),
            };
            let value = match &tuple_items[1] {
                DynSolValue::Uint(val, _) => Some(val.to_string()),
                _ => return Err(Error::Eyre(eyre!("Expected uint in multicall tuple"))),
            };
            let calldata = match &tuple_items[2] {
                DynSolValue::Bytes(data) => data.clone(),
                _ => return Err(Error::Eyre(eyre!("Expected bytes in multicall tuple"))),
            };
            (target, value, calldata)
        }
        _ => return Err(Error::Eyre(eyre!("Unexpected multicall tuple length"))),
    };

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
    trace_factory.add_message(parent_trace, line!(), vec!["Multicall detected:".to_string()]);

    for result in multicall_results {
        let call_desc = if let Some(value) = &result.value {
            format!("├─ [{}] target: {} value: {} wei", result.index, result.target, value)
        } else {
            format!("├─ [{}] target: {}", result.index, result.target)
        };

        trace_factory.add_message(parent_trace, line!(), vec![call_desc]);

        if let Some(decoded) = &result.decoded {
            // Add the decoded function signature
            trace_factory.add_message(
                parent_trace,
                line!(),
                vec![format!("│    └─ {}", decoded.decoded.signature)],
            );

            // Add decoded inputs
            if let Some(inputs) = &decoded.decoded.decoded_inputs {
                if inputs.is_empty() {
                    // Show that there are no parameters
                    trace_factory.add_message(
                        parent_trace,
                        line!(),
                        vec!["│         (no parameters)".to_string()],
                    );
                } else {
                    for (i, input) in inputs.iter().enumerate() {
                        let mut formatted_inputs = display(vec![input.clone()], "│           ");
                        if !formatted_inputs.is_empty() {
                            // Format the first line with input index
                            formatted_inputs[0] = format!(
                                "│         input {}: {}",
                                i,
                                formatted_inputs[0].trim_start_matches("│           ")
                            );

                            // Adjust subsequent lines for proper indentation
                            for j in 1..formatted_inputs.len() {
                                formatted_inputs[j] = formatted_inputs[j]
                                    .replace("│           ", "│                ");
                            }

                            for line in formatted_inputs {
                                trace_factory.add_message(parent_trace, line!(), vec![line]);
                            }
                        } else {
                            // Handle case where display returns empty (e.g., for empty bytes)
                            match input {
                                DynSolValue::Bytes(b) if b.is_empty() => {
                                    trace_factory.add_message(
                                        parent_trace,
                                        line!(),
                                        vec![format!("│         input {}: bytes: 0x (empty)", i)],
                                    );
                                }
                                DynSolValue::String(s) if s.is_empty() => {
                                    trace_factory.add_message(
                                        parent_trace,
                                        line!(),
                                        vec![format!(
                                            "│         input {}: string: \"\" (empty)",
                                            i
                                        )],
                                    );
                                }
                                _ => {
                                    // Fallback for other empty types
                                    trace_factory.add_message(
                                        parent_trace,
                                        line!(),
                                        vec![format!("│         input {}: (empty)", i)],
                                    );
                                }
                            }
                        }
                    }
                }
            }
        } else {
            // Show raw calldata if decoding failed
            trace_factory.add_message(
                parent_trace,
                line!(),
                vec![format!("│    └─ Raw calldata: 0x{}", encode_hex(&result.calldata))],
            );
        }
    }
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
}
