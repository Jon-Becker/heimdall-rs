use hashbrown::HashSet;
use std::time::Instant;

use alloy::primitives::Selector;
use alloy_dyn_abi::{DynSolCall, DynSolReturns, DynSolType, DynSolValue};
use alloy_json_abi::Param;
use eyre::eyre;
use heimdall_common::{
    ether::{
        signatures::{
            cache_signatures_from_abi, score_signature, ResolveSelector, ResolvedFunction,
        },
        types::parse_function_parameters,
    },
    utils::{io::logging::TraceFactory, strings::encode_hex},
};
use heimdall_vm::core::types::{get_padding, get_potential_types_for_word, to_type, Padding};
use tracing::{debug, info, trace, warn};

use crate::{
    error::Error,
    interfaces::{DecodeArgs, DecodeResult},
    utils::{
        decode_multicall, format_multicall_trace, is_multicall_pattern, parse_deployment_bytecode,
        try_decode, try_decode_dynamic_parameter,
    },
};

/// Decodes EVM calldata into human-readable function signatures and parameters
///
/// This function attempts to identify the function being called based on the function
/// selector in the calldata, and then decodes the remaining data according to the
/// function's parameter types. If no matching function is found, it will attempt
/// to infer the parameter types from the raw calldata.
///
/// # Arguments
///
/// * `args` - Configuration parameters for the decode operation
///
/// # Returns
///
/// A DecodeResult containing the resolved function and its decoded parameters
pub async fn decode(mut args: DecodeArgs) -> Result<DecodeResult, Error> {
    let start_time = Instant::now();

    if args.explain && args.openrouter_api_key.is_empty() {
        return Err(Error::Eyre(
            eyre!("OpenRouter API key is required for explaining calldata. Use `heimdall decode --help` for more information.".to_string()),
        ));
    }

    if let Some(abi_path) = args.abi.as_ref() {
        cache_signatures_from_abi(abi_path.into())
            .map_err(|e| Error::Eyre(eyre!("caching signatures from ABI failed: {}", e)))?;
    }

    let start_fetch_time = Instant::now();
    let calldata = args
        .get_calldata()
        .await
        .map_err(|e| Error::FetchError(format!("fetching target calldata failed: {e}")))?;
    debug!("fetching target calldata took {:?}", start_fetch_time.elapsed());

    if calldata.is_empty() {
        return Err(Error::Eyre(eyre!("calldata is empty. is this a value transfer?")));
    }

    let (function_selector, mut byte_args) = if args.constructor {
        debug!("extracting constructor arguments from deployment bytecode.");
        warn!("the --constructor flag is in unstable, and will be improved in future releases.");
        let constructor = parse_deployment_bytecode(calldata)?;
        debug!(
            "parsed constructor argument hex string from deployment bytecode: '{}'",
            encode_hex(&constructor.arguments)
        );
        args.skip_resolving = true;
        ("00000000".to_string(), constructor.arguments)
    } else {
        if calldata.len() < 4 {
            return Err(Error::Eyre(eyre!("calldata must contain a four-byte function selector")));
        }
        (encode_hex(&calldata[..4]), calldata[4..].to_vec())
    };

    if byte_args.len() % 32 != 0 && !args.truncate_calldata {
        warn!("calldata is not a standard size. if decoding fails, consider using the `--truncate-calldata` flag.");
    } else if args.truncate_calldata {
        warn!("calldata is not a standard size. truncating the calldata to a standard size.");
        byte_args.truncate(byte_args.len() - byte_args.len() % 32);
    }

    let start_resolve_time = Instant::now();
    let potential_matches = if !args.skip_resolving {
        match ResolvedFunction::resolve(&function_selector).await {
            Ok(Some(signatures)) => signatures,
            _ => Vec::new(),
        }
    } else {
        Vec::new()
    };
    debug!("resolving potential matches took {:?}", start_resolve_time.elapsed());
    if !potential_matches.is_empty() {
        info!("resolved {} potential function signatures", potential_matches.len());
    }

    let decode_start_time = Instant::now();
    let mut matches = potential_matches
        .iter()
        .map(|potential_match| {
            let inputs = parse_function_parameters(&potential_match.signature)
                .map_err(|e| Error::Eyre(eyre!("parsing function parameters failed: {}", e)))?;
            let ty = DynSolCall::new(
                Selector::default(),
                inputs.to_vec(),
                None,
                DynSolReturns::new(Vec::new()),
            );

            if let Ok(result) = ty
                .abi_decode_input(&byte_args)
                .map_err(|e| Error::Eyre(eyre!("decoding calldata failed: {}", e)))
            {
                let mut found_match = potential_match.clone();
                found_match.decoded_inputs = Some(result);
                Ok(found_match)
            } else {
                debug!(
                    "potential match '{}' ignored. decoding types failed",
                    &potential_match.signature
                );
                Err(Error::Eyre(eyre!(
                    "potential match '{}' ignored. decoding types failed",
                    &potential_match.signature
                )))
            }
        })
        .filter_map(|result| result.ok())
        .collect::<Vec<ResolvedFunction>>();

    if matches.len() > 1 {
        debug!("multiple possible matches found. as of 0.8.0, heimdall uses a heuristic to select the best match.");
        let num_words = byte_args.chunks(32).len();

        matches.sort_by(|a, b| {
            let a_score = score_signature(&a.signature, Some(num_words));
            let b_score = score_signature(&b.signature, Some(num_words));
            b_score.cmp(&a_score)
        });
        for match_ in &matches {
            debug!(
                " > {}: {}",
                match_.signature,
                score_signature(&match_.signature, Some(num_words))
            );
        }
    } else if matches.is_empty() {
        warn!("couldn't find any resolved matches for '{}'", function_selector);

        info!("falling back to raw calldata decoding: https://jbecker.dev/research/decoding-raw-calldata");
        let (decoded_inputs, params) = infer_parameters(&byte_args)?;
        let resolved_function = ResolvedFunction {
            name: format!("Unresolved_{function_selector}"),
            signature: format!(
                "Unresolved_{}({})",
                function_selector,
                params.iter().map(|x| x.ty.to_string()).collect::<Vec<String>>().join(", ")
            ),
            inputs: params.iter().map(|x| x.ty.to_string()).collect::<Vec<String>>(),
            decoded_inputs: Some(decoded_inputs),
        };
        matches.push(resolved_function);
    }

    let selected_match = matches.first().expect("matches is empty").clone();
    debug!("decoding calldata took {:?}", decode_start_time.elapsed());
    info!("decoded {} argument bytes successfully", byte_args.len());

    let multicall_results = if let Some(decoded_inputs) = &selected_match.decoded_inputs {
        let mut multicall_decoded = None;

        for input in decoded_inputs {
            if is_multicall_pattern(input) {
                debug!("Detected multicall pattern");
                match decode_multicall(input, &args).await {
                    Ok(results) => {
                        info!("Successfully decoded {} multicall items", results.len());
                        multicall_decoded = Some(results);
                        break;
                    }
                    Err(e) => {
                        warn!("Failed to decode multicall: {:?}", e);
                    }
                }
            }
        }

        multicall_decoded
    } else {
        None
    };

    debug!("decoding took {:?}", start_time.elapsed());

    let mut trace = TraceFactory::try_from(&selected_match)?;
    if let Some(ref multicall_results) = multicall_results {
        let decode_call = 1; // The main decode call is always index 1
        format_multicall_trace(multicall_results, decode_call, &mut trace);
    }

    Ok(DecodeResult { decoded: selected_match, multicall_results, _trace: trace })
}

