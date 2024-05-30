use super::rpc::get_code;
use crate::{
    constants::{ADDRESS_REGEX, BYTECODE_REGEX},
    utils::strings::decode_hex,
    Error,
};
use ethers::types::Bytes;
use eyre::Result;
use std::fs;
use tracing::error;

/// Given a target, determines whether it is a contract address, bytecode, or file path, and returns
/// the bytecode for the target.
pub async fn get_bytecode_from_target(target: &str, rpc_url: &str) -> Result<Vec<u8>, Error> {
    if ADDRESS_REGEX.is_match(target).unwrap_or(false) {
        // Target is a contract address, so we need to fetch the bytecode from the RPC provider.
        get_code(target, rpc_url).await.map_err(|e| {
            Error::Generic(format!("failed to fetch bytecode from RPC provider: {}", e))
        })
    } else if BYTECODE_REGEX.is_match(target).unwrap_or(false) {
        Ok(decode_hex(target)?)
    } else {
        // Target is a file path, so we need to read the bytecode from the file.
        let contents = fs::read_to_string(target).map_err(|e| {
            error!("failed to open file '{}' .", &target);
            Error::FilesystemError(e)
        })?;

        let contents = contents.replace('\n', "");
        if BYTECODE_REGEX.is_match(&contents).unwrap_or(false) && contents.len() % 2 == 0 {
            Ok(decode_hex(&contents)?)
        } else {
            error!("file '{}' doesn't contain valid bytecode.", &target);
            return Err(Error::ParseError(format!(
                "file '{}' doesn't contain valid bytecode.",
                &target
            )));
        }
    }
}

/// Removes pushed bytes from the bytecode, leaving only the instructions
/// themselves.
///
/// For example:
///   0x6060 (PUSH1 0x60) would become 0x60 (PUSH1).
///   0x60806040 (PUSH1 0x60 PUSH1 0x40) would become 0x60 0x60 (PUSH1 PUSH1).
pub fn remove_pushbytes_from_bytecode(bytecode: Bytes) -> Result<Bytes> {
    let push_range = 0x5f..=0x7f;
    let mut pruned = Vec::new();

    let mut i = 0;
    while i < bytecode.len() {
        if push_range.contains(&bytecode[i]) {
            pruned.push(bytecode[i]);
            i += bytecode[i] as usize - 0x5f + 1;
        } else {
            pruned.push(bytecode[i]);
            i += 1;
        }
    }

    Ok(Bytes::from(pruned))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ethers::types::Bytes;
    use std::{fs, str::FromStr};

    #[test]
    fn test_remove_pushbytes_from_bytecode() {
        let bytecode = Bytes::from_str("0x6040").unwrap();
        let pruned = remove_pushbytes_from_bytecode(bytecode).unwrap();
        assert_eq!(pruned, Bytes::from_str("0x60").unwrap());

        let bytecode = Bytes::from_str("0x60406080").unwrap();
        let pruned = remove_pushbytes_from_bytecode(bytecode).unwrap();
        assert_eq!(pruned, Bytes::from_str("0x6060").unwrap());

        let bytecode = Bytes::from_str(
            "0x604060807f2222222222222222222222222222222222222222222222222222222222222222",
        )
        .unwrap();
        let pruned = remove_pushbytes_from_bytecode(bytecode).unwrap();
        assert_eq!(pruned, Bytes::from_str("0x60607f").unwrap());
    }

    #[tokio::test]
    async fn test_get_bytecode_when_target_is_address() {
        let bytecode = get_bytecode_from_target(
            "0x9f00c43700bc0000Ff91bE00841F8e04c0495000",
            "https://rpc.ankr.com/eth",
        )
        .await
        .expect("failed to get bytecode from target");

        assert!(!bytecode.is_empty());
    }

    #[tokio::test]
    async fn test_get_bytecode_when_target_is_bytecode() {
        let bytecode = get_bytecode_from_target(
            "0x00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001",
            "https://rpc.ankr.com/eth",
        )
        .await
        .expect("failed to get bytecode from target");

        assert!(!bytecode.is_empty());
    }

    #[tokio::test]
    async fn test_get_bytecode_when_target_is_file_path() {
        let file_path = "./mock-file.txt";
        let mock_bytecode = "0x00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001";

        fs::write(file_path, mock_bytecode).expect("failed to write mock bytecode to file");

        let bytecode = get_bytecode_from_target(file_path, "https://rpc.ankr.com/eth")
            .await
            .expect("failed to get bytecode from target");

        assert_eq!(bytecode.len(), 52);

        fs::remove_file(file_path).expect("failed to remove mock file");
    }
}
