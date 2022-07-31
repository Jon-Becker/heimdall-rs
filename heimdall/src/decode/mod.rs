use std::{
    str::FromStr
};

use clap::{AppSettings, Parser};
use ethers::{
    core::types::{H256},
    providers::{Middleware, Provider, Http},
    abi::{ParamType, decode as decode_abi, Function, StateMutability, Param, AbiEncode},
};

use heimdall_common::{
    io::logging::Logger,
    consts::TRANSACTION_HASH_REGEX,
    utils::{
        strings::decode_hex,
    }, ether::{evm::types::{parse_function_parameters, display}, signatures::{resolve_signature, ResolvedFunction}}
};

use strsim::normalized_damerau_levenshtein as similarity;

#[derive(Debug, Clone, Parser)]
#[clap(about = "Decode calldata into readable types",
       after_help = "For more information, read the wiki: https://jbecker.dev/r/heimdall-rs/wiki",
       global_setting = AppSettings::DeriveDisplayOrder, 
       override_usage = "heimdall decode <TARGET> [OPTIONS]")]
pub struct DecodeArgs {
    
    /// The target to decode, either a transaction hash or string of bytes.
    #[clap(required=true)]
    pub target: String,

    /// Set the output verbosity level, 1 - 5.
    #[clap(flatten)]
    pub verbose: clap_verbosity_flag::Verbosity,

    /// The RPC provider to use for fetching target bytecode.
    #[clap(long="rpc-url", short, default_value = "", hide_default_value = true)]
    pub rpc_url: String,

    /// When prompted, always select the default value.
    #[clap(long, short)]
    pub default: bool,

}


