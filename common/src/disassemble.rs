use serde::{Deserialize, Serialize};
use serde_json::Result;

use clap::{AppSettings, Parser};
use ethers::{
    core::types::{Address},
    prelude::*,
    providers::{Middleware, Provider, Http},
};

use crate::consts::{
    ADDRESS_REGEX,
};

#[derive(Debug, Clone, Parser)]
#[clap(about = "Disassemble EVM bytecode to Assembly",
       after_help = "For more information, read the wiki: https://jbecker.dev/r/heimdall-rs/wiki",
       global_setting = AppSettings::DeriveDisplayOrder, 
       override_usage = "heimdall disassemble <TARGET> [OPTIONS]")]
       
pub struct DisassemblerArgs {
    // The target to decompile, either a file, contract address, or ENS name.
    #[clap(required=true)]
    target: String,

    // Set the output verbosity level, 1 - 5.
    #[clap(flatten)]
    verbose: clap_verbosity_flag::Verbosity,
    
    // The output directory to write the decompiled files to
    #[clap(long="output", short, default_value = "./output/", hide_default_value = true)]
    output: String,

    // The RPC provider to use for fetching target bytecode.
    #[clap(long="rpc-url", short, default_value = "http://localhost:8545", hide_default_value = true)]
    rpc_url: String,

    // When prompted, always select the default value.
    #[clap(long, short)]
    default: bool,

}

pub async fn disassemble(args: DisassemblerArgs) {
    println!("{:#?}", args);
    if ADDRESS_REGEX.is_match(&args.target) {

        // We are decompiling a contract address, so we need to fetch the bytecode from the RPC provider.
        let provider = Provider::<Http>::try_from(&args.rpc_url)
            .expect("Failed to connect to RPC provider.");

        let address = args.target.parse::<Address>()
            .expect("Failed to parse address.");

        let code = provider.get_code(address, None).await.expect("Failed to get code.");

        println!("Got code: {:#?}", serde_json::to_string(&code));
        
        }
    else {
        println!("2");
    }
}