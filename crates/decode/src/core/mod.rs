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
        infer_tail_ending_at, is_multicall_pattern, offset_in_words, parse_deployment_bytecode,
        try_decode,
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

/// Recover disjoint tails in head order (canonical ABI), or in physical order
/// when packed payloads corroborate pointers to a smaller, reordered tuple.
fn infer_raw_inputs(words: &[&[u8]]) -> Vec<DynSolType> {
    let offsets = words
        .iter()
        .enumerate()
        .filter_map(|(i, word)| {
            let offset = offset_in_words(word)?;
            (i < offset && offset < words.len()).then_some((i, offset))
        })
        .collect::<Vec<_>>();

    if offsets.is_empty() {
        return words.iter().map(|word| infer_scalar_type(word)).collect();
    }
    let canonical = ordered_inputs(words, &offsets)
        .unwrap_or_else(|prefix| recover_inputs(words, &offsets, prefix));
    let mut head_end = words.len();
    let mut head_pointers = 0;
    for &(i, offset) in &offsets {
        if i >= head_end {
            break;
        }
        head_end = head_end.min(offset);
        head_pointers += 1;
    }
    let head = &offsets[..head_pointers];
    if head.windows(2).any(|w| w[0].1 > w[1].1) {
        // An offset-looking scalar may split a valid canonical tail. Prefer a
        // complete ordered interpretation of this head whenever one exists.
        if let Some(inputs) = reordered_inputs(words, head, head_end, &canonical) {
            let first = head
                .iter()
                .position(|&(_, offset)| offset == head_end)
                .expect("the first physical tail is referenced in the head");
            return ordered_inputs(words, &head[first..])
                .ok()
                .filter(|v| v.len() == head_end)
                .unwrap_or(inputs);
        }
    }
    canonical
}

fn ordered_inputs(words: &[&[u8]], offsets: &[(usize, usize)]) -> Result<Vec<DynSolType>, usize> {
    let mut inputs = Vec::new();
    let mut head_end = words.len();
    let mut next_tail = None;
    let mut offsets = offsets.iter().copied().peekable();
    while let Some((i, offset)) = offsets.next() {
        if i >= head_end {
            break;
        }
        if next_tail.is_some_and(|end| end != offset) {
            continue;
        }
        let inferred = offsets
            .peek()
            .filter(|&&(j, end)| j < head_end.min(offset) && offset < end)
            .and_then(|&(_, end)| {
                infer_dynamic_tail(offset, words, end).filter(|p| p.tail.end == end)
            })
            .or_else(|| infer_dynamic_tail(offset, words, words.len()));
        if let Some(param) = inferred {
            head_end = head_end.min(offset);
            next_tail = Some(param.tail.end);
            inputs.extend(words[inputs.len()..i].iter().map(|w| infer_scalar_type(w)));
            inputs.push(param.ty);
        }
    }
    if next_tail != Some(words.len()) {
        return Err(next_tail.unwrap_or(words.len()));
    }
    inputs.extend(words[inputs.len()..head_end].iter().map(|w| infer_scalar_type(w)));
    Ok(inputs)
}

fn reordered_inputs(
    words: &[&[u8]],
    head: &[(usize, usize)],
    head_end: usize,
    canonical: &[DynSolType],
) -> Option<Vec<DynSolType>> {
    if canonical.len() <= head_end || canonical[head_end..].iter().any(DynSolType::is_dynamic) {
        return None;
    }
    // Index by physical offset so each tail is scanned once. Duplicate head
    // pointers cannot identify disjoint dynamic arguments.
    let mut owners = vec![None; words.len()];
    for &(i, offset) in head {
        if owners[offset].replace(i).is_some() {
            return None;
        }
    }
    let mut end = words.len();
    let mut types = Vec::with_capacity(head.len());
    let mut packed_tails = 0;
    for (start, owner) in owners.into_iter().enumerate().rev() {
        let Some(i) = owner else { continue };
        let param = infer_dynamic_tail(start, words, end)?;
        if param.tail.end != end {
            return None;
        }
        // Integer or offset words can accidentally form an array tail. Require
        // independent packed payloads before changing an ordered interpretation.
        if param.has_packed_data(words) {
            packed_tails += 1;
        } else if !canonical[i].is_dynamic() {
            return None;
        }
        types.push((i, param.ty));
        end = start;
    }
    (packed_tails >= 2).then(|| types_in_head(words, end, types))
}

