use crate::utils::strings::decode_hex;

use super::rpc::get_code;
use alloy::primitives::{bytes::Bytes, Address};
use eyre::{eyre, Result};
use std::fs;

/// Given a target from the CLI, return bytecode of the target.
/// TODO: this can probably be a trait method so we can do something like target.try_get_bytecode()
/// TODO: move to CLI, since its only used in CLI
pub async fn get_bytecode_from_target(target: &str, rpc_url: &str) -> Result<Vec<u8>> {
    // If the target is not an address, it could be bytecode or a file path.
    if let Ok(bytecode) = decode_hex(target) {
        return Ok(bytecode);
    }

    // If the target is an address, fetch the bytecode from the RPC provider.
    if let Ok(address) = target.parse::<Address>() {
        return get_code(address, rpc_url).await;
    }

    // Assuming the target is a file path.
    match fs::read_to_string(target) {
        Ok(contents) => {
            let cleaned_contents = contents.replace('\n', "");
            decode_hex(&cleaned_contents)
                .map_err(|_| eyre!("invalid target: file does not contain valid bytecode"))
        }
        Err(_) => Err(eyre!("invalid target")),
    }
}

/// Removes pushed bytes from the bytecode, leaving only the instructions
/// themselves.
///
/// For example:
///   0x6060 (PUSH1 0x60) would become 0x60 (PUSH1).
///   0x60806040 (PUSH1 0x60 PUSH1 0x40) would become 0x60 0x60 (PUSH1 PUSH1).
pub fn remove_pushbytes_from_bytecode(bytecode: alloy::primitives::Bytes) -> Result<Bytes> {
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
    use alloy::hex::FromHex;

    use super::*;
    use std::fs;

    #[test]
    fn test_remove_pushbytes_from_bytecode() {
        let bytecode = alloy::primitives::Bytes::from_hex("0x6040").expect("invalid");
        let pruned = remove_pushbytes_from_bytecode(bytecode).unwrap();
        assert_eq!(pruned.to_vec(), alloy::primitives::Bytes::from_hex("0x60").expect("invalid"));

        let bytecode = alloy::primitives::Bytes::from_hex("0x60406080").expect("invalid");
        let pruned = remove_pushbytes_from_bytecode(bytecode).unwrap();
        assert_eq!(pruned.to_vec(), alloy::primitives::Bytes::from_hex("0x6060").expect("invalid"));

        let bytecode = alloy::primitives::Bytes::from_hex(
            "0x604060807f2222222222222222222222222222222222222222222222222222222222222222",
        )
        .expect("invalid");
        let pruned = remove_pushbytes_from_bytecode(bytecode).unwrap();
        assert_eq!(
            pruned.to_vec(),
            alloy::primitives::Bytes::from_hex("0x60607f").expect("invalid")
        );
    }

    #[tokio::test]
    async fn test_get_bytecode_when_target_is_address() {
        let rpc_url = std::env::var("RPC_URL").unwrap_or_else(|_| {
            println!("RPC_URL not set, skipping test");
            std::process::exit(0);
        });

        let bytecode =
            get_bytecode_from_target("0x9f00c43700bc0000Ff91bE00841F8e04c0495000", &rpc_url)
                .await
                .expect("failed to get bytecode from target");

        assert!(!bytecode.is_empty());
    }

    #[tokio::test]
    async fn test_get_bytecode_when_target_is_bytecode() {
        let rpc_url = std::env::var("RPC_URL").unwrap_or_else(|_| {
            println!("RPC_URL not set, skipping test");
            std::process::exit(0);
        });

        let bytecode = get_bytecode_from_target(
            "0x00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001",
            &rpc_url,
        )
        .await
        .expect("failed to get bytecode from target");

        assert!(!bytecode.is_empty());
    }

    #[tokio::test]
    async fn test_get_bytecode_when_target_is_file_path() {
        let rpc_url = std::env::var("RPC_URL").unwrap_or_else(|_| {
            println!("RPC_URL not set, skipping test");
            std::process::exit(0);
        });

        let file_path = "./mock-file.txt";
        let mock_bytecode = "0x00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001";

        fs::write(file_path, mock_bytecode).expect("failed to write mock bytecode to file");

        let bytecode = get_bytecode_from_target(file_path, &rpc_url)
            .await
            .expect("failed to get bytecode from target");

        assert_eq!(bytecode.len(), 52);

        fs::remove_file(file_path).expect("failed to remove mock file");
    }
}
