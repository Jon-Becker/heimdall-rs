mod tests;

use std::sync::Arc;

use clap::{AppSettings, Parser};


use ethers::{providers::{Ws, Provider, Middleware}, types::BlockNumber};
use heimdall_common::io::logging::Logger;

#[derive(Debug, Clone, Parser)]
#[clap(about = "Advanced mempool monitoring, allowing for cURL triggers",
       after_help = "For more information, read the wiki: https://jbecker.dev/r/heimdall-rs/wiki",
       global_setting = AppSettings::DeriveDisplayOrder,
       override_usage = "heimdall monitor <TARGET> [OPTIONS]")]
pub struct MonitorArgs {
    /// Set the output verbosity level, 1 - 5.
    #[clap(flatten)]
    pub verbose: clap_verbosity_flag::Verbosity,

    /// The RPC provider to use for fetching target bytecode.
    #[clap(long = "rpc-url", short, default_value = "", hide_default_value = true)]
    pub rpc_url: String,
}

#[allow(deprecated)]
pub fn monitor(args: MonitorArgs) {
    let (logger, mut trace) = Logger::new(args.verbose.log_level().unwrap().as_str());

    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();

    // We are decoding a transaction hash, so we need to fetch the calldata from the RPC provider.
    rt.block_on(async {

        // make sure the RPC provider isn't empty
        if args.rpc_url.is_empty() {
            logger.error("decoding an on-chain transaction requires an RPC provider. Use `heimdall decode --help` for more information.");
            std::process::exit(1);
        }

        // create new provider
        let provider = match Provider::<Ws>::connect(&args.rpc_url).await {
            Ok(provider) => provider,
            Err(_) => {
                logger.error(&format!("failed to connect to RPC provider '{}' .", &args.rpc_url));
                std::process::exit(1)
            }
        };

        let client = Arc::new(provider);
        let last_block = client.get_block(BlockNumber::Latest).await.unwrap().unwrap().number.unwrap();
        println!("last_block: {}", last_block);

    });
}
