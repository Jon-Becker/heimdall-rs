mod util;

use std::time::Duration;

use clap::{AppSettings, Parser};
use ethers::{
    abi::{decode as decode_abi, AbiEncode, Function, Param, ParamType, StateMutability},
    types::Transaction,
};

use heimdall_common::{
    constants::TRANSACTION_HASH_REGEX,
    ether::{
        evm::types::{display, parse_function_parameters},
        rpc::get_transaction,
        signatures::{score_signature, ResolveSelector, ResolvedFunction},
    },
    io::logging::Logger,
    utils::strings::decode_hex,
};

use indicatif::ProgressBar;
use strsim::normalized_damerau_levenshtein as similarity;

use crate::decode::util::get_explanation;

#[derive(Debug, Clone, Parser)]
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
    #[clap(flatten)]
    pub verbose: clap_verbosity_flag::Verbosity,

    /// The RPC provider to use for fetching target bytecode.
    #[clap(long = "rpc-url", short, default_value = "", hide_default_value = true)]
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
}

#[allow(deprecated)]
pub fn decode(args: DecodeArgs) {
    let (logger, mut trace) = Logger::new(match args.verbose.log_level() {
        Some(level) => level.as_str(),
        None => "SILENT",
    });
    let mut raw_transaction: Transaction = Transaction::default();
    let calldata;

    // check if we require an OpenAI API key
    if args.explain && args.openai_api_key.is_empty() {
        logger.error("OpenAI API key is required for explaining calldata. Use `heimdall decode --help` for more information.");
        std::process::exit(1);
    }

    // determine whether or not the target is a transaction hash
    if TRANSACTION_HASH_REGEX.is_match(&args.target).unwrap() {
        // We are decoding a transaction hash, so we need to fetch the calldata from the RPC
        // provider.
        raw_transaction = get_transaction(&args.target, &args.rpc_url, &logger);

        calldata = raw_transaction.input.to_string().replacen("0x", "", 1);
    } else {
        calldata = args.target.clone();
    }

    // check if calldata is present
    if calldata.is_empty() {
        logger.error(&format!("empty calldata found at '{}' .", &args.target));
        std::process::exit(1);
    }

    // normalize
    let calldata = calldata.replacen("0x", "", 1);

    // check if the calldata length is a standard length
    if calldata.len() % 2 != 0 {
        logger.error("calldata is not a valid hex string.");
        std::process::exit(1);
    }

    // if calldata isn't a multiple of 64, it may be harder to decode.
    if calldata[8..].len() % 64 != 0 {
        logger.warn("calldata is not a standard size. decoding may fail since each word is not exactly 32 bytes long.");
    }

    // parse the two parts of calldata, inputs and selector
    let function_selector = calldata[0..8].to_owned();
    let byte_args = match decode_hex(&calldata[8..]) {
        Ok(byte_args) => byte_args,
        Err(_) => {
            logger.error("failed to parse bytearray from calldata.");
            std::process::exit(1)
        }
    };

    // get the function signature possibilities
    let potential_matches = match ResolvedFunction::resolve(&function_selector) {
        Some(signatures) => signatures,
        None => Vec::new(),
    };
    let mut matches: Vec<ResolvedFunction> = Vec::new();

    for potential_match in &potential_matches {
        // convert the string inputs into a vector of decoded types
        let mut inputs: Vec<ParamType> = Vec::new();
        if let Some(type_) = parse_function_parameters(potential_match.signature.to_owned()) {
            for input in type_ {
                inputs.push(input);
            }
        }

        match decode_abi(&inputs, &byte_args) {
            Ok(result) => {
                // convert tokens to params
                let mut params: Vec<Param> = Vec::new();
                for (i, input) in inputs.iter().enumerate() {
                    params.push(Param {
                        name: format!("arg{i}"),
                        kind: input.to_owned(),
                        internal_type: None,
                    });
                }
                // build the decoded function to verify it's a match
                let decoded_function_call = Function {
                    name: potential_match.name.to_string(),
                    inputs: params,
                    outputs: Vec::new(),
                    constant: None,
                    state_mutability: StateMutability::NonPayable,
                }
                .encode_input(&result);
                match decoded_function_call {
                    Ok(decoded_function_call) => {
                        // decode the function call in trimmed bytes, removing 0s, because contracts
                        // can use nonstandard sized words and padding is
                        // hard
                        let cleaned_bytes = decoded_function_call.encode_hex().replace('0', "");
                        let decoded_function_call = match cleaned_bytes
                            .split_once(&function_selector.replace('0', ""))
                        {
                            Some(decoded_function_call) => decoded_function_call.1,
                            None => {
                                logger.debug(&format!("potential match '{}' ignored. decoded inputs differed from provided calldata.", &potential_match.signature).to_string());
                                continue
                            }
                        };

                        // if the decoded function call matches (95%) the function signature, add it
                        // to the list of matches
                        if similarity(decoded_function_call, &calldata[8..].replace('0', "")).abs() >=
                            0.90
                        {
                            let mut found_match = potential_match.clone();
                            found_match.decoded_inputs = Some(result);
                            matches.push(found_match);
                        } else {
                            logger.debug(&format!("potential match '{}' ignored. decoded inputs differed from provided calldata.", &potential_match.signature).to_string());
                        }
                    }
                    Err(_) => {
                        logger.debug(
                            &format!(
                                "potential match '{}' ignored. type checking failed",
                                &potential_match.signature
                            )
                            .to_string(),
                        );
                    }
                }
            }
            Err(_) => {
                logger.debug(
                    &format!(
                        "potential match '{}' ignored. decoding types failed",
                        &potential_match.signature
                    )
                    .to_string(),
                );
            }
        }
    }

    // truncate target for prettier display
    let mut shortened_target = args.target;
    if shortened_target.len() > 66 {
        shortened_target = shortened_target.chars().take(66).collect::<String>() +
            "..." +
            &shortened_target.chars().skip(shortened_target.len() - 16).collect::<String>();
    }

    if matches.is_empty() {
        logger.warn("couldn't find any matches for the given function signature.");

        // build a trace of the calldata
        let decode_call = trace.add_call(
            0,
            line!(),
            "heimdall".to_string(),
            "decode".to_string(),
            vec![shortened_target],
            "()".to_string(),
        );
        trace.br(decode_call);
        trace.add_message(
            decode_call,
            line!(),
            vec![format!(
                "selector: 0x{function_selector}{}",
                if function_selector == "00000000" { " (fallback?)" } else { "" },
            )],
        );
        trace.add_message(
            decode_call,
            line!(),
            vec![format!("calldata: {} bytes", calldata.len() / 2usize)],
        );
        trace.br(decode_call);

        // print out the decoded inputs
        let mut inputs: Vec<String> = Vec::new();
        for (i, input) in calldata[8..]
            .chars()
            .collect::<Vec<char>>()
            .chunks(64)
            .map(|c| c.iter().collect::<String>())
            .collect::<Vec<String>>()
            .iter()
            .enumerate()
        {
            inputs.push(
                format!(
                    "{} {}:{}{}",
                    if i == 0 { "input" } else { "     " },
                    i,
                    if i.to_string().len() <= 3 {
                        " ".repeat(3 - i.to_string().len())
                    } else {
                        "".to_string()
                    },
                    input
                )
                .to_string(),
            )
        }
        trace.add_message(decode_call, line!(), inputs);

        // force the trace to display
        trace.level = 4;
        trace.display();
    } else {
        let mut selection: u8 = 0;

        // sort matches by signature using score heuristic from `score_signature`
        matches.sort_by(|a, b| {
            let a_score = score_signature(&a.signature);
            let b_score = score_signature(&b.signature);
            b_score.cmp(&a_score)
        });

        if matches.len() > 1 {
            selection = logger.option(
                "warn",
                "multiple possible matches found. select an option below",
                matches.iter().map(|x| x.signature.clone()).collect(),
                Some(0u8),
                args.default,
            );
        }

        let selected_match = match matches.get(selection as usize) {
            Some(selected_match) => selected_match,
            None => {
                logger.error("invalid selection.");
                std::process::exit(1)
            }
        };

        let decode_call = trace.add_call(
            0,
            line!(),
            "heimdall".to_string(),
            "decode".to_string(),
            vec![shortened_target],
            "()".to_string(),
        );
        trace.br(decode_call);
        trace.add_message(
            decode_call,
            line!(),
            vec![format!("name:      {}", selected_match.name)],
        );
        trace.add_message(
            decode_call,
            line!(),
            vec![format!("signature: {}", selected_match.signature)],
        );
        trace.add_message(decode_call, line!(), vec![format!("selector:  0x{function_selector}")]);
        trace.add_message(
            decode_call,
            line!(),
            vec![format!("calldata:  {} bytes", calldata.len() / 2usize)],
        );
        trace.br(decode_call);

        // build decoded string for --explain
        let decoded_string = &mut format!(
            "{}\n{}\n{}\n{}",
            format!("name: {}", selected_match.name),
            format!("signature: {}", selected_match.signature),
            format!("selector: 0x{function_selector}"),
            format!("calldata: {} bytes", calldata.len() / 2usize)
        );

        // build inputs
        for (i, input) in selected_match.decoded_inputs.as_ref().unwrap().iter().enumerate() {
            let mut decoded_inputs_as_message = display(vec![input.to_owned()], "           ");
            if decoded_inputs_as_message.is_empty() {
                break
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

        // force the trace to display
        trace.level = 4;
        trace.display();

        if args.explain && !matches.is_empty() {
            // get a new progress bar
            let explain_progress = ProgressBar::new_spinner();
            explain_progress.enable_steady_tick(Duration::from_millis(100));
            explain_progress.set_style(logger.info_spinner());
            explain_progress.set_message("attempting to explain calldata...");

            match get_explanation(
                decoded_string.to_string(),
                raw_transaction,
                &args.openai_api_key,
                &logger,
            ) {
                Some(explanation) => {
                    explain_progress.finish_and_clear();
                    logger.success(&format!("Transaction explanation: {}", explanation.trim()));
                }
                None => {
                    explain_progress.finish_and_clear();
                    logger.error("failed to get explanation from OpenAI.");
                }
            };
        }
    }
}

/// Decode calldata into a Vec of potential ResolvedFunctions
/// ## Example
/// ```no_run
/// # use crate::heimdall::decode::decode_calldata;
/// const CALLDATA: &'static str = "0xd57e/* snip */2867"
///
/// let potential_matches = decode_calldata(CALLDATA.to_string());
#[allow(deprecated)]
#[allow(dead_code)]
pub fn decode_calldata(calldata: String) -> Option<Vec<ResolvedFunction>> {
    let (logger, _) = Logger::new("ERROR");

    // parse the two parts of calldata, inputs and selector
    let function_selector =
        calldata.replacen("0x", "", 1).get(0..8).unwrap_or("0x00000000").to_string();
    let byte_args = match decode_hex(&calldata[8..]) {
        Ok(byte_args) => byte_args,
        Err(_) => {
            logger.error("failed to parse bytearray from calldata.");
            std::process::exit(1)
        }
    };

    // get the function signature possibilities
    let potential_matches = match ResolvedFunction::resolve(&function_selector) {
        Some(signatures) => signatures,
        None => Vec::new(),
    };
    let mut matches: Vec<ResolvedFunction> = Vec::new();
    for potential_match in &potential_matches {
        // convert the string inputs into a vector of decoded types
        let mut inputs: Vec<ParamType> = Vec::new();

        if let Some(type_) = parse_function_parameters(potential_match.signature.to_owned()) {
            for input in type_ {
                inputs.push(input);
            }
        }

        match decode_abi(&inputs, &byte_args) {
            Ok(result) => {
                // convert tokens to params
                let mut params: Vec<Param> = Vec::new();
                for (i, input) in inputs.iter().enumerate() {
                    params.push(Param {
                        name: format!("arg{i}"),
                        kind: input.to_owned(),
                        internal_type: None,
                    });
                }
                // build the decoded function to verify it's a match
                let decoded_function_call = Function {
                    name: potential_match.name.to_string(),
                    inputs: params,
                    outputs: Vec::new(),
                    constant: None,
                    state_mutability: StateMutability::NonPayable,
                }
                .encode_input(&result);
                match decoded_function_call {
                    Ok(decoded_function_call) => {
                        // decode the function call in trimmed bytes, removing 0s, because contracts
                        // can use nonstandard sized words and padding is
                        // hard
                        let cleaned_bytes = decoded_function_call.encode_hex().replace('0', "");
                        let decoded_function_call = match cleaned_bytes
                            .split_once(&function_selector.replace('0', ""))
                        {
                            Some(decoded_function_call) => decoded_function_call.1,
                            None => {
                                logger.debug(&format!("potential match '{}' ignored. decoded inputs differed from provided calldata.", &potential_match.signature).to_string());
                                continue
                            }
                        };

                        // if the decoded function call matches (95%) the function signature, add it
                        // to the list of matches
                        if similarity(decoded_function_call, &calldata[8..].replace('0', "")).abs() >=
                            0.90
                        {
                            let mut found_match = potential_match.clone();
                            found_match.decoded_inputs = Some(result);
                            matches.push(found_match);
                        } else {
                            logger.debug(&format!("potential match '{}' ignored. decoded inputs differed from provided calldata.", &potential_match.signature).to_string());
                        }
                    }
                    Err(_) => {
                        logger.debug(
                            &format!(
                                "potential match '{}' ignored. type checking failed",
                                &potential_match.signature
                            )
                            .to_string(),
                        );
                    }
                }
            }
            Err(_) => {
                logger.debug(
                    &format!(
                        "potential match '{}' ignored. decoding types failed",
                        &potential_match.signature
                    )
                    .to_string(),
                );
            }
        }
    }

    if matches.is_empty() {
        return None
    }

    Some(matches)
}
