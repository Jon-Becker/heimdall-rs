mod core;
mod util;

use std::{collections::HashSet, time::Duration};

use clap::{AppSettings, Parser};
use derive_builder::Builder;
use ethers::{
    abi::{decode as decode_abi, Param, ParamType, Token},
    types::Transaction,
};

use heimdall_common::{
    constants::{CALLDATA_REGEX, TRANSACTION_HASH_REGEX},
    ether::{
        evm::core::types::{
            get_padding, get_potential_types_for_word, parse_function_parameters, to_type, Padding,
        },
        rpc::get_transaction,
        signatures::{score_signature, ResolveSelector, ResolvedFunction},
    },
    info_spinner,
    utils::{
        io::{logging::TraceFactory, types::display},
        strings::{encode_hex, StringExt},
    },
};
use heimdall_config::parse_url_arg;

use indicatif::ProgressBar;
use tracing::{debug, error, info, trace, warn};

use crate::{
    decode::{core::abi::try_decode_dynamic_parameter, util::get_explanation},
    error::Error,
};

#[derive(Debug, Clone, Parser, Builder)]
#[clap(
    about = "Decode calldata into readable types",
    after_help = "For more information, read the wiki: https://jbecker.dev/r/heimdall-rs/wiki",
    global_setting = AppSettings::DeriveDisplayOrder,
    override_usage = "heimdall decode <TARGET> [OPTIONS]"
)]
pub struct DecodeArgs {
    /// The target to decode, either a transaction hash or string of bytes.
    #[clap(required = true)]
    pub target: String,

    /// Set the output verbosity level, 1 - 5.

    /// The RPC provider to use for fetching target calldata.
    /// This can be an explicit URL or a reference to a MESC endpoint.
    #[clap(long, short, parse(try_from_str = parse_url_arg), default_value = "", hide_default_value = true)]
    pub rpc_url: String,

    /// Your OpenAI API key, used for explaining calldata.
    #[clap(long, short, default_value = "", hide_default_value = true)]
    pub openai_api_key: String,

    /// Whether to explain the decoded calldata using OpenAI.
    #[clap(long)]
    pub explain: bool,

    /// When prompted, always select the default value.
    #[clap(long, short)]
    pub default: bool,

    /// Whether to truncate nonstandard sized calldata.
    #[clap(long, short)]
    pub truncate_calldata: bool,

    /// Whether to skip resolving selectors. Heimdall will attempt to guess types.
    #[clap(long = "skip-resolving")]
    pub skip_resolving: bool,
}

impl DecodeArgsBuilder {
    pub fn new() -> Self {
        Self {
            target: Some(String::new()),
            rpc_url: Some(String::new()),
            openai_api_key: Some(String::new()),
            explain: Some(false),
            default: Some(true),
            truncate_calldata: Some(false),
            skip_resolving: Some(false),
        }
    }
}

