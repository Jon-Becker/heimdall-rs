use backtrace::Backtrace;
use std::{io, panic};

mod cfg;
mod decode;
mod decompile;
mod dump;
mod snapshot;

use clap::{Parser, Subcommand};

use cfg::{cfg, CFGArgs};
use colored::Colorize;
use crossterm::{
    event::DisableMouseCapture,
    execute,
    terminal::{disable_raw_mode, LeaveAlternateScreen},
};

use decode::{decode, DecodeArgs};
use decompile::{decompile, DecompilerArgs};
use dump::{dump, DumpArgs};

use heimdall_cache::{cache, CacheArgs};
use heimdall_common::{
    ether::evm::disassemble::*,
    io::logging::Logger,
    utils::version::{current_version, remote_version},
};
use heimdall_config::{config, get_config, ConfigArgs};
use tui::{backend::CrosstermBackend, Terminal};

#[derive(Debug, Parser)]
#[clap(name = "heimdall", author = "Jonathan Becker <jonathan@jbecker.dev>", version)]
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

    #[clap(name = "dump", about = "Dump the value of all storage slots accessed by a contract")]
    Dump(DumpArgs),
    // #[clap(
    //     name = "snapshot",
    //     about = "Infer function information from bytecode, including access control, gas
    // consumption, storage accesses, event emissions, and more" )]
    // Snapshot(SnapshotArgs),
}

fn main() {
    let args = Arguments::parse();

    // handle catching panics with
    panic::set_hook(Box::new(|panic_info| {
        // cleanup the terminal
        let stdout = io::stdout();
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend).unwrap();
        disable_raw_mode().unwrap();
        execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture).unwrap();
        terminal.show_cursor().unwrap();

        // print the panic message
        let backtrace = Backtrace::new();
        let (logger, _) = Logger::new("TRACE");
        logger.fatal(&format!(
            "thread 'main' encountered a fatal error: '{}'!",
            panic_info.to_string().bright_white().on_bright_red().bold(),
        ));
        logger.fatal(&format!("Stack Trace:\n\n{backtrace:#?}"));
    }));

    let configuration = get_config();
    match args.sub {
        Subcommands::Disassemble(mut cmd) => {
            // if the user has not specified a rpc url, use the default
            if cmd.rpc_url.as_str() == "" {
                cmd.rpc_url = configuration.rpc_url;
            }

            disassemble(cmd);
        }

        Subcommands::Decompile(mut cmd) => {
            // if the user has not specified a rpc url, use the default
            if cmd.rpc_url.as_str() == "" {
                cmd.rpc_url = configuration.rpc_url;
            }

            decompile(cmd);
        }

        Subcommands::Decode(mut cmd) => {
            // if the user has not specified a rpc url, use the default
            if cmd.rpc_url.as_str() == "" {
                cmd.rpc_url = configuration.rpc_url;
            }

            // if the user has not specified a openai api key, use the default
            if cmd.openai_api_key.as_str() == "" {
                cmd.openai_api_key = configuration.openai_api_key;
            }

            decode(cmd);
        }

        Subcommands::CFG(mut cmd) => {
            // if the user has not specified a rpc url, use the default
            if cmd.rpc_url.as_str() == "" {
                cmd.rpc_url = configuration.rpc_url;
            }

            cfg(cmd);
        }

        Subcommands::Dump(mut cmd) => {
            // if the user has not specified a rpc url, use the default
            if cmd.rpc_url.as_str() == "" {
                cmd.rpc_url = configuration.rpc_url;
            }

            // if the user has not specified a transpose api key, use the default
            if cmd.transpose_api_key.as_str() == "" {
                cmd.transpose_api_key = configuration.transpose_api_key;
            }

            dump(cmd);
        }

        // Subcommands::Snapshot(mut cmd) => {
        //     // if the user has not specified a rpc url, use the default
        //     if cmd.rpc_url.as_str() == "" {
        //         cmd.rpc_url = configuration.rpc_url;
        //     }

        //     snapshot(cmd);
        // }
        Subcommands::Config(cmd) => {
            config(cmd);
        }

        Subcommands::Cache(cmd) => {
            _ = cache(cmd);
        }
    }

    // check if the version is up to date
    let remote_version = remote_version();
    let current_version = current_version();

    if remote_version.gt(&current_version) {
        let (logger, _) = Logger::new("TRACE");
        println!();
        logger.info("great news! An update is available!");
        logger
            .info(&format!("you can update now by running: `bifrost --version {remote_version}`"));
    }
}
