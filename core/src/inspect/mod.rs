use std::collections::HashMap;

use clap::{AppSettings, Parser};
use clap_verbosity_flag::Verbosity;
use derive_builder::Builder;

use heimdall_common::{
    ether::rpc::get_trace,
    utils::{io::logging::Logger, strings::ToLowerHex},
};

use crate::{decode::DecodeArgsBuilder, error::Error};

#[derive(Debug, Clone, Parser, Builder)]
#[clap(
    about = "Detailed inspection of Ethereum transactions, including calldata & trace decoding, log visualization, and more.",
    after_help = "For more information, read the wiki: https://jbecker.dev/r/heimdall-rs/wiki",
    global_setting = AppSettings::DeriveDisplayOrder,
    override_usage = "heimdall inspect <TARGET> [OPTIONS]"
)]
pub struct InspectArgs {
    /// The target transaction hash to inspect.
    #[clap(required = true)]
    pub target: String,

    /// Set the output verbosity level, 1 - 5.
    #[clap(flatten)]
    pub verbose: clap_verbosity_flag::Verbosity,

    /// The RPC provider to use for fetching target calldata.
    #[clap(long = "rpc-url", short, default_value = "", hide_default_value = true)]
    pub rpc_url: String,

    /// When prompted, always select the default value.
    #[clap(long, short)]
    pub default: bool,
}

impl InspectArgsBuilder {
    pub fn new() -> Self {
        Self {
            target: Some(String::new()),
            verbose: Some(clap_verbosity_flag::Verbosity::new(0, 1)),
            rpc_url: Some(String::new()),
            default: Some(true),
        }
    }
}

/// The entrypoint for the inspect module. This function will analyze the given transaction and
/// provide a detailed inspection of the transaction, including calldata & trace decoding, log
/// visualization, and more.
#[allow(deprecated)]
pub async fn inspect(args: InspectArgs) -> Result<(), Error> {
    // set logger environment variable if not already set
    // TODO: abstract this to a heimdall_common util
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var(
            "RUST_LOG",
            match args.verbose.log_level() {
                Some(level) => level.as_str(),
                None => "SILENT",
            },
        );
    }

    // get a new logger and trace
    let (_logger, mut trace) = Logger::new(match args.verbose.log_level() {
        Some(level) => level.as_str(),
        None => "SILENT",
    });

    // get calldata from RPC
    // let transaction = get_transaction(&args.target, &args.rpc_url)
    //     .await
    //     .map_err(|e| Error::RpcError(e.to_string()))?;

    // get trace
    let block_trace =
        get_trace(&args.target, &args.rpc_url).await.map_err(|e| Error::RpcError(e.to_string()))?;

    let decode_call = trace.add_call(
        0,
        line!(),
        "heimdall".to_string(),
        "inspect".to_string(),
        vec![args.target.clone()],
        "()".to_string(),
    );

    if let Some(transaction_traces) = block_trace.trace {
        let mut trace_indices = HashMap::new();

        for transaction_trace in transaction_traces {
            println!("trace: {:?}", transaction_trace);
            let trace_address = transaction_trace
                .trace_address
                .iter()
                .map(|address| address.to_string())
                .collect::<Vec<_>>()
                .join(".");
            let parent_address = trace_address
                .split('.')
                .take(trace_address.split('.').count() - 1)
                .collect::<Vec<_>>()
                .join(".");

            // get trace index from parent_address
            let parent_index = trace_indices.get(&parent_address).unwrap_or(&decode_call);

            // get action
            match transaction_trace.action {
                ethers::types::Action::Call(call) => {
                    // attempt to decode calldata
                    let calldata = call.input.to_string();

                    if !calldata.replacen("0x", "", 1).is_empty() {
                        let result = crate::decode::decode(
                            DecodeArgsBuilder::new()
                                .target(calldata)
                                .rpc_url("https://eth.llamarpc.com".to_string())
                                .verbose(Verbosity::new(2, 0))
                                .build()
                                .map_err(|_e| Error::DecodeError)?,
                        )
                        .await?;

                        // get first result
                        if let Some(resolved_function) = result.first() {
                            // convert decoded inputs Option<Vec<Token>> to Vec<Token>
                            let decoded_inputs =
                                resolved_function.decoded_inputs.clone().unwrap_or_default();

                            // get index of parent
                            let parent_index = trace.add_call(
                                *parent_index,
                                line!(),
                                call.to.to_lower_hex(),
                                resolved_function.name.clone(),
                                decoded_inputs
                                    .iter()
                                    .map(|token| format!("{:?}", token))
                                    .collect::<Vec<String>>(),
                                "()".to_string(),
                            );

                            // add trace_address to trace_indices
                            trace_indices.insert(trace_address.clone(), parent_index);
                            trace.add_info(
                                parent_index,
                                line!(),
                                &format!("trace_address: {:?}", trace_address),
                            );
                        } else {
                            unimplemented!();
                        }
                    } else {
                        // value transfer
                        trace.add_call(
                            *parent_index,
                            line!(),
                            call.to.to_lower_hex(),
                            "transfer".to_string(),
                            vec![format!("{} wei", call.value)],
                            "()".to_string(),
                        );
                    }
                }
                ethers::types::Action::Create(_) => todo!(),
                ethers::types::Action::Suicide(_) => todo!(),
                ethers::types::Action::Reward(_) => todo!(),
            }
        }
    }

    trace.display();

    Ok(())
}
