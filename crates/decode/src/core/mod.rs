use std::time::Instant;

use alloy::primitives::Selector;
use alloy_dyn_abi::{DynSolCall, DynSolReturns, DynSolType};
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
use tracing::{debug, info, trace, warn};

use crate::{
    error::Error,
    interfaces::{DecodeArgs, DecodeResult},
    utils::{
        decode_multicall, format_multicall_trace, infer_dynamic_tail, infer_scalar_type,
        is_multicall_pattern, offset_in_words, parse_deployment_bytecode, try_decode,
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

    // check if we require an OpenRouter API key
    if args.explain && args.openrouter_api_key.is_empty() {
        return Err(Error::Eyre(
            eyre!("OpenRouter API key is required for explaining calldata. Use `heimdall decode --help` for more information.".to_string()),
        ));
    }

    // parse and cache signatures from the ABI, if provided
    if let Some(abi_path) = args.abi.as_ref() {
        cache_signatures_from_abi(abi_path.into())
            .map_err(|e| Error::Eyre(eyre!("caching signatures from ABI failed: {}", e)))?;
    }

    // get the bytecode from the target
    let start_fetch_time = Instant::now();
    let mut calldata = args
        .get_calldata()
        .await
        .map_err(|e| Error::FetchError(format!("fetching target calldata failed: {e}")))?;
    debug!("fetching target calldata took {:?}", start_fetch_time.elapsed());

    if calldata.is_empty() {
        return Err(Error::Eyre(eyre!("calldata is empty. is this a value transfer?")));
    }

    // if args.constructor is true, we need to extract the constructor arguments and use that
    // as the calldata
    if args.constructor {
        debug!("extracting constructor arguments from deployment bytecode.");
        warn!("the --constructor flag is in unstable, and will be improved in future releases.");
        let constructor = parse_deployment_bytecode(calldata)?;

        debug!(
            "parsed constructor argument hex string from deployment bytecode: '{}'",
            encode_hex(&constructor.arguments)
        );

        // prefix with four zero bytes to avoid selector issues
        calldata =
            [0x00, 0x00, 0x00, 0x00].into_iter().chain(constructor.arguments.into_iter()).collect();

        // ensure we dont resolve signatures, this is a constructor not calldata
        args.skip_resolving = true;
    }

    // if the calldata isnt a standard size, i.e. (len - 4) % 32 != 0, we should warn the user
    // and/or truncate it
    if (calldata[4..].len() % 32 != 0) && !args.truncate_calldata {
        warn!("calldata is not a standard size. if decoding fails, consider using the `--truncate-calldata` flag.");
    } else if args.truncate_calldata {
        warn!("calldata is not a standard size. truncating the calldata to a standard size.");

        // truncate calldata to a standard size
        let selector = calldata[0..4].to_owned();
        let args = calldata[4..][..calldata[4..].len() - (calldata[4..].len() % 32)].to_owned();
        calldata = [selector, args].concat();
    }

    // parse the two parts of calldata, inputs and selector
    let function_selector = encode_hex(&calldata[0..4]);
    let byte_args = &calldata[4..];

    // get the function signature possibilities
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

    // iterate over potential matches and attempt to decode the calldata with them
    let decode_start_time = Instant::now();
    let mut matches = potential_matches
        .iter()
        .map(|potential_match| {
            // decode the signature into Vec<DynSolType>
            let inputs = parse_function_parameters(&potential_match.signature)
                .map_err(|e| Error::Eyre(eyre!("parsing function parameters failed: {}", e)))?;
            let ty = DynSolCall::new(
                Selector::default(),
                inputs.to_vec(),
                None,
                DynSolReturns::new(Vec::new()),
            );

            if let Ok(result) = ty
                .abi_decode_input(byte_args)
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
        let num_words = calldata[4..].chunks(32).len();

        matches.sort_by(|a, b| {
            let a_score = score_signature(&a.signature, Some(num_words));
            let b_score = score_signature(&b.signature, Some(num_words));
            b_score.cmp(&a_score)
        });
        // debug print
        for match_ in &matches {
            debug!(
                " > {}: {}",
                match_.signature,
                score_signature(&match_.signature, Some(num_words))
            );
        }
    } else if matches.is_empty() {
        warn!("couldn't find any resolved matches for '{}'", function_selector);

        if matches.is_empty() {
            info!("falling back to raw calldata decoding: https://jbecker.dev/research/decoding-raw-calldata");

            let calldata_words = byte_args.chunks(32).collect::<Vec<_>>();
            let potential_inputs = infer_raw_inputs(&calldata_words);
            trace!(
                "potential parameter inputs, ({:?})",
                potential_inputs.iter().map(|x| x.to_string()).collect::<Vec<String>>()
            );

            let (decoded_inputs, params) = try_decode(&potential_inputs, &calldata[4..])
                .map_err(|e| Error::Eyre(eyre!("dynamically decoding calldata failed: {}", e)))?;
            // build a ResolvedFunction to add to matches
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
        } // End of raw decoding
    }

    let selected_match = matches.first().expect("matches is empty").clone();
    debug!("decoding calldata took {:?}", decode_start_time.elapsed());
    info!("decoded {} bytes successfully", calldata.len());

    // Check for multicall pattern
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

    // Create trace factory with multicall support
    let mut trace = TraceFactory::try_from(&selected_match)?;
    if let Some(ref multicall_results) = multicall_results {
        // Add multicall results to trace
        let decode_call = 1; // The main decode call is always index 1
        format_multicall_trace(multicall_results, decode_call, &mut trace);
    }

    Ok(DecodeResult { decoded: selected_match, multicall_results, _trace: trace })
}

/// Alloy reads one word per inferred parameter from the start of the tuple.
/// Only accept a dynamic layout if its tails account for every word after the head.
fn infer_raw_inputs(words: &[&[u8]]) -> Vec<DynSolType> {
    let mut inputs = Vec::new();
    let mut head_end = words.len();
    let mut next_tail = None;

    let mut offsets = words
        .iter()
        .enumerate()
        .filter_map(|(i, word)| {
            let offset = offset_in_words(word)?;
            (i < offset && offset < words.len()).then_some((i, offset))
        })
        .peekable();
    while let Some((i, offset)) = offsets.next() {
        if i >= head_end {
            break;
        }
        if next_tail.is_some_and(|expected| offset != expected) {
            continue;
        }

        // The next head offset can disambiguate packed bytes from a scalar array.
        // A scalar can look like an offset too, so only use that boundary on an exact fit.
        let inferred = offsets
            .peek()
            .filter(|&&(j, end)| j < head_end.min(offset) && offset < end)
            .and_then(|&(_, end)| {
                infer_dynamic_tail(offset, &words[..end]).filter(|param| param.tail.end == end)
            })
            .or_else(|| infer_dynamic_tail(offset, words));
        if let Some(parameter) = inferred {
            head_end = head_end.min(offset);
            next_tail = Some(parameter.tail.end);
            inputs.extend(words[inputs.len()..i].iter().map(|word| infer_scalar_type(word)));
            inputs.push(parameter.ty);
        }
    }

    if next_tail.is_some_and(|end| end != words.len()) {
        inputs.clear();
        head_end = words.len();
    }
    inputs.extend(words[inputs.len()..head_end].iter().map(|word| infer_scalar_type(word)));
    inputs
}

#[cfg(test)]
mod tests {
    use alloy_dyn_abi::DynSolValue;
    use heimdall_common::utils::strings::encode_hex;

    use super::decode;
    use crate::DecodeArgsBuilder;

    async fn decode_raw(arguments: &DynSolValue) -> Vec<DynSolValue> {
        let target = format!("0xdeadbeef{}", encode_hex(&arguments.abi_encode_params()));
        let args = DecodeArgsBuilder::new()
            .target(target)
            .skip_resolving(true)
            .raw(true)
            .build()
            .expect("valid decode arguments");
        decode(args)
            .await
            .expect("raw decode failed")
            .decoded
            .decoded_inputs
            .expect("decoded inputs")
    }

    fn nested_array(mut value: DynSolValue, depth: usize) -> DynSolValue {
        for _ in 0..depth {
            value = DynSolValue::Array(vec![value]);
        }
        value
    }

    #[tokio::test]
    async fn raw_decode_preserves_scalar_positions_after_an_ambiguous_offset() {
        let values = [32u8, 1, 7, 42].map(DynSolValue::from).to_vec();
        assert_eq!(decode_raw(&DynSolValue::Tuple(values.clone())).await, values);
    }

    #[tokio::test]
    async fn raw_decode_preserves_wide_scalar_arrays() {
        for first in [0u8, 1] {
            let array = DynSolValue::Array((first..first + 9).map(DynSolValue::from).collect());
            assert_eq!(decode_raw(&array).await, [array]);
        }
    }

    #[tokio::test]
    async fn raw_decode_keeps_multiple_tails_in_head_order() {
        let mut values = (0u16..8).map(DynSolValue::from).collect::<Vec<_>>();
        values.push(DynSolValue::from(300u16));
        let inputs = vec![
            DynSolValue::Array(values),
            DynSolValue::from(17u8),
            DynSolValue::Bytes(vec![0x41; 32]),
        ];
        assert_eq!(decode_raw(&DynSolValue::Tuple(inputs.clone())).await, inputs);
    }

    #[tokio::test]
    async fn raw_decode_preserves_leaves_at_the_array_depth_limit() {
        for leaf in [
            DynSolValue::from(7u8),
            DynSolValue::Bytes(vec![0x41]),
            DynSolValue::Bytes(vec![0x41; 32]),
        ] {
            let array = nested_array(leaf, 4);
            assert_eq!(decode_raw(&array).await, [array]);
        }
    }

    #[tokio::test]
    async fn raw_decode_falls_back_to_words_beyond_the_array_depth_limit() {
        let array = nested_array(DynSolValue::from(7u8), 5);
        // Five singleton arrays encode five offset/length pairs followed by the value.
        let words = [32u8, 1, 32, 1, 32, 1, 32, 1, 32, 1, 7].map(DynSolValue::from);
        assert_eq!(decode_raw(&array).await, words);
    }

    #[tokio::test]
    async fn raw_decode_finishes_on_repeated_offsets() {
        for offset in [32u8, 64] {
            let words = vec![DynSolValue::from(offset); 48];
            assert_eq!(decode_raw(&DynSolValue::Tuple(words.clone())).await, words);
        }
    }
}
