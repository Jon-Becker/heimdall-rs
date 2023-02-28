mod tests;

use std::env;
use std::fs;
use heimdall_cache::read_cache;
use heimdall_cache::store_cache;

use clap::{AppSettings, Parser};
use ethers::{
    core::types::{Address},
    providers::{Middleware, Provider, Http},
};
use heimdall_common::{
    ether::evm::{
        vm::VM
    },
    constants::{ ADDRESS_REGEX, BYTECODE_REGEX },
    io::{ logging::* },
};


#[derive(Debug, Clone, Parser)]
#[clap(about = "Dump the value of all storage slots accessed by a contract",
       after_help = "For more information, read the wiki: https://jbecker.dev/r/heimdall-rs/wiki",
       global_setting = AppSettings::DeriveDisplayOrder,
       override_usage = "heimdall dump <TARGET> [OPTIONS]")]
pub struct DumpArgs {

    /// The target to find and dump the storage slots of.
    #[clap(required=true)]
    pub target: String,

    /// Set the output verbosity level, 1 - 5.
    #[clap(flatten)]
    pub verbose: clap_verbosity_flag::Verbosity,

    /// The output directory to write the output to
    #[clap(long="output", short, default_value = "", hide_default_value = true)]
    pub output: String,

    /// The RPC provider to use for fetching on-chain data.
    #[clap(long="rpc-url", short, default_value = "", hide_default_value = true)]
    pub rpc_url: String,

    /// When prompted, always select the default value.
    #[clap(long, short)]
    pub default: bool,
}

pub fn dump(args: DumpArgs) {
    use std::time::Instant;
    let now = Instant::now();

    let (logger, mut trace)= Logger::new(args.verbose.log_level().unwrap().as_str());

    // truncate target for prettier display
    let mut shortened_target = args.target.clone();
    if shortened_target.len() > 66 {
        shortened_target = shortened_target.chars().take(66).collect::<String>() + "..." + &shortened_target.chars().skip(shortened_target.len() - 16).collect::<String>();
    }

    // add the call to the trace
    let dump_call = trace.add_call(
        0, line!(),
        "heimdall".to_string(),
        "dump".to_string(),
        vec![shortened_target],
        "()".to_string()
    );

    // parse the output directory
    let mut output_dir: String;
    if &args.output.len() <= &0 {
        output_dir = match env::current_dir() {
            Ok(dir) => dir.into_os_string().into_string().unwrap(),
            Err(_) => {
                logger.error("failed to get current directory.");
                std::process::exit(1);
            }
        };
        output_dir.push_str("/output");
    }
    else {
        output_dir = args.output.clone();
    }

    // fetch bytecode
    let contract_bytecode: String;
    if ADDRESS_REGEX.is_match(&args.target).unwrap() {

        // push the address to the output directory
        if &output_dir != &args.output {
            output_dir.push_str(&format!("/{}", &args.target));
        }

        // create new runtime block
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        // We are working with a contract address, so we need to fetch the bytecode from the RPC provider.
        contract_bytecode = rt.block_on(async {

            // check the cache for a matching address
            match read_cache(&format!("contract.{}", &args.target)) {
                Some(bytecode) => {
                    logger.debug(&format!("found cached bytecode for '{}' .", &args.target));
                    return bytecode;
                },
                None => {}
            }

            // make sure the RPC provider isn't empty
            if &args.rpc_url.len() <= &0 {
                logger.error("fetching an on-chain contract requires an RPC provider. Use `heimdall dump --help` for more information.");
                std::process::exit(1);
            }

            // create new provider
            let provider = match Provider::<Http>::try_from(&args.rpc_url) {
                Ok(provider) => provider,
                Err(_) => {
                    logger.error(&format!("failed to connect to RPC provider '{}' .", &args.rpc_url));
                    std::process::exit(1)
                }
            };

            // safely unwrap the address
            let address = match args.target.parse::<Address>() {
                Ok(address) => address,
                Err(_) => {
                    logger.error(&format!("failed to parse address '{}' .", &args.target));
                    std::process::exit(1)
                }
            };

            // fetch the bytecode at the address
            let bytecode_as_bytes = match provider.get_code(address, None).await {
                Ok(bytecode) => bytecode,
                Err(_) => {
                    logger.error(&format!("failed to fetch bytecode from '{}' .", &args.target));
                    std::process::exit(1)
                }
            };

            // cache the results
            store_cache(&format!("contract.{}", &args.target), bytecode_as_bytes.to_string().replacen("0x", "", 1), None);

            bytecode_as_bytes.to_string().replacen("0x", "", 1)
        });

    }
    else if BYTECODE_REGEX.is_match(&args.target).unwrap() {
        contract_bytecode = args.target.clone();
    }
    else {

        // push the address to the output directory
        if &output_dir != &args.output {
            output_dir.push_str("/local");
        }

        // We are analyzing a file, so we need to read the bytecode from the file.
        contract_bytecode = match fs::read_to_string(&args.target) {
            Ok(contents) => {
                if BYTECODE_REGEX.is_match(&contents).unwrap() && contents.len() % 2 == 0 {
                    contents.replacen("0x", "", 1)
                }
                else {
                    logger.error(&format!("file '{}' doesn't contain valid bytecode.", &args.target));
                    std::process::exit(1)
                }
            },
            Err(_) => {
                logger.error(&format!("failed to open file '{}' .", &args.target));
                std::process::exit(1)
            }
        };
    }

    logger.debug(&format!("Dumped storage slots in {:?}.", now.elapsed()));
}