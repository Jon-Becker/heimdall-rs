use std::num::ParseIntError;

use ethers::{
    abi::AbiEncode,
    prelude::{I256, U256},
};
use fancy_regex::Regex;

use crate::constants::REDUCE_HEX_REGEX;

/// Converts a signed integer into an unsigned integer
///
/// ## Arguments
/// signed: I256 - the signed integer to convert
///
/// ## Returns
/// U256 - the unsigned integer
///
/// ## Example
/// ```no_run
/// use ethers::prelude::{I256, U256};
/// use heimdall::utils::strings::unsign_int;
///
/// let signed = I256::from(-1);
/// let unsigned = unsign_int(signed);
///
/// assert_eq!(unsigned, U256::from(0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff));
/// ```
pub fn sign_uint(unsigned: U256) -> I256 {
    I256::from_raw(unsigned)
}

/// Decodes a hex string into a vector of bytes
///
/// ## Arguments
/// s: &str - the hex string to decode
///
/// ## Returns
/// Result<Vec<u8>, ParseIntError> - the decoded vector of bytes
///
/// ## Example
/// ```no_run
/// use heimdall::utils::strings::decode_hex;
///
/// let decoded = decode_hex("00010203");
/// assert_eq!(decoded, Ok(vec![0, 1, 2, 3]));
/// ```
pub fn decode_hex(s: &str) -> Result<Vec<u8>, ParseIntError> {
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16)).collect()
}

/// Encodes a vector of bytes into a hex string
///
/// ## Arguments
/// s: Vec<u8> - the vector of bytes to encode
///
/// ## Returns
/// String - the encoded hex string
///
/// ## Example
/// ```no_run
/// use heimdall::utils::strings::encode_hex;
///
/// let encoded = encode_hex(vec![0, 1, 2, 3]);
/// assert_eq!(encoded, String::from("00010203"));
/// ```
pub fn encode_hex(s: Vec<u8>) -> String {
    s.iter().map(|b| format!("{b:02x}")).collect()
}

/// Encodes a U256 into a hex string, removing leading zeros
///
/// ## Arguments
/// s: U256 - the U256 to encode
///
/// ## Returns
/// String - the encoded hex string
///
/// ## Example
/// ```no_run
/// use ethers::prelude::U256;
/// use heimdall::utils::strings::encode_hex_reduced;
///
/// let encoded = encode_hex_reduced(U256::from(0));
/// assert_eq!(encoded, String::from("0"));
///
/// let encoded = encode_hex_reduced(U256::from(1));
/// assert_eq!(encoded, String::from("0x01"));
/// ```
pub fn encode_hex_reduced(s: U256) -> String {
    if s > U256::from(0) {
        REDUCE_HEX_REGEX.replace(&s.encode_hex(), "0x").to_string()
    } else {
        String::from("0")
    }
}

/// Converts a hex string to an ASCII string
///
/// ## Arguments
/// s: &str - the hex string to convert
///
/// ## Returns
/// String - the ASCII string
///
/// ## Example
/// ```no_run
/// use heimdall::utils::strings::hex_to_ascii;
///
/// let ascii = hex_to_ascii("0x68656c6c6f20776f726c64");
/// assert_eq!(ascii, String::from("hello world"));
/// ```
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

/// Replaces the last occurrence of a substring in a string
///
/// ## Arguments
/// s: String - the string to search
/// old: &str - the substring to replace
/// new: &str - the substring to replace with
///
/// ## Returns
/// String - the resulting string
///
/// ## Example
/// ```no_run
/// use heimdall::utils::strings::replace_last;
///
/// let replaced = replace_last(String::from("arg0 + arg1"), "arg1", "arg2");
/// assert_eq!(replaced, String::from("arg0 + arg2"));
///
/// let replaced = replace_last(String::from("arg0 + arg1 + arg1"), "arg1", "arg2");
/// assert_eq!(replaced, String::from("arg0 + arg1 + arg2"));
/// ```
pub fn replace_last(s: String, old: &str, new: &str) -> String {
    let new = new.chars().rev().collect::<String>();
    s.chars().rev().collect::<String>().replacen(old, &new, 1).chars().rev().collect::<String>()
}

