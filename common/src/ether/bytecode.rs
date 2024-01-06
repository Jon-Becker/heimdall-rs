use super::rpc::get_code;
use crate::{
    constants::{ADDRESS_REGEX, BYTECODE_REGEX},
    debug_max,
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
        debug_max!("using provided bytecode for snapshotting.");

        // Target is already a bytecode, so we just need to remove 0x from the begining
        Ok(target.replacen("0x", "", 1))
    } else {
        debug_max!("using provided file for snapshotting.");

        // Target is a file path, so we need to read the bytecode from the file.
        let contents = fs::read_to_string(target).map_err(|e| {
            logger.error(&format!("failed to open file '{}' .", &target));
            Error::FilesystemError(e)
        })?;

        let contents = contents.replace('\n', "");
        if BYTECODE_REGEX
            .is_match(&contents)
            .map_err(|e| Error::Generic(format!("failed to match bytecode regex: {}", e)))? &&
            contents.len() % 2 == 0
        {
            Ok(contents.replacen("0x", "", 1))
        } else {
            logger.error(&format!("file '{}' doesn't contain valid bytecode.", &target));
            return Err(Error::ParseError("file doesn't contain valid bytecode.".to_string()));
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
        .expect("failed to get bytecode from target");

        assert!(BYTECODE_REGEX.is_match(&bytecode).expect("failed to match bytecode regex"));
    }

    #[tokio::test]
    async fn test_get_bytecode_when_target_is_bytecode() {
        let bytecode = get_bytecode_from_target(
            "0x00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001",
            "https://rpc.ankr.com/eth",
        )
        .await
        .expect("failed to get bytecode from target");

        assert!(BYTECODE_REGEX.is_match(&bytecode).expect("failed to match bytecode regex"));
    }

    #[tokio::test]
    async fn test_get_bytecode_when_target_is_file_path() {
        let file_path = "./mock-file.txt";
        let mock_bytecode = "0x00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001";

        fs::write(file_path, mock_bytecode).expect("failed to write mock bytecode to file");

        let bytecode = get_bytecode_from_target(file_path, "https://rpc.ankr.com/eth")
            .await
            .expect("failed to get bytecode from target");

        assert!(BYTECODE_REGEX.is_match(&bytecode).expect("failed to match bytecode regex"));

        fs::remove_file(file_path).expect("failed to remove mock file");
    }
}