/// The entrypoint for the decode module. This will attempt to decode the arguments of the target
/// calldata, without the ABI of the target contract.
#[allow(deprecated)]
pub async fn decode(args: DecodeArgs) -> Result<Vec<ResolvedFunction>, Error> {
    let mut trace = TraceFactory::default();

    // check if we require an OpenAI API key
    if args.explain && args.openai_api_key.is_empty() {
        return Err(Error::Generic(
            "OpenAI API key is required for explaining calldata. Use `heimdall decode --help` for more information.".to_string(),
        ));
    }

    // init variables
    let mut raw_transaction: Transaction = Transaction::default();
    let mut calldata;

    // determine whether or not the target is a transaction hash
    if TRANSACTION_HASH_REGEX.is_match(&args.target).unwrap_or(false) {
        // We are decoding a transaction hash, so we need to fetch the calldata from the RPC
        // provider.
        raw_transaction = get_transaction(&args.target, &args.rpc_url).await.map_err(|_| {
            Error::RpcError("failed to fetch transaction from RPC provider.".to_string())
        })?;

        calldata = raw_transaction.input.clone();
    } else if CALLDATA_REGEX.is_match(&args.target).unwrap_or(false) {
        // We are decoding raw calldata, so we can just use the provided calldata.
        calldata = args
            .target
            .parse()
            .map_err(|_| Error::Generic("failed to parse calldata from target.".to_string()))?;
    } else {
        return Err(Error::Generic(
            "invalid target. must be a transaction hash or calldata (bytes).".to_string(),
        ));
    }

    // if calldata isn't a multiple of 32, it may be harder to decode.
    if (calldata[4..].len() % 32 != 0) && !args.truncate_calldata {
        warn!("calldata is not a standard size. decoding may fail since each word is not exactly 32 bytes long.");
        warn!("if decoding fails, try using the --truncate-calldata flag to truncate the calldata to a standard size.");
    } else if args.truncate_calldata {
        warn!("calldata is not a standard size. truncating the calldata to a standard size.");

        // truncate calldata to a standard size
        let selector = calldata[0..4].to_owned();
        let args = calldata[4..][..calldata[4..].len() - (calldata[4..].len() % 32)].to_owned();
        calldata = [selector, args].concat().into();
    }

    // parse the two parts of calldata, inputs and selector
    let function_selector = encode_hex(calldata[0..4].to_owned());
    let byte_args = &calldata[4..];

    // get the function signature possibilities
    let potential_matches = if !args.skip_resolving {
        match ResolvedFunction::resolve(&function_selector).await {
            Ok(Some(signatures)) => signatures,
            _ => Vec::new(),
        }
    } else {
        Vec::new()
    };
    if potential_matches.is_empty() && !args.skip_resolving {
        warn!("couldn't resolve potential matches for the given function selector.");
    }

    let mut matches: Vec<ResolvedFunction> = Vec::new();
    for potential_match in &potential_matches {
        // convert the string inputs into a vector of decoded types
        let inputs: Vec<ParamType> = parse_function_parameters(&potential_match.signature)
            .map_err(|e| Error::Generic(format!("failed to parse function parameters: {}", e)))?;

        if let Ok(result) = decode_abi(&inputs, byte_args) {
            // convert tokens to params
            let mut params: Vec<Param> = Vec::new();
            for (i, input) in inputs.iter().enumerate() {
                params.push(Param {
                    name: format!("arg{i}"),
                    kind: input.to_owned(),
                    internal_type: None,
                });
            }

            let mut found_match = potential_match.clone();
            found_match.decoded_inputs = Some(result);
            matches.push(found_match);
        } else {
            debug!(
                "potential match '{}' ignored. decoding types failed",
                &potential_match.signature
            );
        }
    }

    if matches.is_empty() {
        warn!("couldn't find any matches for the given function selector.");
        // attempt to decode calldata regardless

        // we're going to build a Vec<ParamType> of all possible types for each
        let mut potential_inputs: Vec<ParamType> = Vec::new();

        // chunk in blocks of 32 bytes
        let calldata_words = calldata[4..].chunks(32).map(|x| x.to_owned()).collect::<Vec<_>>();

        // while calldata_words is not empty, iterate over it
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
                    params.iter().map(|x| x.kind.to_string()).collect::<Vec<String>>().join(", ")
                ),
                inputs: params.iter().map(|x| x.kind.to_string()).collect::<Vec<String>>(),
                decoded_inputs: Some(decoded_inputs),
            };

            matches.push(resolved_function);
        } else {
            error!("failed to dynamically decode calldata.");
            return Err(Error::DecodeError);
        }
    }

    // sort matches by signature using score heuristic from `score_signature`
    matches.sort_by(|a, b| {
        let a_score = score_signature(&a.signature);
        let b_score = score_signature(&b.signature);
        b_score.cmp(&a_score)
    });

    if matches.len() > 1 {
        warn!("multiple possible matches found. as of 0.8.0, heimdall uses a heuristic to select the best match.");
    }

    let selected_match = matches.first().expect("matches is empty").clone();

    let decode_call = trace.add_call(
        0,
        line!(),
        "heimdall".to_string(),
        "decode".to_string(),
        vec![args.target.truncate(64)],
        "()".to_string(),
    );
    trace.br(decode_call);
    trace.add_message(decode_call, line!(), vec![format!("name:      {}", selected_match.name)]);
    trace.add_message(
        decode_call,
        line!(),
        vec![format!("signature: {}", selected_match.signature)],
    );
    trace.add_message(decode_call, line!(), vec![format!("selector:  0x{function_selector}")]);
    trace.add_message(decode_call, line!(), vec![format!("calldata:  {} bytes", calldata.len())]);
    trace.br(decode_call);

    // build decoded string for --explain
    let decoded_string = &mut format!(
        "{}\n{}\n{}\n{}",
        format!("name: {}", selected_match.name),
        format!("signature: {}", selected_match.signature),
        format!("selector: 0x{function_selector}"),
        format!("calldata: {} bytes", calldata.len())
    );

    // build inputs
    for (i, input) in
        selected_match.decoded_inputs.as_ref().ok_or(Error::DecodeError)?.iter().enumerate()
    {
        let mut decoded_inputs_as_message = display(vec![input.to_owned()], "           ");
        if decoded_inputs_as_message.is_empty() {
            break;
        }

        if i == 0 {
            decoded_inputs_as_message[0] = format!(
                "input {}:{}{}",
                i,
                " ".repeat(4 - i.to_string().len()),
                decoded_inputs_as_message[0].replacen("           ", "", 1)
            )
        } else {
            decoded_inputs_as_message[0] = format!(
                "      {}:{}{}",
                i,
                " ".repeat(4 - i.to_string().len()),
                decoded_inputs_as_message[0].replacen("           ", "", 1)
            )
        }

        // add to trace and decoded string
        trace.add_message(decode_call, 1, decoded_inputs_as_message.clone());
        decoded_string.push_str(&format!("\n{}", decoded_inputs_as_message.clone().join("\n")));
    }

    // TODO: move to cli
    trace.display();

    if args.explain {
        // get a new progress bar
        let explain_progress = ProgressBar::new_spinner();
        explain_progress.enable_steady_tick(Duration::from_millis(100));
        explain_progress.set_style(info_spinner!());
        explain_progress.set_message("attempting to explain calldata...");

        match get_explanation(decoded_string.to_string(), raw_transaction, &args.openai_api_key)
            .await
        {
            Some(explanation) => {
                explain_progress.finish_and_clear();
                info!("Transaction explanation: {}", explanation.trim());
            }
            None => {
                explain_progress.finish_and_clear();
                error!("failed to get explanation from OpenAI.");
            }
        };
    }

    Ok(matches)
}

// Attempt to decode the given calldata with the given types.
fn try_decode(inputs: &[ParamType], byte_args: &[u8]) -> Result<(Vec<Token>, Vec<Param>), Error> {
    if let Ok(result) = decode_abi(inputs, byte_args) {
        // convert tokens to params
        let mut params: Vec<Param> = Vec::new();
        for (i, input) in inputs.iter().enumerate() {
            params.push(Param {
                name: format!("arg{i}"),
                kind: input.to_owned(),
                internal_type: None,
            });
        }

        return Ok((result, params));
    }

    Err(Error::DecodeError)
}
