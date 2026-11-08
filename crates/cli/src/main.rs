//! The Heimdall CLI is a command line interface for interacting with Heimdall modules.

pub(crate) mod args;
pub(crate) mod output;

use args::{Arguments, Subcommands};
use clap::Parser;
use eyre::{eyre, Result};
use heimdall_cache::cache;
use output::{named_file, print_with_less, Output, OutputTarget};
use tracing::info;

use heimdall_common::utils::{
    hex::ToLowerHex,
    version::{current_version, remote_nightly_version, remote_version},
};
use heimdall_config::{config, Configuration};
use heimdall_core::{
    heimdall_cfg::cfg, heimdall_decoder::decode, heimdall_decompiler::decompile,
    heimdall_disassembler::disassemble, heimdall_dump::dump, heimdall_inspect::inspect,
};

#[allow(clippy::large_stack_frames)]
#[tokio::main]
async fn main() -> Result<()> {
    let args = Arguments::parse();

    let _ = args.logs.init_tracing();

    let current_version = current_version();
    let nightly = current_version.is_nightly();
    let version_check = tokio::spawn(async move {
        if nightly {
            remote_nightly_version().await
        } else {
            remote_version().await
        }
    });

    let configuration =
        Configuration::load().map_err(|e| eyre!("failed to load configuration: {}", e))?;
    match args.sub {
        Subcommands::Disassemble(mut cmd) => {
            apply_default(&mut cmd.rpc_url, &configuration.rpc_url);
            let result = disassemble(cmd.clone())
                .await
                .map_err(|e| eyre!("failed to disassemble bytecode: {e}"))?;
            Output::new(
                &cmd.output,
                OutputTarget::bytecode(&cmd.target, &cmd.rpc_url),
                &cmd.rpc_url,
            )
            .emit(&named_file(&cmd.name, "disassembled.asm"), &result, "assembly")
            .await?;
        }
        Subcommands::Decompile(mut cmd) => {
            apply_default(&mut cmd.rpc_url, &configuration.rpc_url);
            apply_default(&mut cmd.openrouter_api_key, &configuration.openrouter_api_key);
            apply_default(&mut cmd.model, &configuration.openrouter_model);
            apply_default(&mut cmd.etherscan_api_key, &configuration.etherscan_api_key);
            let result = decompile(cmd.clone())
                .await
                .map_err(|e| eyre!("failed to decompile bytecode: {e}"))?;
            let output = Output::new(
                &cmd.output,
                OutputTarget::bytecode(&cmd.target, &cmd.rpc_url),
                &cmd.rpc_url,
            );
            if output.is_print() {
                let mut text = format!("ABI:\n\n{}\n", serde_json::to_string_pretty(&result.abi)?);
                if let Some(source) = &result.source {
                    text.push_str(&format!("Source:\n\n{source}\n"));
                }
                print_with_less(&text)
                    .await
                    .map_err(|e| eyre!("failed to print decompiled bytecode: {e}"))?;
            } else {
                output
                    .write(
                        &named_file(&cmd.name, "abi.json"),
                        &serde_json::to_string_pretty(&result.abi)?,
                        "ABI",
                    )
                    .await?;
                if let Some(source) = &result.source {
                    let filename =
                        if cmd.include_solidity { "decompiled.sol" } else { "decompiled.yul" };
                    output.write(&named_file(&cmd.name, filename), source, "source").await?;
                }
            }
        }
        Subcommands::Decode(mut cmd) => {
            apply_default(&mut cmd.rpc_url, &configuration.rpc_url);
            apply_default(&mut cmd.openrouter_api_key, &configuration.openrouter_api_key);
            apply_default(&mut cmd.model, &configuration.openrouter_model);
            let result =
                decode(cmd.clone()).await.map_err(|e| eyre!("failed to decode calldata: {e}"))?;
            let output = Output::new(
                &cmd.output,
                OutputTarget::calldata(&cmd.target, cmd.raw),
                &cmd.rpc_url,
            );
            if output.is_print() {
                result.display();
            } else {
                output.write("decoded.json", &result.to_json()?, "decoded output").await?;
            }
        }
        Subcommands::Cfg(mut cmd) => {
            apply_default(&mut cmd.rpc_url, &configuration.rpc_url);
            let result =
                cfg(cmd.clone()).await.map_err(|e| eyre!("failed to generate cfg: {e}"))?;
            Output::new(
                &cmd.output,
                OutputTarget::bytecode(&cmd.target, &cmd.rpc_url),
                &cmd.rpc_url,
            )
            .emit(&named_file(&cmd.name, "cfg.dot"), &result.as_dot(cmd.color_edges), "cfg")
            .await?;
        }
        Subcommands::Dump(mut cmd) => {
            apply_default(&mut cmd.rpc_url, &configuration.rpc_url);
            let result =
                dump(cmd.clone()).await.map_err(|e| eyre!("failed to dump storage: {e}"))?;
            let mut lines = vec![String::from("slot,value")];
            for (slot, value) in result {
                lines.push(format!("{},{}", slot.to_lower_hex(), value.to_lower_hex()));
            }
            Output::new(&cmd.output, OutputTarget::OnChain(&cmd.target), &cmd.rpc_url)
                .emit(&named_file(&cmd.name, "dump.csv"), &lines.join("\n"), "dump")
                .await?;
        }
        Subcommands::Inspect(mut cmd) => {
            apply_default(&mut cmd.rpc_url, &configuration.rpc_url);
            apply_default(&mut cmd.transpose_api_key, &configuration.transpose_api_key);
            let result = inspect(cmd.clone())
                .await
                .map_err(|e| eyre!("failed to inspect transaction: {e}"))?;
            result.display();
            let output = Output::new(&cmd.output, OutputTarget::OnChain(&cmd.target), &cmd.rpc_url);
            if output.is_print() {
                let text = format!(
                    "Decoded Trace:\n\n{}\n",
                    serde_json::to_string_pretty(&result.decoded_trace)?
                );
                print_with_less(&text)
                    .await
                    .map_err(|e| eyre!("failed to print decoded trace: {e}"))?;
            } else {
                output
                    .write(
                        &named_file(&cmd.name, "decoded_trace.json"),
                        &serde_json::to_string_pretty(&result.decoded_trace)?,
                        "decoded trace",
                    )
                    .await?;
            }
        }
        Subcommands::Config(cmd) => {
            config(cmd).map_err(|e| eyre!("failed to configure: {e}"))?;
        }
        Subcommands::Cache(cmd) => {
            cache(cmd).map_err(|e| eyre!("failed to manage cache: {e}"))?;
        }
    }

    if version_check.is_finished() {
        if let Ok(Ok(remote_ver)) = version_check.await {
            if current_version.is_nightly() && current_version.ne(&remote_ver) {
                info!("great news! A new nightly build is available!");
                info!("you can update now by running: `bifrost +nightly`");
            } else if remote_ver.gt(&current_version) {
                info!("great news! An update is available!");
                info!("you can update now by running: `bifrost --version {}`", remote_ver);
            }
        }
    }

    Ok(())
}

fn apply_default(value: &mut String, fallback: &str) {
    if value.is_empty() {
        value.push_str(fallback);
    }
}

#[cfg(test)]
mod tests {
    use super::apply_default;

    #[test]
    fn defaults_only_replace_empty_values() {
        let mut missing = String::new();
        apply_default(&mut missing, "configured");
        assert_eq!(missing, "configured");
        let mut explicit = String::from("explicit");
        apply_default(&mut explicit, "configured");
        assert_eq!(explicit, "explicit");
    }
}