/// Finds balanced encapsulator in a string
///
/// ## Arguments
/// s: String - the string to search
/// encap: (char, char) - the encapsulator to search for
///
/// ## Returns
/// (usize, usize, bool) - the start and end indices of the balanced encapsulator, and whether or
/// not it was found
///
/// ## Example
/// ```no_run
/// use heimdall::utils::strings::find_balanced_encapsulator;
///
/// let (start, end, is_balanced) = find_balanced_encapsulator(String::from("arg0 + arg1"), ('(', ')'));
/// assert_eq!(start, 0);
/// assert_eq!(end, 9);
/// assert_eq!(is_balanced, true);
/// ```
pub fn find_balanced_encapsulator(s: &str, encap: (char, char)) -> (usize, usize, bool) {
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

/// Finds balanced parentheses in a string, starting from the end
///
/// ## Arguments
/// s: String - the string to search
/// encap: (char, char) - the encapsulator to search for
///
/// ## Returns
/// (usize, usize, bool) - the start and end indices of the balanced parentheses, and whether or not
/// they were found
///
/// ## Example
/// ```no_run
/// use heimdall::utils::strings::find_balanced_encapsulator_backwards;
///
/// let (start, end, is_balanced) = find_balanced_encapsulator_backwards(String::from("arg0 + arg1"), ('(', ')'));
/// assert_eq!(start, 0);
/// assert_eq!(end, 9);
/// assert_eq!(is_balanced, true);
/// ```
pub fn find_balanced_encapsulator_backwards(s: &str, encap: (char, char)) -> (usize, usize, bool) {
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

/// Encodes a number into a base26 string
///
/// ## Arguments
/// n: usize - the number to encode
///
/// ## Returns
/// String - the encoded string
///
/// ## Example
/// ```no_run
/// use heimdall::utils::strings::base26_encode;
///
/// let encoded = base26_encode(0);
/// assert_eq!(encoded, String::from("a"));
///
/// let encoded = base26_encode(25);
/// assert_eq!(encoded, String::from("z"));
///
/// let encoded = base26_encode(26);
/// assert_eq!(encoded, String::from("aa"));
/// ```
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

/// Splits a string by a regular expression
///
/// ## Arguments
/// input: &str - the string to split
/// pattern: Regex - the regular expression to split by
///
/// ## Returns
/// Vec<String> - the vector of substrings
///
/// ## Example
/// ```no_run
/// use fancy_regex::Regex;
/// use heimdall::utils::strings::split_string_by_regex;
///
/// let pattern = Regex::new(r"\s+").unwrap();
/// let substrings = split_string_by_regex("arg0 + arg1", pattern);
/// assert_eq!(substrings, vec!["arg0", "+", "arg1"]);
/// ```
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

/// Extracts the condition from a require() or if() statement
///
/// ## Arguments
/// s: &str - the string to extract the condition from
/// keyword: &str - the keyword to search for, either "require" or "if"
///
/// ## Returns
/// Option<String> - the extracted condition, if found
///
/// ## Example
/// ```no_run
/// use heimdall::utils::strings::extract_condition;
///
/// let condition = extract_condition("require(arg0 > 0)", "require");
/// assert_eq!(condition, Some(String::from("arg0 > 0")));
/// ```
///
/// ## Example 2
/// ```no_run
/// use heimdall::utils::strings::extract_condition;
///
/// let condition = extract_condition("if (arg0 > 0) {", "if");
/// assert_eq!(condition, Some(String::from("arg0 > 0")));
/// ```
pub fn extract_condition(s: &str, keyword: &str) -> Option<String> {
    // find the keyword
    if let Some(start) = s.find(keyword) {
        // slice the string after the keyword
        let sliced = s[start + keyword.len()..].to_string();

        // find the balanced encapsulator
        let (start, end, is_balanced) = find_balanced_encapsulator(&sliced, ('(', ')'));

        // extract the condition if balanced encapsulator is found
        if is_balanced {
            let mut condition = &sliced[start + 1..end - 1];

            // require() statements can include revert messages or error codes
            if condition.contains(", ") {
                condition = condition.split(", ").collect::<Vec<&str>>()[0];
            }

            return Some(condition.trim().to_string())
        }
    }

    None
}

/// Tokenizes an expression into a vector of tokens
///
/// ## Arguments
/// s: &str - the expression to tokenize
///
/// ## Returns
/// Vec<String> - the vector of tokens
///
/// ## Example
/// ```no_run
/// use heimdall::utils::strings::tokenize;
///
/// let tokens = tokenize("arg0 + arg1");
/// assert_eq!(tokens, vec!["arg0", "+", "arg1"]);
///
/// let tokens = tokenize("(arg0 + arg1) > (msg.value + 1)");
/// assert_eq!(tokens, vec!["(", "arg0", "+", "arg1", ")", ">", "(", "msg.value", "+", "1", ")"]);
///
/// let tokens = tokenize("if (arg0 >= 0) {");
/// assert_eq!(tokens, vec!["if", "(", "arg0", ">=", "0", ")", "{"]);
/// ```
pub fn tokenize(s: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut token = String::new();

    // List of characters that should be treated as separate tokens
    let separators = ['(', ')', '+', '-', '*', '/', '=', '>', '<', '!', '&', '|', ';', '%', '^'];

    // List of characters that can be part of a two-character operator
    let compound_operator_first_chars = ['=', '>', '<', '!', '&', '|'];

    // Helper variable to keep track of the last character
    let mut last_char = None;

    // Iterate over each character in the input string
    for c in s.chars() {
        // If the current character is a separator or a whitespace
        if separators.contains(&c) || c.is_whitespace() {
            // If the current token is not empty, push it to the vector
            if !token.is_empty() {
                tokens.push(token.clone());
                token.clear();
            }

            // Check if current character and last character form a compound operator (like "==",
            // ">=", "&&", "||")
            if let Some(last) = last_char {
                if compound_operator_first_chars.contains(&last) &&
                    (c == '=' || c == '&' || c == '|')
                {
                    // Remove the last character as a single token
                    tokens.pop();
                    // Add the compound operator as a single token
                    tokens.push(format!("{}{}", last, c));
                } else if separators.contains(&c) {
                    tokens.push(c.to_string());
                }
            } else if separators.contains(&c) {
                tokens.push(c.to_string());
            }
        } else {
            // Append the current character to the current token
            token.push(c);
        }

        // Update last_char for the next iteration
        if !c.is_whitespace() {
            last_char = Some(c);
        } else {
            last_char = None;
        }
    }

    // If there is a token at the end of the string, add it to the vector
    if !token.is_empty() {
        tokens.push(token);
    }

    tokens
}

#[derive(Debug, PartialEq)]
pub enum TokenType {
    Control,
    Operator,
    Constant,
    Variable,
    Function,
}

/// Classifies a token as a variable, constant, operator, or function call, and returns its
/// precedence
///
/// ## Arguments
/// token: &str - the token to classify
///
/// ## Returns
/// (String, usize) - the token's classification, and precedence
///
/// ## Example
/// ```no_run
/// use heimdall::utils::strings::classify_token;
///
/// let (classification, precedence) = classify_token("0x01");
/// assert_eq!(classification, TokenType::Constant);
/// assert_eq!(precedence, 0);
///
/// let (classification, precedence) = classify_token("arg0");
/// assert_eq!(classification, TokenType::Variable);
/// assert_eq!(precedence, 0);
///
/// let (classification, precedence) = classify_token("+");
/// assert_eq!(classification, TokenType::Operator);
/// assert_eq!(precedence, 1);
///
/// let (classification, precedence) = classify_token("*");
/// assert_eq!(classification, TokenType::Operator);
/// assert_eq!(precedence, 2);
///
/// let (classification, precedence) = classify_token(">");
/// assert_eq!(classification, TokenType::Operator);
/// assert_eq!(precedence, 2);
///
/// let (classification, precedence) = classify_token("==");
/// assert_eq!(classification, TokenType::Operator);
/// assert_eq!(precedence, 2);
///
/// let (classification, precedence) = classify_token("memory[0x01]");
/// assert_eq!(classification, TokenType::Variable);
///
/// let (classification, precedence) = classify_token("uint256");
/// assert_eq!(classification, TokenType::Function);
///
/// let (classification, precedence) = classify_token("keccak256");
/// assert_eq!(classification, Token::Function);
pub fn classify_token(token: &str) -> TokenType {
    // return if the token is a parenthesis
    if token == "(" || token == ")" {
        return TokenType::Control
    }

    // check if the token is an operator
    let operators = ['+', '-', '*', '/', '=', '>', '<', '!', '&', '|', '%', '^'];
    if token.chars().all(|c| operators.contains(&c)) {
        return TokenType::Operator
    }

    // check if the token is a constant
    if token.starts_with("0x") || token.parse::<U256>().is_ok() {
        return TokenType::Constant
    }

    // check if the token is a variable
    if [
        "memory", "storage", "var", "msg.", "block.", "this.", "tx.", "arg", "ret", "calldata",
        "abi.",
    ]
    .iter()
    .any(|keyword| token.contains(keyword))
    {
        return TokenType::Variable
    }

    // this token must be a function call
    TokenType::Function
}
