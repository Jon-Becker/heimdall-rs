use super::rpc::get_code;
use crate::{
    constants::{ADDRESS_REGEX, BYTECODE_REGEX},
    utils::io::logging::Logger,
};
use std::fs;

pub async fn get_contract_bytecode(
    target: &str,
    rpc_url: &str,
    logger: &Logger,
) -> Result<String, Box<dyn std::error::Error>> {
    if ADDRESS_REGEX.is_match(target)? {
        // We are snapshotting a contract address, so we need to fetch the bytecode from the RPC
        // provider.
        get_code(target, rpc_url).await
    } else if BYTECODE_REGEX.is_match(target)? {
        logger.debug_max("using provided bytecode for snapshotting.");
        Ok(target.replacen("0x", "", 1))
    } else {
        logger.debug_max("using provided file for snapshotting.");

        // We are snapshotting a file, so we need to read the bytecode from the file.
        match fs::read_to_string(target) {
            Ok(contents) => {
                let _contents = contents.replace('\n', "");
                if BYTECODE_REGEX.is_match(&_contents)? && _contents.len() % 2 == 0 {
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
    use std::fs;

    use crate::utils::io::logging::get_logger_and_trace;

    use super::*;
    use fancy_regex::Regex;

    #[tokio::test]
    async fn test_get_bytecode_when_target_is_address() {
        let bytecode_regex = Regex::new(r"^[0-9a-fA-F]{0,50000}$").unwrap();
        let (logger, _) = get_logger_and_trace(&clap_verbosity_flag::Verbosity::new(-1, 0));
        let bytecode = get_contract_bytecode(
            "0x9f00c43700bc0000Ff91bE00841F8e04c0495000",
            "https://eth.llamarpc.com",
            &logger,
        )
        .await
        .unwrap();

        assert!(bytecode_regex.is_match(&bytecode).unwrap());
        // Not possible to express with regex since fancy_regex
        // doesn't support look-arounds
        assert!(!bytecode.starts_with("0x"));
    }

    #[tokio::test]
    async fn test_get_bytecode_when_target_is_bytecode() {
        let bytecode_regex = Regex::new(r"^[0-9a-fA-F]{0,50000}$").unwrap();
        let (logger, _) = get_logger_and_trace(&clap_verbosity_flag::Verbosity::new(-1, 0));
        let bytecode = get_contract_bytecode(
            "0x00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001",
            "https://eth.llamarpc.com",
            &logger,
        ).await.unwrap();

        assert!(bytecode_regex.is_match(&bytecode).unwrap());
        assert!(!bytecode.starts_with("0x"));
    }

    #[tokio::test]
    async fn test_get_bytecode_when_target_is_file_path() {
        let (logger, _) = get_logger_and_trace(&clap_verbosity_flag::Verbosity::new(-1, 0));
        let bytecode_regex = Regex::new(r"^[0-9a-fA-F]{0,50000}$").unwrap();
        let file_path = "./mock-file.txt";
        let mock_bytecode = "0x00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001";

        fs::write(file_path, mock_bytecode).unwrap();

        let bytecode =
            get_contract_bytecode(file_path, "https://eth.llamarpc.com", &logger).await.unwrap();

        assert!(bytecode_regex.is_match(&bytecode).unwrap());
        assert!(!bytecode.starts_with("0x"));

        fs::remove_file(file_path).unwrap();
    }
}
