
use std::{num::ParseIntError};

use ethers::{prelude::{I256, U256}, abi::AbiEncode};

use crate::consts::REDUCE_HEX_REGEX;


// Convert an unsigned integer into a signed one
pub fn sign_uint(unsigned: U256) -> I256 {
    return I256::from_raw(U256::from(unsigned))
}


// decode a hex into an array of integer values
pub fn decode_hex(s: &str) -> Result<Vec<u8>, ParseIntError> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16))
        .collect()
}


// convert a U256 to hex without leading 0s
pub fn encode_hex_reduced(s: U256) -> String {

    if s > U256::from(0) {
        REDUCE_HEX_REGEX.replace(&s.clone().encode_hex(), "0x").to_string()
    }
    else {
        String::from("0")
    }
}


// replace the last occurrence of a string with a new string
pub fn replace_last(s: String, old: &str, new: &str) -> String {
    let new = new.chars().rev().collect::<String>();
    s.chars().rev().collect::<String>().replacen(old, &new, 1).chars().rev().collect::<String>()
}