/// Several signatures can describe the same words. Prefer a complete layout
/// from the prefix reached by the forward pass. If it fails,
/// the real pointer can be anywhere in the head, including beyond that prefix.
fn recover_inputs(words: &[&[u8]], offsets: &[(usize, usize)], prefix: usize) -> Vec<DynSolType> {
    let stop = offsets.partition_point(|&(i, _)| i < prefix);
    let mut remaining = words.len();
    let previous = inputs_from_end(words, &offsets[..stop], prefix, |start, end| {
        let span = end - start;
        if span > remaining {
            return None;
        }
        let param = infer_dynamic_tail(start, words, end)?;
        if param.tail.end == end {
            return Some(param.ty);
        }
        remaining -= span;
        None
    });
    if let Some(inputs) = previous {
        return inputs;
    }

    // An unsuccessful reordered interpretation must leave the canonical
    // allowance available for the actual tuple tails.
    let mut canonical_budget = words.len();
    let mut reordered_budget = words.len();
    inputs_from_end(words, offsets, words.len(), |start, end| {
        infer_tail_ending_at(words, start, end, &mut canonical_budget, &mut reordered_budget)
            .map(|p| p.ty)
    })
    .unwrap_or_else(|| words.iter().map(|w| infer_scalar_type(w)).collect())
}

fn inputs_from_end(
    words: &[&[u8]],
    offsets: &[(usize, usize)],
    head_limit: usize,
    mut probe: impl FnMut(usize, usize) -> Option<DynSolType>,
) -> Option<Vec<DynSolType>> {
    let mut end = words.len();
    let mut tails = Vec::new();
    for &(i, start) in offsets.iter().rev() {
        // All accepted pointers must remain in the tuple head.
        let rightmost_head = tails.first().map_or(i, |&(index, _)| index);
        if start <= rightmost_head || start >= end {
            continue;
        }
        if let Some(ty) = probe(start, end) {
            tails.push((i, ty));
            end = start;
        }
    }
    (end <= head_limit && !tails.is_empty()).then(|| types_in_head(words, end, tails))
}

fn types_in_head(words: &[&[u8]], end: usize, tails: Vec<(usize, DynSolType)>) -> Vec<DynSolType> {
    let mut inputs = words[..end].iter().map(|w| infer_scalar_type(w)).collect::<Vec<_>>();
    for (i, ty) in tails {
        inputs[i] = ty;
    }
    inputs
}

#[cfg(test)]
mod tests {
    use alloy::primitives::{address, B256};
    use alloy_dyn_abi::DynSolValue;
    use heimdall_common::utils::strings::encode_hex;

    use super::decode;
    use crate::DecodeArgsBuilder;

    async fn decode_raw(arguments: &DynSolValue) -> Vec<DynSolValue> {
        decode_raw_bytes(&arguments.abi_encode_params()).await
    }

