mod tests;

use std::{
    str::FromStr
};

use clap::{AppSettings, Parser};
use ethers::{
    core::types::{H256},
    providers::{Middleware, Provider, Http}, types::{Transaction, Address},
};

use heimdall_common::{
    io::logging::Logger,
    constants::TRANSACTION_HASH_REGEX,
    ether::{evm::{vm::VM}}, utils::strings::encode_hex
};

#[derive(Debug, Clone, Parser)]
#[clap(about = "Trace a contract interaction, revealing the internals of the transaction.",
       after_help = "For more information, read the wiki: https://jbecker.dev/r/heimdall-rs/wiki",
       global_setting = AppSettings::DeriveDisplayOrder, 
       override_usage = "heimdall trace <TARGET> [OPTIONS]")]
pub struct TraceArgs {
    
    /// The transaction hash of the target to trace.
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
pub fn trace(args: TraceArgs) {
    let (logger, mut trace)= Logger::new(args.verbose.log_level().unwrap().as_str());
    let raw_transaction: Transaction;
    let calldata;
    let value;
    let interacted_with;
    let contract_bytecode;

    // determine whether or not the target is a transaction hash
    if TRANSACTION_HASH_REGEX.is_match(&args.target).unwrap() {

        // create new runtime block
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        
        // We are decoding a transaction hash, so we need to fetch the calldata from the RPC provider.
        raw_transaction = rt.block_on(async {

            // make sure the RPC provider isn't empty
            if &args.rpc_url.len() <= &0 {
                logger.error("tracing an on-chain transaction requires an RPC provider. Use `heimdall decode --help` for more information.");
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

            return raw_transaction;
        });

        value = raw_transaction.value;
        calldata = raw_transaction.input.to_string();
        interacted_with = match raw_transaction.to {
            Some(to) => to,
            None => {
                logger.error(&format!("transaction '{}' is a contract creation.", &args.target).to_string());
                std::process::exit(1)
            }
        };

        // get the bytecode of the contract that was interacted with
        contract_bytecode = rt.block_on(async {

            // create new provider
            let provider = match Provider::<Http>::try_from(&args.rpc_url) {
                Ok(provider) => provider,
                Err(_) => {
                    logger.error(&format!("failed to connect to RPC provider '{}' .", &args.rpc_url).to_string());
                    std::process::exit(1)
                }
            };

            // fetch the bytecode at the address
            let bytecode_as_bytes = match provider.get_code(interacted_with, None).await {
                Ok(bytecode) => bytecode,
                Err(_) => {
                    logger.error(&format!("failed to fetch bytecode from '{:?}' .", &interacted_with.to_string()).to_string());
                    std::process::exit(1)
                }
            };

            return bytecode_as_bytes.to_string().replacen("0x", "", 1);
        });

    }
    else {
        logger.error(&format!("'{}' is not a valid transaction hash.", &args.target));
        std::process::exit(1);
    }

    // check if calldata is present
    if calldata.len() == 0 {
        logger.error(&format!("empty calldata found at '{}' .", &args.target));
        std::process::exit(1);
    }

    // check if bytecode is present
    if contract_bytecode.len() == 0 {
        logger.error(&format!("empty calldata found at '{}' .", &args.target));
        std::process::exit(1);
    }

    // make a new VM object
    let mut vm = VM::new(
        contract_bytecode.clone(),
        calldata.clone(),
        format!("0x{}", encode_hex(interacted_with.as_bytes().to_vec())),
        format!("0x{}", encode_hex(interacted_with.as_bytes().to_vec())),
        format!("0x{}", encode_hex(raw_transaction.from.as_bytes().to_vec())),
        value.as_u128(),
        raw_transaction.gas.as_u128(),
    );

    // run the VM
    while vm.bytecode.len() >= (vm.instruction * 2 + 2) as usize {
        let state = vm.step();

        println!("{:?}", state.last_instruction.opcode_details.unwrap().name);

        if vm.exitcode != 255 || vm.returndata.len() > 0 {
            break;
        }
    }
    
    // force the trace to display
    trace.level = 4;
    trace.display();

}