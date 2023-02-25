use std::{panic};
use backtrace::Backtrace;

mod cfg;
mod decode;
mod decompile;

use clap::{Parser, Subcommand};

use colored::Colorize;
use heimdall_config::{config, get_config, ConfigArgs};
use heimdall_cache::{cache, CacheArgs};
use heimdall_common::{ether::evm::disassemble::*, io::{logging::Logger}};
use decompile::{decompile, DecompilerArgs};
use decode::{decode, DecodeArgs};
use cfg::{cfg, CFGArgs};

#[derive(Debug, Parser)]
#[clap(
    name = "heimdall",
    author = "Jonathan Becker <jonathan@jbecker.dev>",
    version
)]
pub struct Arguments {
    #[clap(subcommand)]
    pub sub: Subcommands,
}

#[derive(Debug, Subcommand)]
#[clap(
    about = "Heimdall is an advanced Ethereum smart contract toolkit for forensic and heuristic analysis.",
    after_help = "For more information, read the wiki: https://jbecker.dev/r/heimdall-rs/wiki"
)]
#[allow(clippy::large_enum_variant)]
pub enum Subcommands {

    #[clap(name = "disassemble", about = "Disassemble EVM bytecode to assembly")]
    Disassemble(DisassemblerArgs),

    #[clap(name = "decompile", about = "Decompile EVM bytecode to Solidity")]
    Decompile(DecompilerArgs),

    #[clap(name = "cfg", about = "Generate a visual control flow graph for EVM bytecode")]
    CFG(CFGArgs),

    #[clap(name = "decode", about = "Decode calldata into readable types")]
    Decode(DecodeArgs),

    #[clap(name = "config", about = "Display and edit the current configuration")]
    Config(ConfigArgs),

    #[clap(name = "cache", about = "Manage heimdall-rs' cached files")]
    Cache(CacheArgs),
}

fn main() {
    let args = Arguments::parse();

    // handle catching panics with
    panic::set_hook(
        Box::new(|panic_info| {
            let backtrace = Backtrace::new();
            let (logger, _)= Logger::new("TRACE");
            logger.fatal(
                &format!(
                    "thread 'main' encountered a fatal error: '{}'!", 
                    panic_info.to_string().bright_white().on_bright_red().bold(),
                )
            );
            logger.fatal(&format!("Stack Trace:\n\n{backtrace:#?}"));
        }
    ));

    let configuration = get_config();
    match args.sub {


        Subcommands::Disassemble(mut cmd) => {
            
            // if the user has not specified a rpc url, use the default
            match cmd.rpc_url.as_str() {
                "" => {
                    cmd.rpc_url = configuration.rpc_url;
                }
                _ => {}
            };

            disassemble(cmd);
        }


        Subcommands::Decompile(mut cmd) => {
            
            // if the user has not specified a rpc url, use the default
            match cmd.rpc_url.as_str() {
                "" => {
                    cmd.rpc_url = configuration.rpc_url;
                }
                _ => {}
            };

            decompile(cmd);
        }


        Subcommands::Decode(mut cmd) => {
            
            // if the user has not specified a rpc url, use the default
            match cmd.rpc_url.as_str() {
                "" => {
                    cmd.rpc_url = configuration.rpc_url;
                }
                _ => {}
            };

            decode(cmd);
        }


        Subcommands::CFG(mut cmd) => {
            
            // if the user has not specified a rpc url, use the default
            match cmd.rpc_url.as_str() {
                "" => {
                    cmd.rpc_url = configuration.rpc_url;
                }
                _ => {}
            };

            cfg(cmd);
        }


        Subcommands::Config(cmd) => {
            config(cmd);
        }


        Subcommands::Cache(cmd) => {
            _ = cache(cmd);
        }
        

    }
}
