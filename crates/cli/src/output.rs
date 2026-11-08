use std::{env, io::Write};

use eyre::{eyre, Result};
use heimdall_common::{
    ether::{bytecode::contract_address_target, calldata::transaction_hash_target, rpc},
    utils::io::file::write_file,
};

/// How a command interprets its target when choosing the default output directory.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum OutputTarget<'a> {
    Local,
    OnChain(&'a str),
}

impl<'a> OutputTarget<'a> {
    pub(crate) fn bytecode(target: &'a str, rpc_url: &str) -> Self {
        if contract_address_target(target, rpc_url).is_some() {
            Self::OnChain(target)
        } else {
            Self::Local
        }
    }

    pub(crate) fn calldata(target: &'a str, raw: bool) -> Self {
        if transaction_hash_target(target, raw).is_some() {
            Self::OnChain(target)
        } else {
            Self::Local
        }
    }
}

/// Destination and target context shared by command output writers.
pub(crate) struct Output<'a> {
    destination: &'a str,
    target: OutputTarget<'a>,
    rpc_url: &'a str,
}

impl<'a> Output<'a> {
    pub(crate) fn new(destination: &'a str, target: OutputTarget<'a>, rpc_url: &'a str) -> Self {
        Self { destination, target, rpc_url }
    }

    pub(crate) fn is_print(&self) -> bool {
        self.destination == "print"
    }

    pub(crate) async fn write(&self, filename: &str, contents: &str, label: &str) -> Result<()> {
        let path = build_output_path(self.destination, self.target, self.rpc_url, filename)
            .await
            .map_err(|e| eyre!("failed to build output path: {e}"))?;
        write_file(&path, contents).map_err(|e| eyre!("failed to write {label}: {e}"))
    }

    pub(crate) async fn emit(&self, filename: &str, contents: &str, label: &str) -> Result<()> {
        if self.is_print() {
            print_with_less(contents).await.map_err(|e| eyre!("failed to print {label}: {e}"))
        } else {
            self.write(filename, contents, label).await
        }
    }
}

pub(crate) fn named_file(name: &str, filename: &str) -> String {
    if name.is_empty() {
        filename.to_string()
    } else {
        format!("{name}-{filename}")
    }
}

/// build a standardized output path for the given parameters. follows the following cases:
/// - if `output` is the default value (`output`)
///   - on-chain targets use `/output/{chain_id}/{target}/{filename}`
///   - local targets use `/output/local/{filename}`
/// - if `output` is specified, return `/{output}/{filename}`
pub(crate) async fn build_output_path(
    output: &str,
    target: OutputTarget<'_>,
    rpc_url: &str,
    filename: &str,
) -> Result<String> {
    if output == "output" {
        let cwd = env::current_dir()?
            .into_os_string()
            .into_string()
            .map_err(|_| eyre!("Unable to get current working directory"))?;

        return match target {
            OutputTarget::OnChain(target) => {
                let chain_id =
                    rpc::chain_id(rpc_url).await.map_err(|_| eyre!("Unable to get chain id"))?;
                Ok(format!("{cwd}/output/{chain_id}/{target}/{filename}"))
            }
            OutputTarget::Local => Ok(format!("{cwd}/output/local/{filename}")),
        };
    }

    Ok(format!("{output}/{filename}"))
}

/// pass the input to the `less` command
pub(crate) async fn print_with_less(input: &str) -> Result<()> {
    let mut child =
        std::process::Command::new("less").stdin(std::process::Stdio::piped()).spawn()?;

    let stdin = child.stdin.as_mut().ok_or_else(|| eyre!("unable to get stdin for less"))?;
    stdin.write_all(input.as_bytes())?;

    child.wait()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires RPC_URL for Ethereum mainnet"]
    fn address_path_uses_rpc_chain_id() {
        const CHILD: &str = "HEIMDALL_OUTPUT_PATH_TEST_CHILD";
        if std::env::var_os(CHILD).is_some() {
            let rpc_url = std::env::var("RPC_URL").expect("RPC_URL must be set for this test");
            let runtime = tokio::runtime::Runtime::new().unwrap();
            let target = "0x0000000000000000000000000000000000000001";
            let path = runtime
                .block_on(build_output_path(
                    "output",
                    OutputTarget::OnChain(target),
                    &rpc_url,
                    "cfg.dot",
                ))
                .unwrap();
            assert!(path.ends_with(&format!("/output/1/{target}/cfg.dot")));
            return;
        }
        let home = std::env::temp_dir().join(format!("heimdall-output-rpc-{}", std::process::id()));
        std::fs::create_dir_all(&home).unwrap();
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--ignored", "--exact", "output::tests::address_path_uses_rpc_chain_id"])
            .env(CHILD, "1")
            .env("HOME", &home)
            .status()
            .unwrap();
        std::fs::remove_dir_all(home).unwrap();
        assert!(status.success());
    }

    #[tokio::test]
    async fn local_and_explicit_paths_do_not_require_rpc() {
        let local =
            build_output_path("output", OutputTarget::bytecode("0x6000", ""), "", "cfg.dot")
                .await
                .unwrap();
        assert!(local.ends_with("/output/local/cfg.dot"));
        let explicit = build_output_path(
            "/some_dir",
            OutputTarget::OnChain("0x0000000000000000000000000000000000000001"),
            "",
            "cfg.dot",
        )
        .await
        .unwrap();
        assert_eq!(explicit, "/some_dir/cfg.dot");
    }

    #[tokio::test]
    async fn raw_bytecode_and_calldata_use_local_output_paths() {
        let twenty_bytes = "0x0000000000000000000000000000000000000001";
        let thirty_two_bytes = "0x0000000000000000000000000000000000000000000000000000000000000000";
        let bytecode = OutputTarget::bytecode(twenty_bytes, "");
        let calldata = OutputTarget::calldata(thirty_two_bytes, true);
        assert!(build_output_path("output", bytecode, "", "disassembled.asm")
            .await
            .unwrap()
            .ends_with("/output/local/disassembled.asm"));
        assert!(build_output_path("output", calldata, "http://127.0.0.1:0", "decoded.json")
            .await
            .unwrap()
            .ends_with("/output/local/decoded.json"));
        assert_eq!(
            OutputTarget::bytecode(twenty_bytes, "http://rpc"),
            OutputTarget::OnChain(twenty_bytes)
        );
        assert_eq!(OutputTarget::bytecode(thirty_two_bytes, "http://rpc"), OutputTarget::Local);
        assert_eq!(
            OutputTarget::calldata(thirty_two_bytes, false),
            OutputTarget::OnChain(thirty_two_bytes)
        );
        assert_eq!(OutputTarget::calldata(twenty_bytes, false), OutputTarget::Local);
    }

    #[tokio::test]
    async fn output_writes_the_named_file() {
        let dir = std::env::temp_dir().join(format!("heimdall-output-{}", std::process::id()));
        let output = Output::new(dir.to_str().unwrap(), OutputTarget::bytecode("0x6000", ""), "");
        output.emit(&named_file("contract", "cfg.dot"), "digraph {}", "cfg").await.unwrap();
        assert_eq!(std::fs::read_to_string(dir.join("contract-cfg.dot")).unwrap(), "digraph {}");
        std::fs::remove_dir_all(dir).unwrap();
        assert_eq!(named_file("", "cfg.dot"), "cfg.dot");
    }
}
