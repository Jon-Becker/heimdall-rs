pub(crate) mod error;
pub(crate) mod log_args;
pub(crate) mod output;

use error::Error;
use log_args::LogArgs;
use output::{build_output_path, print_with_less};
use tracing::{info, Level};

use clap::{Parser, Subcommand};

use heimdall_cache::{cache, CacheArgs};
use heimdall_common::utils::{
    io::file::{write_file, write_lines_to_file},
    version::{current_version, remote_version},
};
use heimdall_config::{config, ConfigArgs, Configuration};
use heimdall_core::{
    heimdall_cfg::{cfg, CFGArgs},
    heimdall_decoder::{decode, DecodeArgs},
    heimdall_disassembler::{disassemble, DisassemblerArgs},
    heimdall_dump::{dump, DumpArgs},
    heimdall_decompiler::{decompile, DecompilerArgs},
    heimdall_inspect::{inspect, InspectArgs},
    heimdall_snapshot::{snapshot, SnapshotArgs},
};

#[derive(Debug, Parser)]
#[clap(name = "heimdall", author = "Jonathan Becker <jonathan@jbecker.dev>", version)]
pub struct Arguments {
    #[clap(subcommand)]
    pub sub: Subcommands,

    #[clap(flatten)]
    logs: LogArgs,
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
        name = "inspect",
        about = "Detailed inspection of Ethereum transactions, including calldata & trace decoding, log visualization, and more"
    )]
    Inspect(InspectArgs),

    #[clap(
        name = "snapshot",
        about = "Infer function information from bytecode, including access control, gas
    consumption, storage accesses, event emissions, and more"
    )]
    Snapshot(SnapshotArgs),
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let args = Arguments::parse();

    // setup logging
    let _ = args.logs.init_tracing();

    let configuration = Configuration::load()
        .map_err(|e| Error::Generic(format!("failed to load configuration: {}", e)))?;
    match args.sub {
        Subcommands::Disassemble(mut cmd) => {
            // if the user has not specified a rpc url, use the default
            if cmd.rpc_url.as_str() == "" {
                cmd.rpc_url = configuration.rpc_url;
            }

            // if the user has passed an output filename, override the default filename
            let mut filename: String = "disassembled.asm".to_string();
            let given_name = cmd.name.as_str();

            if !given_name.is_empty() {
                filename = format!("{}-{}", given_name, filename);
            }

            let assembly = disassemble(cmd.clone())
                .await
                .map_err(|e| Error::Generic(format!("failed to disassemble bytecode: {}", e)))?;

            if cmd.output == "print" {
                print_with_less(&assembly)
                    .await
                    .map_err(|e| Error::Generic(format!("failed to print assembly: {}", e)))?;
            } else {
                let output_path =
                    build_output_path(&cmd.output, &cmd.target, &cmd.rpc_url, &filename)
                        .await
                        .map_err(|e| {
                            Error::Generic(format!("failed to build output path: {}", e))
                        })?;

                write_file(&output_path, &assembly)
                    .map_err(|e| Error::Generic(format!("failed to write assembly: {}", e)))?;
            }
        }

        Subcommands::Decompile(mut cmd) => {
            // if the user has not specified a rpc url, use the default
            if cmd.rpc_url.as_str() == "" {
                cmd.rpc_url = configuration.rpc_url;
            }

            // if the user has passed an output filename, override the default filename
            let mut abi_filename: String = "abi.json".to_string();
            let mut decompiled_output_filename: String = "decompiled".to_string();

            let given_name = cmd.name.as_str();

            if !given_name.is_empty() {
                abi_filename = format!("{}-{}", given_name, abi_filename);
                decompiled_output_filename =
                    format!("{}-{}", given_name, decompiled_output_filename);
            }

            let result = decompile(cmd.clone())
                .await
                .map_err(|e| Error::Generic(format!("failed to decompile bytecode: {}", e)))?;

            if args.logs.verbosity.level() >= Level::DEBUG {
                result.display();
            }

            if cmd.output == "print" {
                let mut output_str = String::new();
                output_str.push_str(&format!(
                    "ABI:\n\n[{}]\n",
                    serde_json::to_string_pretty(&result.abi).map_err(Error::SerdeError)?
                ));

                if let Some(source) = &result.source {
                    output_str.push_str(&format!("Source:\n\n{}\n", source));
                }

                print_with_less(&output_str).await.map_err(|e| {
                    Error::Generic(format!("failed to print decompiled bytecode: {}", e))
                })?;
            } else {
                // write the contract ABI
                let output_path =
                    build_output_path(&cmd.output, &cmd.target, &cmd.rpc_url, &abi_filename)
                        .await
                        .map_err(|e| {
                            Error::Generic(format!("failed to build output path: {}", e))
                        })?;

                write_file(
                    &output_path,
                    &serde_json::to_string_pretty(&result.abi).map_err(Error::SerdeError)?,
                )
                .map_err(|e| Error::Generic(format!("failed to write ABI: {}", e)))?;

                // write the contract source
                if let Some(source) = &result.source {
                    let output_path = if cmd.include_solidity {
                        build_output_path(
                            &cmd.output,
                            &cmd.target,
                            &cmd.rpc_url,
                            &format!("{}.sol", &decompiled_output_filename),
                        )
                        .await
                        .map_err(|e| {
                            Error::Generic(format!("failed to build output path: {}", e))
                        })?
                    } else {
                        build_output_path(
                            &cmd.output,
                            &cmd.target,
                            &cmd.rpc_url,
                            &format!("{}.yul", &decompiled_output_filename,),
                        )
                        .await
                        .map_err(|e| {
                            Error::Generic(format!("failed to build output path: {}", e))
                        })?
                    };
                    write_file(&output_path, source)
                        .map_err(|e| Error::Generic(format!("failed to write source: {}", e)))?;
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

            let result = decode(cmd)
                .await
                .map_err(|e| Error::Generic(format!("failed to decode calldata: {}", e)))?;

            result.display()
        }

        Subcommands::CFG(mut cmd) => {
            // if the user has not specified a rpc url, use the default
            if cmd.rpc_url.as_str() == "" {
                cmd.rpc_url = configuration.rpc_url;
            }

            // if the user has passed an output filename, override the default filename
            let mut filename = "cfg.dot".to_string();
            let given_name = cmd.name.as_str();

            if !given_name.is_empty() {
                filename = format!("{}-{}", given_name, filename);
            }
            let cfg = cfg(cmd.clone())
                .await
                .map_err(|e| Error::Generic(format!("failed to generate cfg: {}", e)))?;
            let stringified_dot = cfg.as_dot();

            if args.logs.verbosity.level() >= Level::DEBUG {
                cfg.display();
            }

            if cmd.output == "print" {
                print_with_less(&stringified_dot)
                    .await
                    .map_err(|e| Error::Generic(format!("failed to print cfg: {}", e)))?;
            } else {
                let output_path =
                    build_output_path(&cmd.output, &cmd.target, &cmd.rpc_url, &filename)
                        .await
                        .map_err(|e| {
                            Error::Generic(format!("failed to build output path: {}", e))
                        })?;
                write_file(&output_path, &stringified_dot)
                    .map_err(|e| Error::Generic(format!("failed to write cfg: {}", e)))?;
            }
        }

        Subcommands::Dump(mut cmd) => {
            // if the user has not specified a rpc url, use the default
            if cmd.rpc_url.as_str() == "" {
                cmd.rpc_url = configuration.rpc_url;
            }

            // if the user has passed an output filename, override the default filename
            let mut filename = "dump.csv".to_string();
            let given_name = cmd.name.as_str();

            if !given_name.is_empty() {
                filename = format!("{}-{}", given_name, filename);
            }

            // if the user has not specified a transpose api key, use the default
            if cmd.transpose_api_key.as_str() == "" {
                cmd.transpose_api_key = configuration.transpose_api_key;
            }

            let result = dump(cmd.clone())
                .await
                .map_err(|e| Error::Generic(format!("failed to dump storage: {}", e)))?;
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
                print_with_less(&lines.join("\n"))
                    .await
                    .map_err(|e| Error::Generic(format!("failed to print dump: {}", e)))?;
            } else {
                let output_path =
                    build_output_path(&cmd.output, &cmd.target, &cmd.rpc_url, &filename)
                        .await
                        .map_err(|e| {
                            Error::Generic(format!("failed to build output path: {}", e))
                        })?;

                write_lines_to_file(&output_path, lines)
                    .map_err(|e| Error::Generic(format!("failed to write dump: {}", e)))?;
            }
        }

        Subcommands::Snapshot(mut cmd) => {
            // if the user has not specified a rpc url, use the default
            if cmd.rpc_url.as_str() == "" {
                cmd.rpc_url = configuration.rpc_url;
            }

            // if the user has passed an output filename, override the default filename
            let mut filename = "snapshot.csv".to_string();
            let given_name = cmd.name.as_str();

            if !given_name.is_empty() {
                filename = format!("{}-{}", given_name, filename);
            }

            let snapshot_result = snapshot(cmd.clone())
                .await
                .map_err(|e| Error::Generic(format!("failed to snapshot contract: {}", e)))?;
            let csv_lines = snapshot_result.generate_csv();

            if args.logs.verbosity.level() >= Level::DEBUG {
                snapshot_result.display();
            }

            if cmd.output == "print" {
                print_with_less(&csv_lines.join("\n"))
                    .await
                    .map_err(|e| Error::Generic(format!("failed to print snapshot: {}", e)))?;
            } else {
                let output_path =
                    build_output_path(&cmd.output, &cmd.target, &cmd.rpc_url, &filename)
                        .await
                        .map_err(|e| {
                            Error::Generic(format!("failed to build output path: {}", e))
                        })?;

                write_lines_to_file(&output_path, csv_lines)
                    .map_err(|e| Error::Generic(format!("failed to write snapshot: {}", e)))?;
            }
        }

        Subcommands::Inspect(mut cmd) => {
            // if the user has not specified a rpc url, use the default
            if cmd.rpc_url.as_str() == "" {
                cmd.rpc_url = configuration.rpc_url;
            }

            // if the user has not specified a transpose api key, use the default
            if cmd.transpose_api_key.is_empty() {
                cmd.transpose_api_key = configuration.transpose_api_key;
            }

            // if the user has passed an output filename, override the default filename
            let mut filename = "decoded_trace.json".to_string();
            let given_name = cmd.name.as_str();

            if !given_name.is_empty() {
                filename = format!("{}-{}", given_name, filename);
            }

            let inspect_result = inspect(cmd.clone())
                .await
                .map_err(|e| Error::Generic(format!("failed to inspect transaction: {}", e)))?;
            inspect_result.display();

            if cmd.output == "print" {
                let mut output_str = String::new();

                if let Some(decoded_trace) = inspect_result.decoded_trace {
                    output_str.push_str(&format!(
                        "Decoded Trace:\n\n{}\n",
                        serde_json::to_string_pretty(&decoded_trace)?
                    ));
                }

                print_with_less(&output_str)
                    .await
                    .map_err(|e| Error::Generic(format!("failed to print decoded trace: {}", e)))?;
            } else if let Some(decoded_trace) = inspect_result.decoded_trace {
                // write decoded trace with serde
                let output_path =
                    build_output_path(&cmd.output, &cmd.target, &cmd.rpc_url, &filename)
                        .await
                        .map_err(|e| {
                            Error::Generic(format!("failed to build output path: {}", e))
                        })?;

                write_file(&output_path, &serde_json::to_string_pretty(&decoded_trace)?)
                    .map_err(|e| Error::Generic(format!("failed to write decoded trace: {}", e)))?;
            }
        }

        Subcommands::Config(cmd) => {
            config(cmd).map_err(|e| Error::Generic(format!("failed to configure: {}", e)))?;
        }

        Subcommands::Cache(cmd) => {
            cache(cmd).map_err(|e| Error::Generic(format!("failed to manage cache: {}", e)))?;
        }
    }

    // check if the version is up to date
    let remote_version = remote_version()
        .await
        .map_err(|e| Error::Generic(format!("failed to get remote version: {}", e)))?;
    let current_version = current_version();

    if remote_version.gt(&current_version) {
        info!("great news! An update is available!");
        info!("you can update now by running: `bifrost --version {}`", remote_version);
    }

    Ok(())
}
