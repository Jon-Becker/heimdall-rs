use std::{panic};
use backtrace::Backtrace;

mod decode;
mod decompile;

use clap::{Parser, Subcommand};

use colored::Colorize;
use heimdall_config::{config, get_config, ConfigArgs};
use heimdall_common::{ether::evm::disassemble::*, io::{logging::Logger}};
use decompile::{decompile, DecompilerArgs};
use decode::{decode, DecodeArgs};


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

    #[clap(name = "decode", about = "Decode calldata into readable types")]
    Decode(DecodeArgs),

    #[clap(name = "config", about = "Display and edit the current configuration")]
    Config(ConfigArgs),


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
                    "thread 'main' encountered a fatal error: '{}' at '/src/{}:{}'!", 
                    panic_info.to_string().split("'").collect::<Vec<&str>>()[1]
                        .to_lowercase().bright_white().on_bright_red().bold(),
                    panic_info.location().unwrap().file().split("/src/")
                        .collect::<Vec<&str>>()[1],
                    panic_info.location().unwrap().line()
                )
            );
            logger.fatal(&format!("Stack Trace:\n\n{:#?}", backtrace));
        }
    ));

    let configuration = get_config();
    match args.sub {

        Subcommands::Disassemble(mut cmd) => {
            
            // if the user has not specified a rpc url, use the default
            match cmd.rpc_url.as_str() {
                "" => {
                    cmd.rpc_url = configuration.rpc_url.clone();
                }
                _ => {}
            };

            disassemble(cmd);
        }

        Subcommands::Decompile(mut cmd) => {
            
            // if the user has not specified a rpc url, use the default
            match cmd.rpc_url.as_str() {
                "" => {
                    cmd.rpc_url = configuration.rpc_url.clone();
                }
                _ => {}
            };

            decompile(cmd);
        }

        Subcommands::Decode(mut cmd) => {
            
            // if the user has not specified a rpc url, use the default
            match cmd.rpc_url.as_str() {
                "" => {
                    cmd.rpc_url = configuration.rpc_url.clone();
                }
                _ => {}
            };

            decode(cmd);
        }

        Subcommands::Config(cmd) => {
            config(cmd);
        }
        
    }
}
