use super::rpc::get_code;
use crate::{
    constants::{ADDRESS_REGEX, BYTECODE_REGEX},
    error::Error,
    utils::io::logging::Logger,
};
use std::fs;

pub async fn get_bytecode_from_target(target: &str, rpc_url: &str) -> Result<String, Error> {
    let (logger, _) = Logger::new("");

    if ADDRESS_REGEX
        .is_match(target)
        .map_err(|e| Error::Generic(format!("failed to match address regex: {}", e)))?
    {
        // Target is a contract address, so we need to fetch the bytecode from the RPC provider.
        get_code(target, rpc_url).await.map_err(|e| {
            Error::Generic(format!("failed to fetch bytecode from RPC provider: {}", e))
        })
    } else if BYTECODE_REGEX
        .is_match(target)
        .map_err(|e| Error::Generic(format!("failed to match bytecode regex: {}", e)))?
    {
        // Target is already a bytecode, so we just need to remove 0x from the begining
        Ok(target.replacen("0x", "", 1))
    } else {
        // Target is a file path, so we need to read the bytecode from the file.
        match fs::read_to_string(target) {
            Ok(contents) => {
                logger.debug(&format!("reading bytecode from '{}'", &target));

                let _contents = contents.replace('\n', "");
                if BYTECODE_REGEX
                    .is_match(&_contents)
                    .map_err(|e| Error::Generic(format!("failed to match bytecode regex: {}", e)))? &&
                    _contents.len() % 2 == 0
                {
                    Ok(_contents.replacen("0x", "", 1))
                } else {
                    logger.error(&format!("file '{}' doesn't contain valid bytecode.", &target));
                    std::process::exit(1)
                }
            }
            Err(_) => {
                logger.error(&format!("failed to open file '{}' .", &target));
                std::process::exit(1)
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[tokio::test]
    async fn test_get_bytecode_when_target_is_address() {
        let bytecode = get_bytecode_from_target(
            "0x9f00c43700bc0000Ff91bE00841F8e04c0495000",
            "https://rpc.ankr.com/eth",
        )
        .await
        .unwrap();

        assert!(BYTECODE_REGEX.is_match(&bytecode).unwrap());
    }

    #[tokio::test]
    async fn test_get_bytecode_when_target_is_bytecode() {
        let bytecode = get_bytecode_from_target(
            "0x00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001",
            "https://rpc.ankr.com/eth",
        )
        .await
        .unwrap();

        assert!(BYTECODE_REGEX.is_match(&bytecode).unwrap());
    }

    #[tokio::test]
    async fn test_get_bytecode_when_target_is_file_path() {
        let file_path = "./mock-file.txt";
        let mock_bytecode = "0x00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001";

        fs::write(file_path, mock_bytecode).unwrap();

        let bytecode =
            get_bytecode_from_target(file_path, "https://rpc.ankr.com/eth").await.unwrap();

        assert!(BYTECODE_REGEX.is_match(&bytecode).unwrap());

        fs::remove_file(file_path).unwrap();
    }
}
