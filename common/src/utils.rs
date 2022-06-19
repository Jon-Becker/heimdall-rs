
use std::num::ParseIntError;

use ethers::{prelude::{I256, U256}};

// Convert an unsigned integer into a signed one
pub fn sign_uint(unsigned: U256) -> I256 {
    return I256::from_raw(U256::from(unsigned))
}
pub fn decode_hex(s: &str) -> Result<Vec<u8>, ParseIntError> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16))
        .collect()
}