/// Decode ABI parameter bytes without requiring a function selector.
///
/// Types are inferred using the same heuristics as unresolved calldata.
pub fn decode_parameters(bytes: &[u8]) -> Result<Vec<DynSolValue>, Error> {
    infer_parameters(bytes).map(|(values, _)| values)
}

fn infer_parameters(bytes: &[u8]) -> Result<(Vec<DynSolValue>, Vec<Param>), Error> {
    let words = bytes.chunks(32).map(|word| word.to_vec()).collect::<Vec<_>>();
    let mut potential_inputs: Vec<DynSolType> = Vec::new();
    let mut covered_words = HashSet::new();
    let mut head_end = words.len();
    for (i, word) in words.iter().enumerate() {
        // Dynamic parameter tails follow the complete ABI head; do not reinterpret tail words as
        // new parameters, even if the input contains unused trailing data.
        if i >= head_end {
            break;
        }
        if covered_words.contains(&i) {
            continue;
        }
        if let Some(abi_encoded) = try_decode_dynamic_parameter(i, &words)? {
            if abi_encoded.coverages.iter().any(|index| *index >= words.len()) {
                return Err(Error::BoundsError);
            }
            if let Some(first_tail_word) =
                abi_encoded.coverages.iter().filter(|index| **index > i).min()
            {
                head_end = head_end.min(*first_tail_word);
            }
            potential_inputs.push(to_type(&abi_encoded.ty));
            covered_words.extend(abi_encoded.coverages);
        } else {
            let (_, mut potential_types) = get_potential_types_for_word(word);
            match get_padding(word) {
                Padding::Left => {
                    potential_types.retain(|t| t.starts_with("uint") || t.starts_with("address"))
                }
                _ => potential_types.retain(|t| t.starts_with("bytes") || t.starts_with("string")),
            }
            let potential_type =
                to_type(potential_types.first().expect("potential types is empty"));
            potential_inputs.push(potential_type);
            covered_words.insert(i);
        }
    }
    trace!(
        "potential parameter inputs, ({:?})",
        potential_inputs.iter().map(|x| x.to_string()).collect::<Vec<_>>()
    );
    try_decode(&potential_inputs, bytes)
        .map_err(|e| Error::Eyre(eyre!("dynamically decoding calldata failed: {}", e)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interfaces::DecodeArgsBuilder;

    #[test]
    fn dynamic_abi_payload_is_not_reinterpreted_as_parameters() {
        // ABI encoding of one uint256[] argument containing [1, 2].
        let mut bytes = vec![0_u8; 128];
        bytes[31] = 32;
        bytes[63] = 2;
        bytes[95] = 1;
        bytes[127] = 2;
        let expected = decode_parameters(&bytes).expect("valid array encoding");
        assert_eq!(expected.len(), 1);
        assert!(matches!(&expected[0], DynSolValue::Array(items) if items.len() == 2));

        // Solidity decoders permit data after the ABI value; it is not another argument.
        bytes.extend([0; 32]);
        assert_eq!(decode_parameters(&bytes).expect("trailing word is permitted"), expected);
    }

    #[test]
    fn reads_all_head_parameters_before_dynamic_tail() {
        // ABI encoding of (uint256[]([1, 2]), uint256(7)).
        let mut bytes = vec![0_u8; 160];
        bytes[31] = 64;
        bytes[63] = 7;
        bytes[95] = 2;
        bytes[127] = 1;
        bytes[159] = 2;
        let expected = decode_parameters(&bytes).expect("valid tuple encoding");
        assert_eq!(expected.len(), 2);
        assert!(matches!(&expected[0], DynSolValue::Array(items) if items.len() == 2));
        assert!(
            matches!(&expected[1], DynSolValue::Uint(value, _) if *value == alloy::primitives::U256::from(7))
        );
        bytes.extend([0; 32]);
        assert_eq!(decode_parameters(&bytes).expect("trailing word is permitted"), expected);
    }

    #[tokio::test]
    async fn rejects_calldata_shorter_than_a_selector() {
        for target in ["0x12", "0x1234", "0x123456"] {
            let args = DecodeArgsBuilder::new().target(target.to_owned()).build().unwrap();
            let error = decode(args).await.unwrap_err();
            assert!(error.to_string().contains("four-byte function selector"));
        }
    }
}