    async fn decode_raw_bytes(encoded: &[u8]) -> Vec<DynSolValue> {
        let target = format!("0xdeadbeef{}", encode_hex(encoded));
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

    fn fixed_bytes(bytes: &[u8]) -> DynSolValue {
        DynSolValue::FixedBytes(B256::right_padding_from(bytes), bytes.len())
    }

    async fn assert_raw_array_in_both_orders(first: DynSolValue, second: DynSolValue) {
        for rows in [vec![first.clone(), second.clone()], vec![second, first]] {
            let array = DynSolValue::Array(rows);
            assert_eq!(decode_raw(&array).await, [array]);
        }
    }

    #[tokio::test]
    async fn raw_decode_keeps_array_tails_next_to_offset_like_scalars() {
        let fixed = DynSolValue::Array(vec![fixed_bytes(b"AB"), fixed_bytes(b"CD")]);
        let integers = DynSolValue::Array([31u16, 256, 257].map(DynSolValue::from).to_vec());
        for arguments in [
            vec![fixed.clone(), DynSolValue::from(128u8)],
            vec![DynSolValue::from(96u8), integers.clone()],
            vec![DynSolValue::from(32u8), integers],
            vec![fixed, DynSolValue::from(160u8), DynSolValue::Array(vec![DynSolValue::from(9u8)])],
        ] {
            assert_eq!(decode_raw(&DynSolValue::Tuple(arguments.clone())).await, arguments);
        }
    }

    #[tokio::test]
    async fn raw_decode_finds_the_full_head_after_a_false_offset() {
        let array = DynSolValue::Array(vec![DynSolValue::from(1u8); 2]);
        let marker = fixed_bytes(&[0xfe, 0xec]);
        for args in [
            vec![32u8.into(), 6u8.into(), marker.clone(), array.clone()],
            vec![64u8.into(), array, 6u8.into(), marker, 7u8.into()],
        ] {
            assert_eq!(decode_raw(&DynSolValue::Tuple(args.clone())).await, args);
        }
    }

    #[tokio::test]
    async fn raw_decode_finds_an_array_after_rejecting_a_partial_packed_tail() {
        let values = [31u16, 256, 257, 258].map(DynSolValue::from).to_vec();
        let args = vec![128u8.into(), DynSolValue::Array(values), 128u8.into()];
        assert_eq!(decode_raw(&DynSolValue::Tuple(args.clone())).await, args);
    }

    #[tokio::test]
    async fn raw_decode_preserves_a_canonical_tuple_with_an_apparent_reordered_tail() {
        let array = DynSolValue::Array(vec![0u8.into(), 1u8.into()]);
        let marker = fixed_bytes(&[0xbe, 0xef]);
        for args in [
            vec![array.clone(), 352u16.into(), 96u8.into(), 32u8.into(), 128u8.into()],
            vec![marker.clone(), array, 352u16.into(), 128u8.into(), 2u8.into(), marker],
        ] {
            assert_eq!(decode_raw(&DynSolValue::Tuple(args.clone())).await, args);
        }
    }

    // Alloy emits tails in head order. Move just the head pointers so the same
    // disjoint tails occur in the specified physical order.
    async fn assert_bytes_in_physical_order(values: &[DynSolValue], order: &[usize]) {
        let physical = order.iter().map(|&i| values[i].clone()).collect();
        let mut encoded = DynSolValue::Tuple(physical).abi_encode_params();
        let head = encoded[..32 * values.len()].to_vec();
        for (&index, offset) in order.iter().zip(head.chunks_exact(32)) {
            encoded[32 * index..32 * (index + 1)].copy_from_slice(offset);
        }
        assert_eq!(decode_raw_bytes(&encoded).await, values);
    }

    #[tokio::test]
    async fn raw_decode_reads_disjoint_tails_in_argument_order() {
        let text = [
            DynSolValue::Bytes(b"hello world!".to_vec()),
            DynSolValue::Bytes(b"customer billing address".to_vec()),
        ];
        assert_bytes_in_physical_order(&text, &[1, 0]).await;

        let binary = [
            DynSolValue::Bytes(vec![0xff, 0xfe]),
            DynSolValue::Bytes(vec![0x81; 40]),
            DynSolValue::Bytes(vec![0x80; 65]),
        ];
        for order in [[0, 2, 1], [1, 0, 2], [1, 2, 0], [2, 0, 1], [2, 1, 0]] {
            assert_bytes_in_physical_order(&binary, &order).await;
        }
    }

    #[tokio::test]
    async fn raw_decode_reads_disjoint_array_tails_in_element_order() {
        for values in [
            [vec![0xca, 0xfe, 0xfe], vec![0x81; 40], vec![0x80; 65]],
            [vec![], vec![0xff, 0], vec![0xfe, 0]],
        ] {
            let values = values.map(DynSolValue::Bytes);
            for order in [[0, 2, 1], [1, 0, 2], [1, 2, 0], [2, 0, 1], [2, 1, 0]] {
                let physical =
                    DynSolValue::Array(order.iter().map(|&i| values[i].clone()).collect());
                let mut encoded = physical.abi_encode_params();
                // Array offsets are relative to the first word after its length.
                let offsets = &mut encoded[64..64 + 32 * order.len()];
                let sorted = offsets.to_vec();
                for (&index, offset) in order.iter().zip(sorted.chunks_exact(32)) {
                    offsets[32 * index..32 * (index + 1)].copy_from_slice(offset);
                }
                assert_eq!(decode_raw_bytes(&encoded).await, [DynSolValue::Array(values.to_vec())]);
            }
        }
    }

    #[tokio::test]
    async fn raw_decode_keeps_reordered_offset_values_in_a_flat_array() {
        let values = DynSolValue::Array([160u8, 96, 160, 128].map(DynSolValue::from).to_vec());
        let args = vec![values, DynSolValue::Bytes(vec![0xab; 33])];
        assert_eq!(decode_raw(&DynSolValue::Tuple(args.clone())).await, args);
    }

    #[tokio::test]
    async fn raw_decode_preserves_canonical_byte_arrays_after_a_failed_reordered_probe() {
        let array = DynSolValue::Array(vec![
            DynSolValue::Bytes(vec![0xaa, 0xbb]),
            DynSolValue::Bytes(vec![0xcc, 0xee]),
        ]);
        let args = vec![480u16.into(), 480u16.into(), array.clone(), array, 160u8.into()];
        assert_eq!(decode_raw(&DynSolValue::Tuple(args.clone())).await, args);
    }

    #[tokio::test]
    async fn raw_decode_preserves_addresses_with_trailing_zeros() {
        let addresses = [
            address!("0000000000000000000000000000000000000001"),
            address!("1111111111111111111111111111111111111111"),
            address!("4200000000000000000000000000000000000000"),
        ]
        .map(DynSolValue::Address);
        let flat = DynSolValue::Array(addresses.to_vec());
        assert_eq!(decode_raw(&flat).await, [flat]);
        assert_raw_array_in_both_orders(
            DynSolValue::Array(addresses[..2].to_vec()),
            DynSolValue::Array(addresses[2..].to_vec()),
        )
        .await;
    }

    #[tokio::test]
    async fn raw_decode_keeps_scalar_width_across_differently_padded_array_elements() {
        use alloy::primitives::U256;

        let mut low_byte = [0; 20];
        low_byte[19] = 0xff;
        for [a, b] in [
            [
                DynSolValue::Uint(U256::from(1) << 160, 216),
                DynSolValue::Uint((U256::from(1) << 216) - U256::from(1), 216),
            ],
            [fixed_bytes(&low_byte), fixed_bytes(&[0xff; 20])],
        ] {
            assert_raw_array_in_both_orders(a.clone(), b.clone()).await;
            assert_raw_array_in_both_orders(
                DynSolValue::Array(vec![a]),
                DynSolValue::Array(vec![b]),
            )
            .await;
        }

        // Neither a one-byte integer nor one fixed byte can hold both words.
        let mut low = [0; 32];
        low[31] = 0xff;
        let mut high = [0; 32];
        high[0] = 0xff;
        assert_raw_array_in_both_orders(fixed_bytes(&low), fixed_bytes(&high)).await;

        // One member establishes the padding direction, another the full width.
        let mut prefix = [0; 20];
        prefix[..7].fill(0xee);
        let mut middle = [0xaa; 20];
        middle[..3].fill(0);
        assert_raw_array_in_both_orders(
            DynSolValue::Array(vec![fixed_bytes(&low_byte)]),
            DynSolValue::Array(vec![fixed_bytes(&prefix), fixed_bytes(&middle)]),
        )
        .await;
    }

    #[tokio::test]
    async fn raw_decode_pads_a_short_final_array_element_before_inferring_its_width() {
        let array = DynSolValue::Array(vec![fixed_bytes(&[0xab, 0xcd]), fixed_bytes(&[0, 0xff])]);
        let mut encoded = array.abi_encode_params();
        encoded.truncate(encoded.len() - 30);
        assert_eq!(decode_raw_bytes(&encoded).await, [array]);
    }

    #[tokio::test]
    async fn raw_decode_keeps_nonzero_prefix_bytes_when_a_sibling_looks_like_an_address() {
        let mut address_word = [0; 32];
        address_word[12..].fill(0x11);
        let mut wide_word = [0; 32];
        wide_word[0] = 0x42;
        assert_raw_array_in_both_orders(fixed_bytes(&address_word), fixed_bytes(&wide_word)).await;
    }

    #[tokio::test]
    async fn raw_decode_recognizes_whitespace_in_strings() {
        for text in ["line\nbreak", "tab\there", "line\r\nbreak"] {
            assert_raw_array_in_both_orders(
                DynSolValue::String("ok".into()),
                DynSolValue::String(text.into()),
            )
            .await;
        }
    }

    #[tokio::test]
    async fn raw_decode_keeps_short_text_at_a_confirmed_tuple_boundary() {
        for (text, n) in [("ok".to_owned(), 2u8), ("x".repeat(33), 33u8)] {
            let args = vec![
                DynSolValue::String(text),
                DynSolValue::Array((1..=n).map(DynSolValue::from).collect()),
            ];
            assert_eq!(decode_raw(&DynSolValue::Tuple(args.clone())).await, args);
        }
    }

    #[tokio::test]
    async fn raw_decode_checks_the_next_tuple_boundary_for_long_text() {
        let numbers = DynSolValue::Array(vec![DynSolValue::from(7u8); 257]);
        for size in [255, 256, 257] {
            let args = vec![DynSolValue::String("x".repeat(size)), numbers.clone()];
            assert_eq!(decode_raw(&DynSolValue::Tuple(args.clone())).await, args);
        }
        let args =
            vec![DynSolValue::String("x".repeat(256)), DynSolValue::Array(vec![]), numbers.clone()];
        assert_eq!(decode_raw(&DynSolValue::Tuple(args.clone())).await, args);
        for byte in [0xff, 0x01] {
            let mut binary = vec![b'x'; 256];
            binary.push(byte);
            let args = vec![DynSolValue::Bytes(binary), numbers.clone()];
            assert_eq!(decode_raw(&DynSolValue::Tuple(args.clone())).await, args);
        }
    }

    #[tokio::test]
    async fn raw_decode_recognizes_arrays_of_multiword_strings() {
        let text = format!("{} note", "a".repeat(28));
        let array = DynSolValue::Array(vec![DynSolValue::String(text.clone()); 12]);
        assert_eq!(decode_raw(&array).await, [array]);

        let mut invalid = text.as_bytes().to_vec();
        invalid[32] = 0xff;
        assert_raw_array_in_both_orders(
            DynSolValue::Bytes(text.into_bytes()),
            DynSolValue::Bytes(invalid),
        )
        .await;
    }

    #[tokio::test]
    async fn raw_decode_uses_sibling_width_for_zero_rows() {
        assert_raw_array_in_both_orders(
            DynSolValue::Array(vec![DynSolValue::from(0u8)]),
            DynSolValue::Array(vec![DynSolValue::from(1u8)]),
        )
        .await;
    }

    #[tokio::test]
    async fn raw_decode_preserves_full_words_across_rows_with_different_padding() {
        let mut one = [0; 32];
        one[31] = 1;
        assert_raw_array_in_both_orders(
            DynSolValue::Array(vec![fixed_bytes(&one)]),
            DynSolValue::Array(vec![fixed_bytes(&[0xab; 32])]),
        )
        .await;
    }

    #[tokio::test]
    async fn raw_decode_uses_siblings_to_distinguish_one_byte_payloads_from_arrays() {
        assert_raw_array_in_both_orders(
            DynSolValue::Bytes(vec![0xff]),
            DynSolValue::Bytes(vec![0xff, 0xfe]),
        )
        .await;
        assert_raw_array_in_both_orders(
            DynSolValue::Array(vec![fixed_bytes(b"A")]),
            DynSolValue::Array(vec![fixed_bytes(b"B"), fixed_bytes(b"C")]),
        )
        .await;
    }

    #[tokio::test]
    async fn raw_decode_preserves_multiword_text_with_a_short_text_sibling() {
        for long in ["x".repeat(40), format!("{}€", "x".repeat(31))] {
            assert_raw_array_in_both_orders(
                DynSolValue::String("ok".into()),
                DynSolValue::String(long),
            )
            .await;
        }
    }

    #[tokio::test]
    async fn raw_decode_preserves_binary_data_with_a_text_sibling() {
        let mut binary = vec![b'x'; 32];
        binary.push(0xff);
        for bytes in [binary, vec![0x12, 0x01]] {
            assert_raw_array_in_both_orders(
                DynSolValue::Bytes(b"ok".to_vec()),
                DynSolValue::Bytes(bytes),
            )
            .await;
        }
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
