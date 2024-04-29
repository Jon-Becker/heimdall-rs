use std::{env, io::Write};

use heimdall_common::{
    constants::{ADDRESS_REGEX, TRANSACTION_HASH_REGEX},
    ether::rpc,
};

use crate::error::Error;

/// build a standardized output path for the given parameters. follows the following cases:
/// - if `output` is `print`, return `None`
/// - if `output` is the default value (`output`)
///   - if `target` is a contract_address, return `/output/{chain_id}/{target}/{filename}`
///   - if `target` is a file or raw bytes, return `/output/local/{filename}`
/// - if `output` is specified, return `/{output}/{filename}`
pub async fn build_output_path(
    output: &str,
    target: &str,
    rpc_url: &str,
    filename: &str,
) -> Result<String, Error> {
    // if output is the default value, build a path based on the target
    if output == "output" {
        // get the current working directory
        let cwd = env::current_dir()?
            .into_os_string()
            .into_string()
            .map_err(|_| Error::Generic("Unable to get current working directory".to_string()))?;

        if ADDRESS_REGEX.is_match(target).unwrap_or(false) ||
            TRANSACTION_HASH_REGEX.is_match(target).unwrap_or(false)
        {
            let chain_id = rpc::chain_id(rpc_url)
                .await
                .map_err(|_| Error::Generic("Unable to get chain id".to_string()))?;
            return Ok(format!("{}/output/{}/{}/{}", cwd, chain_id, target, filename))
        } else {
            return Ok(format!("{}/output/local/{}", cwd, filename))
        }
    }

    // output is specified, return the path
    Ok(format!("{}/{}", output, filename))
}

/// pass the input to the `less` command
pub async fn print_with_less(input: &str) -> Result<(), Error> {
    // let mut child =
    //     std::process::Command::new("less").stdin(std::process::Stdio::piped()).spawn()?;

    // let stdin = child
    //     .stdin
    //     .as_mut()
    //     .ok_or_else(|| Error::Generic("unable to get stdin for less".to_string()))?;
    // stdin.write_all(input.as_bytes())?;

    // child.wait()?;
    println!("{}", input);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_output_default_address() {
        let output = "output";
        let target = "0x0000000000000000000000000000000000000001";
        let rpc_url = "https://eth.llamarpc.com";
        let filename = "cfg.dot";

        let path = build_output_path(output, target, rpc_url, filename).await;
        assert!(path
            .expect("failed to build output path")
            .ends_with("/output/1/0x0000000000000000000000000000000000000001/cfg.dot"));
    }

    #[tokio::test]
    async fn test_output_default_local() {
        let output = "output";
        let target =
            "0x00000000000000000000000000000000000000000000000000000000000000000000000000000000";
        let rpc_url = "https://eth.llamarpc.com";
        let filename = "cfg.dot";

        let path = build_output_path(output, target, rpc_url, filename).await;
        assert!(path.expect("failed to build output path").ends_with("/output/local/cfg.dot"));
    }

    #[tokio::test]
    async fn test_output_specified() {
        let output = "/some_dir";
        let target = "0x0000000000000000000000000000000000000001";
        let rpc_url = "https://eth.llamarpc.com";
        let filename = "cfg.dot";

        let path = build_output_path(output, target, rpc_url, filename).await;
        assert_eq!(path.expect("failed to build output path"), "/some_dir/cfg.dot".to_string());
    }
}
