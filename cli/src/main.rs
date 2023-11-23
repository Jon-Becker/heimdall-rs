pub(crate) mod output;

use backtrace::Backtrace;
use output::build_output_path;
use std::{io, panic};

use clap::{Parser, Subcommand};
use colored::Colorize;
use crossterm::{
    event::DisableMouseCapture,
    execute,
    terminal::{disable_raw_mode, LeaveAlternateScreen},
};

use heimdall_cache::{cache, CacheArgs};
use heimdall_common::utils::{
    io::{
        file::{write_file, write_lines_to_file},
        logging::Logger,
    },
    version::{current_version, remote_version},
};
use heimdall_config::{config, get_config, ConfigArgs};
use heimdall_core::{
    cfg::{cfg, output::build_cfg, CFGArgs},
    decode::{decode, DecodeArgs},
    decompile::{decompile, out::abi::ABIStructure, DecompilerArgs},
    disassemble::{disassemble, DisassemblerArgs},
    dump::{dump, DumpArgs},
    snapshot::{snapshot, util::csv::generate_csv, SnapshotArgs},
};
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
    #[clap(
        name = "snapshot",
        about = "Infer functiogn information from bytecode, including access control, gas
    consumption, storage accesses, event emissions, and more"
    )]
    Snapshot(SnapshotArgs),
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Arguments::parse();
    // handle catching panics with
    panic::set_hook(Box::new(|panic_info| {
        // cleanup the terminal (break out of alternate screen, disable mouse capture, and show the
        // cursor)
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

            let assembly = disassemble(cmd.clone()).await?;

            if cmd.output == "print" {
                // TODO: use `less`
                println!("{}", assembly);
            } else {
                let output_path =
                    build_output_path(&cmd.output, &cmd.target, &cmd.rpc_url, "disassembled.asm")
                        .await?;

                write_file(&output_path, &assembly);
            }
        }

        Subcommands::Decompile(mut cmd) => {
            // if the user has not specified a rpc url, use the default
            if cmd.rpc_url.as_str() == "" {
                cmd.rpc_url = configuration.rpc_url;
            }

            let result = decompile(cmd.clone()).await?;

            if cmd.output == "print" {
                if let Some(abi) = &result.abi {
                    println!("ABI:\n\n{}\n", serde_json::to_string_pretty(abi).unwrap());
                }
                if let Some(source) = &result.source {
                    println!("Source:\n\n{}\n", source);
                }
            } else {
                // write the contract ABI
                if let Some(abi) = result.abi {
                    let output_path =
                        build_output_path(&cmd.output, &cmd.target, &cmd.rpc_url, "abi.json")
                            .await?;

                    write_file(
                        &output_path,
                        &format!(
                            "[{}]",
                            abi.iter()
                                .map(|x| {
                                    match x {
                                        ABIStructure::Function(x) => {
                                            serde_json::to_string_pretty(x).unwrap()
                                        }
                                        ABIStructure::Error(x) => {
                                            serde_json::to_string_pretty(x).unwrap()
                                        }
                                        ABIStructure::Event(x) => {
                                            serde_json::to_string_pretty(x).unwrap()
                                        }
                                    }
                                })
                                .collect::<Vec<String>>()
                                .join(",\n")
                        ),
                    );
                }

                // write the contract source
                if let Some(source) = &result.source {
                    let output_path = if cmd.include_solidity {
                        build_output_path(&cmd.output, &cmd.target, &cmd.rpc_url, "decompiled.sol")
                            .await?
                    } else {
                        build_output_path(&cmd.output, &cmd.target, &cmd.rpc_url, "decompiled.yul")
                            .await?
                    };
                    write_file(&output_path, source);
                }
            }
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

            // set cmd.verbose to 6
            cmd.verbose = clap_verbosity_flag::Verbosity::new(5, 0);

            let _ = decode(cmd).await;
        }

        Subcommands::CFG(mut cmd) => {
            // if the user has not specified a rpc url, use the default
            if cmd.rpc_url.as_str() == "" {
                cmd.rpc_url = configuration.rpc_url;
            }

            let cfg = cfg(cmd.clone()).await?;
            let stringified_dot = build_cfg(&cfg, &cmd);

            if cmd.output == "print" {
                println!("{}", stringified_dot);
            } else {
                let output_path =
                    build_output_path(&cmd.output, &cmd.target, &cmd.rpc_url, "cfg.dot").await?;
                write_file(&output_path, &stringified_dot);
            }
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

            let result = dump(cmd.clone()).await?;
            let mut lines = Vec::new();

            // add header
            lines.push(String::from("last_modified,alias,slot,decoded_type,value"));

            // add rows
            for row in result {
                lines.push(format!(
                    "{},{},{},{},{}",
                    row.last_modified, row.alias, row.slot, row.decoded_type, row.value
                ));
            }

            if cmd.output == "print" {
                for line in &lines {
                    println!("{}", line);
                }
            } else {
                let output_path =
                    build_output_path(&cmd.output, &cmd.target, &cmd.rpc_url, "dump.csv").await?;

                write_lines_to_file(&output_path, lines);
            }
        }

        Subcommands::Snapshot(mut cmd) => {
            // if the user has not specified a rpc url, use the default
            if cmd.rpc_url.as_str() == "" {
                cmd.rpc_url = configuration.rpc_url;
            }

            let snapshot_result = snapshot(cmd.clone()).await?;
            let csv_lines = generate_csv(
                &snapshot_result.snapshots,
                &snapshot_result.resolved_errors,
                &snapshot_result.resolved_events,
            );

            if cmd.output == "print" {
                for line in &csv_lines {
                    println!("{}", line);
                }
            } else {
                let output_path =
                    build_output_path(&cmd.output, &cmd.target, &cmd.rpc_url, "snapshot.csv")
                        .await?;

                write_lines_to_file(&output_path, csv_lines);
            }
        }

        Subcommands::Config(cmd) => {
            config(cmd);
        }

        Subcommands::Cache(cmd) => {
            _ = cache(cmd);
        }
    }

    // check if the version is up to date
    let remote_version = remote_version().await;
    let current_version = current_version();

    if remote_version.gt(&current_version) {
        let (logger, _) = Logger::new("TRACE");
        println!();
        logger.info("great news! An update is available!");
        logger
            .info(&format!("you can update now by running: `bifrost --version {remote_version}`"));
    }

    Ok(())
}
