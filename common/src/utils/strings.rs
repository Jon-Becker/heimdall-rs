use std::num::ParseIntError;

use ethers::{
    abi::AbiEncode,
    prelude::{I256, U256},
};
use fancy_regex::Regex;

use crate::constants::REDUCE_HEX_REGEX;

// Convert an unsigned integer into a signed one
pub fn sign_uint(unsigned: U256) -> I256 {
    I256::from_raw(unsigned)
}

// decode a hex into an array of integer values
pub fn decode_hex(s: &str) -> Result<Vec<u8>, ParseIntError> {
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16)).collect()
}

// encode a hex into a string
pub fn encode_hex(s: Vec<u8>) -> String {
    s.iter().map(|b| format!("{b:02x}")).collect()
}

// convert a U256 to hex without leading 0s
pub fn encode_hex_reduced(s: U256) -> String {
    if s > U256::from(0) {
        REDUCE_HEX_REGEX.replace(&s.encode_hex(), "0x").to_string()
    } else {
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
    result = result.replace('\r', "");
    result = result.replace('\n', "");

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
            break
        }
    }
    (start, end + 1, (open == close && end > start && open > 0))
}

// find balanced parentheses in a string, but backwards
pub fn find_balanced_encapsulator_backwards(
    s: String,
    encap: (char, char),
) -> (usize, usize, bool) {
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
            break
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

// splits a string by a given regex
pub fn split_string_by_regex(input: &str, pattern: Regex) -> Vec<String> {
    // Find all matches of the pattern in the input string
    let matches = pattern.find_iter(input);

    // Use the matches to split the input string into substrings
    let mut substrings = vec![];
    let mut last_end = 0;
    for m in matches {
        let m = m.unwrap();
        let start = m.start();
        let end = m.end();
        if start > last_end {
            substrings.push(input[last_end..start].to_string());
        }
        last_end = end;
    }
    if last_end < input.len() {
        substrings.push(input[last_end..].to_string());
    }

    // Return the resulting vector of substrings
    substrings
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sign_uint() {
        let unsigned = U256::from(10);
        let signed = sign_uint(unsigned);
        assert_eq!(signed, I256::from(10));

        let unsigned = U256::from(0);
        let signed = sign_uint(unsigned);
        assert_eq!(signed, I256::from(0));

        let unsigned = U256::from(1000);
        let signed = sign_uint(unsigned);
        assert_eq!(signed, I256::from(1000));
    }

    #[test]
    fn test_decode_hex() {
        let hex = "48656c6c6f20776f726c64"; // "Hello world"
        let result = decode_hex(hex);
        assert_eq!(result, Ok(vec![72, 101, 108, 108, 111, 32, 119, 111, 114, 108, 100]));

        let hex = "abcdef";
        let result = decode_hex(hex);
        assert_eq!(result, Ok(vec![171, 205, 239]));

        let hex = "012345";
        let result = decode_hex(hex);
        assert_eq!(result, Ok(vec![1, 35, 69]));
    }

    #[test]
    fn test_encode_hex() {
        let bytes = vec![72, 101, 108, 108, 111, 32, 119, 111, 114, 108, 100]; // "Hello world"
        let result = encode_hex(bytes);
        assert_eq!(result, "48656c6c6f20776f726c64");

        let bytes = vec![171, 205, 239];
        let result = encode_hex(bytes);
        assert_eq!(result, "abcdef");

        let bytes = vec![1, 35, 69];
        let result = encode_hex(bytes);
        assert_eq!(result, "012345");
    }

    #[test]
    fn test_encode_hex_reduced() {
        let hex = U256::from(10);
        let result = encode_hex_reduced(hex);
        assert_eq!(result, "0x0a");

        let hex = U256::from(0);
        let result = encode_hex_reduced(hex);
        assert_eq!(result, "0");

        let hex = U256::from(1000);
        let result = encode_hex_reduced(hex);
        assert_eq!(result, "0x03e8");
    }

    #[test]
    fn test_hex_to_ascii() {
        let hex = "48656c6c6f20776f726c64"; // "Hello world"
        let result = hex_to_ascii(hex);
        assert_eq!(result, "Hello world");

        let hex = "616263646566"; // "abcdef"
        let result = hex_to_ascii(hex);
        assert_eq!(result, "abcdef");

        let hex = "303132333435"; // "012345"
        let result = hex_to_ascii(hex);
        assert_eq!(result, "012345");
    }

    #[test]
    fn test_replace_last() {
        let s = String::from("Hello, world!");
        let old = "o";
        let new = "0";
        let result = replace_last(s, old, new);
        assert_eq!(result, String::from("Hello, w0rld!"));

        let s = String::from("Hello, world!");
        let old = "l";
        let new = "L";
        let result = replace_last(s, old, new);
        assert_eq!(result, String::from("Hello, worLd!"));
    }

    #[test]
    fn test_find_balanced_encapsulator() {
        let s = String::from("This is (an example) string.");
        let encap = ('(', ')');
        let (start, end, is_balanced) = find_balanced_encapsulator(s, encap);
        assert_eq!(start, 8);
        assert_eq!(end, 20);
        assert_eq!(is_balanced, true);

        let s = String::from("This is an example) string.");
        let encap = ('(', ')');
        let (start, end, is_balanced) = find_balanced_encapsulator(s, encap);
        assert_eq!(start, 0);
        assert_eq!(end, 1);
        assert_eq!(is_balanced, false);

        let s = String::from("This is (an example string.");
        let encap = ('(', ')');
        let (start, end, is_balanced) = find_balanced_encapsulator(s, encap);
        assert_eq!(start, 8);
        assert_eq!(end, 1);
        assert_eq!(is_balanced, false);
    }

    #[test]
    fn test_find_balanced_encapsulator_backwards() {
        let s = String::from("This is (an example) string.");
        let encap = ('(', ')');
        let (start, end, is_balanced) = find_balanced_encapsulator_backwards(s, encap);
        assert_eq!(start, 8);
        assert_eq!(end, 20);
        assert_eq!(is_balanced, true);

        let s = String::from("This is an example) string.");
        let encap = ('(', ')');
        let (_, _, is_balanced) = find_balanced_encapsulator_backwards(s, encap);
        assert_eq!(is_balanced, false);

        let s = String::from("This is (an example string.");
        let encap = ('(', ')');
        let (_, _, is_balanced) = find_balanced_encapsulator_backwards(s, encap);
        assert_eq!(is_balanced, false);
    }

    #[test]
    fn test_base26_encode() {
        let n = 1;
        let result = base26_encode(n);
        assert_eq!(result, "a");

        let n = 26;
        let result = base26_encode(n);
        assert_eq!(result, "z");

        let n = 27;
        let result = base26_encode(n);
        assert_eq!(result, "aa");

        let n = 703;
        let result = base26_encode(n);
        assert_eq!(result, "aaa");
    }

    #[test]
    fn test_split_string_by_regex() {
        let input = "Hello,world!";
        let pattern = fancy_regex::Regex::new(r",").unwrap();
        let result = split_string_by_regex(input, pattern);
        assert_eq!(result, vec!["Hello", "world!"]);

        let input = "This is a test.";
        let pattern = fancy_regex::Regex::new(r"\s").unwrap();
        let result = split_string_by_regex(input, pattern);
        assert_eq!(result, vec!["This", "is", "a", "test."]);

        let input = "The quick brown fox jumps over the lazy dog.";
        let pattern = fancy_regex::Regex::new(r"\s+").unwrap();
        let result = split_string_by_regex(input, pattern);
        assert_eq!(
            result,
            vec!["The", "quick", "brown", "fox", "jumps", "over", "the", "lazy", "dog."]
        );
    }
}
