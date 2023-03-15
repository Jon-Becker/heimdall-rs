
use std::{num::ParseIntError};

use ethers::{prelude::{I256, U256}, abi::AbiEncode};

use crate::constants::REDUCE_HEX_REGEX;


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


// encode a hex into a string
pub fn encode_hex(s: Vec<u8>) -> String {
    s.iter()
        .map(|b| format!("{:02x}", b))
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

// convert a hex string to ascii
pub fn hex_to_ascii(s: &str) -> String {
    let mut result = String::new();
    for i in 0..s.len() / 2 {
        let byte = u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap();
        result.push(byte as char);
    }

    // remove newlines
    result = result.replace("\r", "");
    result = result.replace("\n", "");

    result
}


// replace the last occurrence of a string with a new string
pub fn replace_last(s: String, old: &str, new: &str) -> String {
    let new = new.chars().rev().collect::<String>();
    s.chars().rev().collect::<String>().replacen(old, &new, 1).chars().rev().collect::<String>()
}


// find balanced parentheses in a string
pub fn find_balanced_encapsulator(s: String, encap: (char, char)) -> (usize, usize, bool) {
    let mut open = 0;
    let mut close = 0;
    let mut start = 0;
    let mut end = 0;
    for (i, c) in s.chars().enumerate() {
        if c == encap.0 {
            if open == 0 {
                start = i;
            }
            open += 1;
        } else if c == encap.1 {
            close += 1;
        }
        if open == close && open > 0 {
            end = i;
            break;
        }
    }
    (start, end + 1, (open == close && end > start && open > 0))
}

// find balanced parentheses in a string, but backwards
pub fn find_balanced_encapsulator_backwards(s: String, encap: (char, char)) -> (usize, usize, bool) {
    let mut open = 0;
    let mut close = 0;
    let mut start = 0;
    let mut end = 0;
    for (i, c) in s.chars().rev().enumerate() {
        if c == encap.1 {
            if open == 0 {
                start = i;
            }
            open += 1;
        } else if c == encap.0 {
            close += 1;
        }
        if open == close && open > 0 {
            end = i;
            break;
        }
    }
    (s.len() - end - 1, s.len() - start, (open == close && end > start && open > 0))
}

// convert a number into it's base26 encoded form
pub fn base26_encode(n: usize) -> String {
    let mut s = String::new();
    let mut n = n;
    while n > 0 {
        n -= 1;
        s.push((b'A' + (n % 26) as u8) as char);
        n /= 26;
    }
    s.to_lowercase().chars().rev().collect()
}