#[allow(deprecated)]
pub fn decode(args: DecodeArgs) {
    let (logger, mut trace)= Logger::new(args.verbose.log_level().unwrap().as_str());
    let calldata: String;

    // determine whether or not the target is a transaction hash
    if TRANSACTION_HASH_REGEX.is_match(&args.target) {

        // create new runtime block
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        
        // We are decoding a transaction hash, so we need to fetch the calldata from the RPC provider.
        calldata = rt.block_on(async {

            // make sure the RPC provider isn't empty
            if &args.rpc_url.len() <= &0 {
                logger.error("decoging an on-chain transaction requires an RPC provider. Use `heimdall decode --help` for more information.");
                std::process::exit(1);
            }

            // create new provider
            let provider = match Provider::<Http>::try_from(&args.rpc_url) {
                Ok(provider) => provider,
                Err(_) => {
                    logger.error(&format!("failed to connect to RPC provider '{}' .", &args.rpc_url).to_string());
                    std::process::exit(1)
                }
            };

            // safely unwrap the transaction hash
            let transaction_hash = match H256::from_str(&args.target) {
                Ok(transaction_hash) => transaction_hash,
                Err(_) => {
                    logger.error(&format!("failed to parse transaction hash '{}' .", &args.target));
                    std::process::exit(1)
                }
            };

            // fetch the transaction from the node
            let raw_transaction = match provider.get_transaction(transaction_hash).await {
                Ok(bytecode) => {
                    match bytecode {
                        Some(bytecode) => bytecode,
                        None => {
                            logger.error(&format!("transaction '{}' doesn't exist.", &args.target).to_string());
                            std::process::exit(1)
                        }
                    }
                },
                Err(_) => {
                    logger.error(&format!("failed to fetch calldata from '{}' .", &args.target).to_string());
                    std::process::exit(1)
                }
            };

            return raw_transaction.input.to_string().replace("0x", "")
        });
    }
    else {
        calldata = args.target.clone().replace("0x", "");
    }

    // check if the calldata length is a standard length
    if calldata.len() % 2 != 0 {
        logger.error("calldata is not a valid hex string.");
        std::process::exit(1);
    }

    // if calldata isn't a multiple of 64, it may be harder to decode.
    if calldata[8..].len() % 64 != 0 { logger.warn("calldata is not a standard size. decoding may fail since each word is not exactly 32 bytes long."); }

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
    let potential_matches = match resolve_signature(&function_selector) {
        Some(signatures) => signatures,
        None => Vec::new()
    };
    let mut matches: Vec<ResolvedFunction> = Vec::new();

    for potential_match in &potential_matches {

        // convert the string inputs into a vector of decoded types
        let mut inputs: Vec<ParamType> = Vec::new();
        match parse_function_parameters(potential_match.signature.to_owned()) {
            Some(type_) => {
                for input in type_ {
                    inputs.push(input);
                }
            },
            None => continue
        }
        match decode_abi(&inputs, &byte_args) {
            Ok(result) => {

                // convert tokens to params
                let mut params: Vec<Param> = Vec::new();
                for (i, input) in inputs.iter().enumerate() {
                    params.push(Param {
                        name: format!("arg{}", i),
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
                }.encode_input(&result);
                match decoded_function_call {
                    Ok(decoded_function_call) => {

                        // decode the function call in trimmed bytes, removing 0s, because contracts can use nonstandard sized words
                        // and padding is hard
                        let cleaned_bytes = decoded_function_call.encode_hex().replace("0", "");
                        let decoded_function_call = match cleaned_bytes.split_once(&function_selector.replace("0", "")) {
                            Some(decoded_function_call) => decoded_function_call.1,
                            None => {
                                logger.debug(&format!("potential match '{}' ignored. decoded inputs differed from provided calldata.", &potential_match.signature).to_string());
                                continue
                            }
                        };

                        // if the decoded function call matches (95%) the function signature, add it to the list of matches
                        if similarity(decoded_function_call,&calldata[8..].replace("0", "")).abs() >= 0.90 {
                            let mut found_match = potential_match.clone();
                            found_match.decoded_inputs = Some(result);
                            matches.push(found_match);
                        }
                        else {
                            logger.debug(&format!("potential match '{}' ignored. decoded inputs differed from provided calldata.", &potential_match.signature).to_string());
                        }

                    },
                    Err(_) => { logger.debug(&format!("potential match '{}' ignored. type checking failed", &potential_match.signature).to_string()); }
                }
                
            },
            Err(_) => { logger.debug(&format!("potential match '{}' ignored. decoding types failed", &potential_match.signature).to_string()); }
        }
    }

    // truncate target for prettier display
    let mut shortened_target = args.target;
    if shortened_target.len() > 66 {
        shortened_target = shortened_target.chars().take(66).collect::<String>() + "..." + &shortened_target.chars().skip(shortened_target.len() - 16).collect::<String>();
    }

    if matches.len() == 0 {
        logger.warn("couldn't find any matches for the given function signature.");

        // build a trace of the calldata
        let decode_call = trace.add_call(0, line!(), "heimdall".to_string(), "decode".to_string(), vec![shortened_target], "()".to_string());
        trace.br(decode_call);
        trace.add_message(decode_call, line!(), vec![format!("selector: 0x{}", function_selector).to_string()]);
        trace.add_message(decode_call, line!(), vec![format!("calldata: {} bytes", calldata.len() / 2usize).to_string()]);
        trace.br(decode_call);

        // print out the decoded inputs
        let mut inputs: Vec<String> = Vec::new();
        for (i, input) in calldata[8..].chars().collect::<Vec<char>>().chunks(64).map(|c| c.iter().collect::<String>()).collect::<Vec<String>>().iter().enumerate() {
            inputs.push(
                format!(
                    "{} {}:{}{}",
                    if i == 0 { "input" } else { "     " },
                    i,
                    " ".repeat(3 - i.to_string().len()),
                    input
                ).to_string()
            )
        }
        trace.add_message(decode_call, line!(), inputs);
        
    }
    else {
        let mut selection: u8 = 0;
        if matches.len() > 1 {
            selection = logger.option(
                "warn", "multiple possible matches found. select an option below",
                matches.iter()
                .map(|x| x.signature.clone()).collect(),
                Some(*&(matches.len()-1) as u8),
                args.default
            );
        }

        let selected_match = match matches.get(selection as usize) {
            Some(selected_match) => selected_match,
            None => {
                logger.error("invalid selection.");
                std::process::exit(1)
            }
        };

        // print out the match and it's decoded inputs

        let decode_call = trace.add_call(0, line!(), "heimdall".to_string(), "decode".to_string(), vec![shortened_target], "()".to_string());
        trace.br(decode_call);
        trace.add_message(decode_call, line!(), vec![format!("name:      {}", selected_match.name).to_string()]);
        trace.add_message(decode_call, line!(), vec![format!("signature: {}", selected_match.signature).to_string()]);
        trace.add_message(decode_call, line!(), vec![format!("selector:  0x{}", function_selector).to_string()]);
        trace.add_message(decode_call, line!(), vec![format!("calldata:  {} bytes", calldata.len() / 2usize).to_string()]);
        trace.br(decode_call);
        for (i, input) in selected_match.decoded_inputs.as_ref().unwrap().iter().enumerate() {
            let mut decoded_inputs_as_message = display(vec![input.to_owned()], "           ");
            if i == 0 {
                decoded_inputs_as_message[0] = format!(
                    "input {}:{}{}",
                    i,
                    " ".repeat(4 - i.to_string().len()),
                    decoded_inputs_as_message[0].replacen("           ", "", 1)
                )
            }
            else {
                decoded_inputs_as_message[0] = format!(
                    "      {}:{}{}",
                    i,
                    " ".repeat(4 - i.to_string().len()),
                    decoded_inputs_as_message[0].replacen("           ", "", 1)
                )
            }

            trace.add_message(decode_call, 1, decoded_inputs_as_message);
        }
    }

    // force the trace to display
    trace.level = 4;
    trace.display();

}