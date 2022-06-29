mod decode;
mod decompile;

use clap::{Parser, Subcommand};

use heimdall_config::{config, get_config, ConfigArgs};
use heimdall_common::ether::evm::disassemble::*;
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
    #[clap(name = "decompile", about = "Decompile EVM bytecode to Solidity")]
    Decompile(decompile::DecompilerArgs),

    #[clap(name = "disassemble", about = "Disassemble EVM bytecode to assembly")]
    Disassemble(DisassemblerArgs),

    #[clap(name = "decode", about = "Decode calldata into readable types")]
    Decode(DecodeArgs),

    #[clap(name = "config", about = "Display and edit the current configuration")]
    Config(ConfigArgs),

    #[clap(name = "cache", about = "Manage cached files for Heimdall.")]
    Cache(decompile::DecompilerArgs),
}

fn main() {
    let args = Arguments::parse();

    let configuration = get_config();
    match args.sub {
        Subcommands::Decompile(cmd) => {
            println!("{:#?}", cmd)
        }

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

        Subcommands::Cache(cmd) => {
            println!("{:#?}", cmd)
        }
    }
}
