use std::{collections::HashSet, time::Instant};

use alloy_dyn_abi::DynSolType;
use eyre::eyre;
use heimdall_common::{
    ether::{
        calldata::get_calldata_from_target,
        signatures::{score_signature, ResolveSelector, ResolvedFunction},
        types::parse_function_parameters,
    },
    utils::{io::logging::TraceFactory, strings::encode_hex},
};
use heimdall_vm::core::types::{get_padding, get_potential_types_for_word, to_type, Padding};
use tracing::{debug, info, trace, warn};

use crate::{
    error::Error,
    interfaces::DecodeArgs,
    utils::{parse_deployment_bytecode, try_decode, try_decode_dynamic_parameter},
};

#[derive(Debug, Clone)]
pub struct DecodeResult {
    pub decoded: ResolvedFunction,
    _trace: TraceFactory,
}

impl DecodeResult {
    pub fn display(&self) {
        self._trace.display();
    }
}

pub async fn decode(mut args: DecodeArgs) -> Result<DecodeResult, Error> {
    let start_time = Instant::now();

    // check if we require an OpenAI API key
    if args.explain && args.openai_api_key.is_empty() {
        return Err(Error::Eyre(
            eyre!("OpenAI API key is required for explaining calldata. Use `heimdall decode --help` for more information.".to_string()),
        ));
    }

    // get the bytecode from the target
    let start_fetch_time = Instant::now();
    let mut calldata = get_calldata_from_target(&args.target, &args.rpc_url)
        .await
        .map_err(|e| Error::FetchError(format!("fetching target calldata failed: {}", e)))?;
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
            let as_tuple = DynSolType::Tuple(inputs);

            if let Ok(result) = as_tuple
                .abi_decode(byte_args)
                .map_err(|e| Error::Eyre(eyre!("decoding calldata failed: {}", e)))
            {
                let mut found_match = potential_match.clone();
                found_match.decoded_inputs = result.as_tuple().map(|t| t.to_vec());
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

        if let Ok((decoded_inputs, params)) = try_decode(&potential_inputs, byte_args) {
            // build a ResolvedFunction to add to matches
            let resolved_function = ResolvedFunction {
                name: format!("Unresolved_{}", function_selector),
                signature: format!(
                    "Unresolved_{}({})",
                    function_selector,
                    params.iter().map(|x| x.ty.to_string()).collect::<Vec<String>>().join(", ")
                ),
                inputs: params.iter().map(|x| x.ty.to_string()).collect::<Vec<String>>(),
                decoded_inputs: Some(decoded_inputs),
            };

            matches.push(resolved_function);
        } else {
            return Err(Error::Eyre(eyre!("failed to dynamically decode calldata")));
        }
    }

    let selected_match = matches.first().expect("matches is empty").clone();
    debug!("decoding calldata took {:?}", decode_start_time.elapsed());
    info!("decoded {} bytes successfully", calldata.len());
    debug!("decoding took {:?}", start_time.elapsed());
    Ok(DecodeResult { _trace: TraceFactory::try_from(&selected_match)?, decoded: selected_match })
}
