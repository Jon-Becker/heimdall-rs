use hashbrown::HashSet;
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
use heimdall_vm::core::types::{get_padding, get_potential_types_for_word, to_type, Padding};
use tracing::{debug, info, trace, warn};

use crate::{
    error::Error,
    interfaces::DecodeArgs,
    utils::{
        decode_multicall, format_multicall_trace, is_multicall_pattern, parse_deployment_bytecode,
        try_decode, try_decode_dynamic_parameter,
    },
};

#[derive(Debug, Clone)]
/// Result of a successful decode operation
///
/// Contains the decoded function signature and parameters, as well as
/// a trace factory for displaying the result in a formatted way.
pub struct DecodeResult {
    /// The resolved function with its decoded inputs
    pub decoded: ResolvedFunction,
    /// Multicall results if detected
    pub multicall_results: Option<Vec<crate::utils::MulticallDecoded>>,
    _trace: TraceFactory,
}

impl DecodeResult {
    /// Displays the decoded function signature and parameters in a formatted way
    pub fn display(&self) {
        self._trace.display();
    }

    /// Converts the decode result to JSON, including multicall results if present
    pub fn to_json(&self) -> Result<String, Error> {
        use heimdall_common::ether::types::DynSolValueExt;
        use serde_json::json;

        let mut result = json!({
            "name": self.decoded.name,
            "signature": self.decoded.signature,
            "inputs": self.decoded.inputs,
            "decoded_inputs": if let Some(decoded_inputs) = &self.decoded.decoded_inputs {
                decoded_inputs
                    .iter()
                    .map(|input| input.serialize())
                    .collect::<Vec<_>>()
            } else {
                vec![]
            }
        });

        // Add multicall results if present
        if let Some(multicall_results) = &self.multicall_results {
            let mut multicalls = vec![];

            for mc_result in multicall_results {
                let mut mc_json = json!({
                    "index": mc_result.index,
                    "target": mc_result.target,
                    "value": mc_result.value,
                    "calldata": format!("0x{}", encode_hex(&mc_result.calldata)),
                });

                // Add decoded result if available
                if let Some(decoded) = &mc_result.decoded {
                    mc_json["decoded"] = json!({
                        "name": decoded.decoded.name,
                        "signature": decoded.decoded.signature,
                        "inputs": decoded.decoded.inputs,
                        "decoded_inputs": if let Some(decoded_inputs) = &decoded.decoded.decoded_inputs {
                            decoded_inputs
                                .iter()
                                .map(|input| input.serialize())
                                .collect::<Vec<_>>()
                        } else {
                            vec![]
                        }
                    });
                }

                multicalls.push(mc_json);
            }

            result["multicall_results"] = json!(multicalls);
        }

        serde_json::to_string_pretty(&result)
            .map_err(|e| Error::Eyre(eyre::eyre!("Failed to serialize to JSON: {}", e)))
    }
}

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

    // check if we require an OpenAI API key
    if args.explain && args.openai_api_key.is_empty() {
        return Err(Error::Eyre(
            eyre!("OpenAI API key is required for explaining calldata. Use `heimdall decode --help` for more information.".to_string()),
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

        // Check if this is a known multicall selector that we should handle specially
        let is_known_multicall = matches!(
            function_selector.as_str(),
            "1749e1e3" | // multicall((address,uint256,bytes)[])
            "252dba42" | // aggregate((address,bytes)[])
            "82ad56cb" | // aggregate3((address,bool,bytes)[])
            "174dea71" // aggregate3Value((address,bool,uint256,bytes)[])
        );

        if is_known_multicall {
            info!(
                "detected known multicall selector '{}', using specialized decoding",
                function_selector
            );

            // Create the appropriate signature based on selector
            let signature = match function_selector.as_str() {
                "1749e1e3" => "multicall((address,uint256,bytes)[])",
                "252dba42" => "aggregate((address,bytes)[])",
                "82ad56cb" => "aggregate3((address,bool,bytes)[])",
                "174dea71" => "aggregate3Value((address,bool,uint256,bytes)[])",
                _ => unreachable!(),
            };

            // Parse and decode with the known signature
            let inputs = parse_function_parameters(signature)
                .map_err(|e| Error::Eyre(eyre!("parsing multicall parameters failed: {}", e)))?;
            let ty = DynSolCall::new(
                Selector::default(),
                inputs.to_vec(),
                None,
                DynSolReturns::new(Vec::new()),
            );

            match ty.abi_decode_input(byte_args) {
                Ok(result) => {
                    matches.push(ResolvedFunction {
                        name: signature.split('(').next().unwrap_or("unknown").to_string(),
                        signature: signature.to_string(),
                        inputs: inputs.iter().map(|ty| ty.to_string()).collect(),
                        decoded_inputs: Some(result),
                    });
                }
                Err(_) => {
                    // Fall back to raw decoding if specialized decoding fails
                    info!("specialized multicall decoding failed, falling back to raw decoding");
                }
            }
        }

        if matches.is_empty() {
            info!("falling back to raw calldata decoding: https://jbecker.dev/research/decoding-raw-calldata");

            // we're going to build a Vec<DynSolType> of all possible types for each
            let mut potential_inputs: Vec<DynSolType> = Vec::new();

            // chunk in blocks of 32 bytes
            let calldata_words = calldata[4..].chunks(32).map(|x| x.to_owned()).collect::<Vec<_>>();

            // while calldata_words is not empty, iterate over itcar
            let mut i = 0;
            let mut covered_words = HashSet::new();
            while covered_words.len() != calldata_words.len() {
                let word = calldata_words[i].to_owned();

                // check if the first word is abiencoded
                if let Some(abi_encoded) = try_decode_dynamic_parameter(i, &calldata_words)? {
                    let potential_type = to_type(&abi_encoded.ty);
                    potential_inputs.push(potential_type);
                    covered_words.extend(abi_encoded.coverages);
                } else {
                    let (_, mut potential_types) = get_potential_types_for_word(&word);

                    // perform heuristics
                    // - if we use right-padding, this is probably bytesN
                    // - if we use left-padding, this is probably uintN or intN
                    // - if we use no padding, this is probably bytes32
                    match get_padding(&word) {
                        Padding::Left => potential_types
                            .retain(|t| t.starts_with("uint") || t.starts_with("address")),
                        _ => potential_types
                            .retain(|t| t.starts_with("bytes") || t.starts_with("string")),
                    }

                    let potential_type =
                        to_type(potential_types.first().expect("potential types is empty"));

                    potential_inputs.push(potential_type);
                    covered_words.insert(i);
                }

                i += 1;
            }

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
