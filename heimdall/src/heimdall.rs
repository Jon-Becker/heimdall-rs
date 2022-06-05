mod decompile;

use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[clap(name = "heimdall", author = "Jonathan Becker <jonathan@jbecker.dev>", version)]
       
pub struct Opts {
    #[clap(subcommand)]
    pub sub: Subcommands,
}

#[derive(Debug, Subcommand)]
#[clap(
    about = "Heimdall is an advanced Ethereum smart contract toolkit for forensic and heuristic analysis.",
    after_help = ""
)]
#[allow(clippy::large_enum_variant)]
pub enum Subcommands {
    #[clap(name = "decompile", about = "Decompile EVM bytecode to Solidity")]
    Decompile(decompile::TestArgs),

    #[clap(name = "disassemble", about = "Disassemble EVM bytecode to assembly")]
    Disassemble(decompile::TestArgs),

    #[clap(name = "config", about = "Display and edit the current configuration")]
    Config(decompile::TestArgs),

    #[clap(name = "cache", about = "Manage cached files for Heimdall.")]
    Cache(decompile::TestArgs),

}

fn main() {
    let opts = Opts::parse();
    
    println!("{:#?}", opts);
}