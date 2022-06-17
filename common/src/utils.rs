use std::str::FromStr;

use ethers::{prelude::{I256, U256}, abi::AbiEncode};

pub fn sign_uint(unsigned: U256) -> I256 {
    match I256::from_str(&unsigned.encode_hex().as_str()) {
        Ok(signed) => signed,
        Err(_) => panic!("Parsing unsigned integer failed"),
    }
}