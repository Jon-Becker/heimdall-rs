pub(crate) mod args;
pub(crate) mod output;

use args::{Arguments, Subcommands};
use clap::Parser;
use eyre::{eyre, Result};
use heimdall_cache::cache;
use output::{build_output_path, print_with_less};
use tracing::info;

use heimdall_common::utils::{
    hex::ToLowerHex,
    io::file::write_file,
    version::{current_version, remote_nightly_version, remote_version},
};
use heimdall_config::{config, Configuration};
use heimdall_core::{
    heimdall_cfg::cfg, heimdall_decoder::decode, heimdall_decompiler::decompile,
    heimdall_disassembler::disassemble, heimdall_dump::dump, heimdall_inspect::inspect,
};

#[tokio::main]
async fn main() -> Result<()> {
    let args = Arguments::parse();

    // setup logging
    let _ = args.logs.init_tracing();

    // spawn a new tokio runtime to get remote version while the main runtime is running
    let current_version = current_version();
    let remote_ver = if current_version.is_nightly() {
        tokio::task::spawn(remote_nightly_version()).await??
    } else {
        tokio::task::spawn(remote_version()).await??
    };

    let configuration =
        Configuration::load().map_err(|e| eyre!("failed to load configuration: {}", e))?;
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
                .map_err(|e| eyre!("failed to disassemble bytecode: {}", e))?;

            if cmd.output == "print" {
                print_with_less(&assembly)
                    .await
                    .map_err(|e| eyre!("failed to print assembly: {}", e))?;
            } else {
                let output_path =
                    build_output_path(&cmd.output, &cmd.target, &cmd.rpc_url, &filename)
                        .await
                        .map_err(|e| eyre!("failed to build output path: {}", e))?;

                write_file(&output_path, &assembly)
                    .map_err(|e| eyre!("failed to write assembly: {}", e))?;
            }
        }

        Subcommands::Decompile(mut cmd) => {
            // if the user has not specified a rpc url, use the default
            if cmd.rpc_url.as_str() == "" {
                cmd.rpc_url = configuration.rpc_url;
            }

            // if the user has not specified a openai api key, use the default
            if cmd.openai_api_key.as_str() == "" {
                cmd.openai_api_key = configuration.openai_api_key;
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
                .map_err(|e| eyre!("failed to decompile bytecode: {}", e))?;

            if cmd.output == "print" {
                let mut output_str = String::new();
                output_str.push_str(&format!(
                    "ABI:\n\n[{}]\n",
                    serde_json::to_string_pretty(&result.abi)?
                ));

                if let Some(source) = &result.source {
                    output_str.push_str(&format!("Source:\n\n{}\n", source));
                }

                print_with_less(&output_str)
                    .await
                    .map_err(|e| eyre!("failed to print decompiled bytecode: {}", e))?;
            } else {
                // write the contract ABI
                let output_path =
                    build_output_path(&cmd.output, &cmd.target, &cmd.rpc_url, &abi_filename)
                        .await
                        .map_err(|e| eyre!("failed to build output path: {}", e))?;

                write_file(&output_path, &serde_json::to_string_pretty(&result.abi)?)
                    .map_err(|e| eyre!("failed to write ABI: {}", e))?;

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
                        .map_err(|e| eyre!("failed to build output path: {}", e))?
                    } else {
                        build_output_path(
                            &cmd.output,
                            &cmd.target,
                            &cmd.rpc_url,
                            &format!("{}.yul", &decompiled_output_filename,),
                        )
                        .await
                        .map_err(|e| eyre!("failed to build output path: {}", e))?
                    };
                    write_file(&output_path, source)
                        .map_err(|e| eyre!("failed to write source: {}", e))?;
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

            let result =
                decode(cmd).await.map_err(|e| eyre!("failed to decode calldata: {}", e))?;

            result.display()
        }

        Subcommands::Cfg(mut cmd) => {
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
            let cfg = cfg(cmd.clone()).await.map_err(|e| eyre!("failed to generate cfg: {}", e))?;
            let stringified_dot = cfg.as_dot(cmd.color_edges);

            if cmd.output == "print" {
                print_with_less(&stringified_dot)
                    .await
                    .map_err(|e| eyre!("failed to print cfg: {}", e))?;
            } else {
                let output_path =
                    build_output_path(&cmd.output, &cmd.target, &cmd.rpc_url, &filename)
                        .await
                        .map_err(|e| eyre!("failed to build output path: {}", e))?;
                write_file(&output_path, &stringified_dot)
                    .map_err(|e| eyre!("failed to write cfg: {}", e))?;
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

            let result =
                dump(cmd.clone()).await.map_err(|e| eyre!("failed to dump storage: {}", e))?;
            let mut lines = Vec::new();

            // add header
            lines.push(String::from("slot,value"));

            // add rows
            for (slot, value) in result {
                lines.push(format!("{},{}", slot.to_lower_hex(), value.to_lower_hex()));
            }

            if cmd.output == "print" {
                print_with_less(&lines.join("\n"))
                    .await
                    .map_err(|e| eyre!("failed to print dump: {}", e))?;
            } else {
                let output_path =
                    build_output_path(&cmd.output, &cmd.target, &cmd.rpc_url, &filename)
                        .await
                        .map_err(|e| eyre!("failed to build output path: {}", e))?;

                write_file(&output_path, &lines.join("\n"))
                    .map_err(|e| eyre!("failed to write dump: {}", e))?;
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
                .map_err(|e| eyre!("failed to inspect transaction: {}", e))?;
            inspect_result.display();

            if cmd.output == "print" {
                let mut output_str = String::new();

                output_str.push_str(&format!(
                    "Decoded Trace:\n\n{}\n",
                    serde_json::to_string_pretty(&inspect_result.decoded_trace)?
                ));

                print_with_less(&output_str)
                    .await
                    .map_err(|e| eyre!("failed to print decoded trace: {}", e))?;
            } else {
                // write decoded trace with serde
                let output_path =
                    build_output_path(&cmd.output, &cmd.target, &cmd.rpc_url, &filename)
                        .await
                        .map_err(|e| eyre!("failed to build output path: {}", e))?;

                write_file(
                    &output_path,
                    &serde_json::to_string_pretty(&inspect_result.decoded_trace)?,
                )
                .map_err(|e| eyre!("failed to write decoded trace: {}", e))?;
            }
        }

        Subcommands::Config(cmd) => {
            config(cmd).map_err(|e| eyre!("failed to configure: {}", e))?;
        }

        Subcommands::Cache(cmd) => {
            cache(cmd).map_err(|e| eyre!("failed to manage cache: {}", e))?;
        }
    }

    // check if the version is up to date
    if current_version.is_nightly() && current_version.ne(&remote_ver) {
        info!("great news! A new nightly build is available!");
        info!("you can update now by running: `bifrost +nightly`");
    } else if remote_ver.gt(&current_version) {
        info!("great news! An update is available!");
        info!("you can update now by running: `bifrost --version {}`", remote_ver);
    }

    Ok(())
}
