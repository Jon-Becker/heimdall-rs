use std::{fmt::Write, num::ParseIntError};

use ethers::{
    abi::AbiEncode,
    prelude::{I256, U256},
};
use fancy_regex::Regex;

use crate::constants::REDUCE_HEX_REGEX;

/// Converts a signed integer into an unsigned integer
pub fn sign_uint(unsigned: U256) -> I256 {
    I256::from_raw(unsigned)
}

/// Decodes a hex string into a vector of bytes
///
/// ```
/// use heimdall_common::utils::strings::decode_hex;
///
/// let hex = "48656c6c6f20576f726c64"; // "Hello World" in hex
/// let result = decode_hex(hex);
/// assert_eq!(result, Ok(vec![72, 101, 108, 108, 111, 32, 87, 111, 114, 108, 100]));
/// ```
pub fn decode_hex(s: &str) -> Result<Vec<u8>, ParseIntError> {
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16)).collect()
}

/// Encodes a vector of bytes into a hex string
///
/// ```
/// use heimdall_common::utils::strings::encode_hex;
///
/// let bytes = vec![72, 101, 108, 108, 111, 32, 87, 111, 114, 108, 100];
/// let result = encode_hex(bytes);
/// assert_eq!(result, "48656c6c6f20576f726c64");
/// ```
pub fn encode_hex(s: Vec<u8>) -> String {
    s.iter().fold(String::new(), |mut acc, b| {
        write!(acc, "{b:02x}", b = b).unwrap();
        acc
    })
}

/// Encodes a U256 into a hex string, removing leading zeros
///
/// ```
/// use ethers::types::U256;
/// use heimdall_common::utils::strings::encode_hex_reduced;
///
/// let u256 = U256::max_value();
/// let result = encode_hex_reduced(u256);
/// assert_eq!(result, "0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff");
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
/// ```
/// use heimdall_common::utils::strings::hex_to_ascii;
///
/// let hex = "48656c6c6f20576f726c64"; // "Hello World" in hex
/// let result = hex_to_ascii(hex);
/// assert_eq!(result, "Hello World");
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
/// ```
/// use heimdall_common::utils::strings::replace_last;
///
/// let s = "Hello, world!";
/// let old = "o";
/// let new = "0";
/// let result = replace_last(s, old, new);
/// assert_eq!(result, String::from("Hello, w0rld!"));
/// ```
pub fn replace_last(s: &str, old: &str, new: &str) -> String {
    let new = new.chars().rev().collect::<String>();
    s.chars().rev().collect::<String>().replacen(old, &new, 1).chars().rev().collect::<String>()
}

/// Finds balanced encapsulator in a string
///
/// ```
/// use heimdall_common::utils::strings::find_balanced_encapsulator;
///
/// let s = "Hello (World)";
/// let result = find_balanced_encapsulator(s, ('(', ')'));
/// assert_eq!(result, (6, 13, true));
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
/// ```
/// use heimdall_common::utils::strings::find_balanced_encapsulator_backwards;
///
/// let s = "Hello (World)";
/// let result = find_balanced_encapsulator_backwards(s, ('(', ')'));
/// assert_eq!(result, (6, 13, true));
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
/// ```
/// use heimdall_common::utils::strings::base26_encode;
///
/// let n = 123456789;
/// let result = base26_encode(n);
/// assert_eq!(result, "jjddja");
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
/// ```
/// use heimdall_common::utils::strings::extract_condition;
///
/// let s = "require(a == b)";
/// let result = extract_condition(s, "require");
/// assert_eq!(result, Some("a == b".to_string()));
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
/// ```
/// use heimdall_common::utils::strings::tokenize;
///
/// let s = "a + b * c";
/// let result = tokenize(s);
/// assert_eq!(result, vec!["a", "+", "b", "*", "c"]);
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

/// Returns a collapsed version of a string if this string is greater than 66 characters in length.
/// The collapsed string consists of the first 66 characters, followed by an ellipsis ("..."), and
/// then the last 16 characters of the original string. ```
/// use heimdall_common::utils::strings::get_shortned_target;
///
/// let long_target = "0".repeat(80);
/// let shortened_target = get_shortned_target(&long_target);
/// ```
pub fn get_shortned_target(target: &str) -> String {
    let mut shortened_target = target.to_string();

    if shortened_target.len() > 66 {
        shortened_target = shortened_target.chars().take(66).collect::<String>() +
            "..." +
            &shortened_target.chars().skip(shortened_target.len() - 16).collect::<String>();
    }

    shortened_target
}

#[cfg(test)]
mod tests {
    use ethers::types::{I256, U256};

    use crate::utils::strings::*;

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
        let s = "Hello, world!";
        let old = "o";
        let new = "0";
        let result = replace_last(s, old, new);
        assert_eq!(result, String::from("Hello, w0rld!"));

        let s = "Hello, world!";
        let old = "l";
        let new = "L";
        let result = replace_last(s, old, new);
        assert_eq!(result, String::from("Hello, worLd!"));
    }

    #[test]
    fn test_find_balanced_encapsulator() {
        let s = String::from("This is (an example) string.");
        let encap = ('(', ')');
        let (start, end, is_balanced) = find_balanced_encapsulator(&s, encap);
        assert_eq!(start, 8);
        assert_eq!(end, 20);
        assert!(is_balanced);

        let s = String::from("This is an example) string.");
        let encap = ('(', ')');
        let (start, end, is_balanced) = find_balanced_encapsulator(&s, encap);
        assert_eq!(start, 0);
        assert_eq!(end, 1);
        assert!(!is_balanced);

        let s = String::from("This is (an example string.");
        let encap = ('(', ')');
        let (start, end, is_balanced) = find_balanced_encapsulator(&s, encap);
        assert_eq!(start, 8);
        assert_eq!(end, 1);
        assert!(!is_balanced);
    }

    #[test]
    fn test_find_balanced_encapsulator_backwards() {
        let s = String::from("This is (an example) string.");
        let encap = ('(', ')');
        let (start, end, is_balanced) = find_balanced_encapsulator_backwards(&s, encap);
        assert_eq!(start, 8);
        assert_eq!(end, 20);
        assert!(is_balanced);

        let s = String::from("This is an example) string.");
        let encap = ('(', ')');
        let (_, _, is_balanced) = find_balanced_encapsulator_backwards(&s, encap);
        assert!(!is_balanced);

        let s = String::from("This is (an example string.");
        let encap = ('(', ')');
        let (_, _, is_balanced) = find_balanced_encapsulator_backwards(&s, encap);
        assert!(!is_balanced);
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

    #[test]
    fn test_extract_condition_present_balanced() {
        let s = "require(arg0 == (address(arg0)));";
        let keyword = "require";
        let expected = Some("arg0 == (address(arg0))".to_string());
        assert_eq!(extract_condition(s, keyword), expected);
    }

    #[test]
    fn test_extract_condition_present_unbalanced() {
        let s = "require(arg0 == (address(arg0));";
        let keyword = "require";
        let expected = None;
        assert_eq!(extract_condition(s, keyword), expected);
    }

    #[test]
    fn test_extract_condition_not_present() {
        let s = "if (0x01 < var_c.length) {";
        let keyword = "require";
        let expected = None;
        assert_eq!(extract_condition(s, keyword), expected);
    }

    #[test]
    fn test_extract_condition_multiple_keywords() {
        let s = "require(var_c.length == var_c.length, \"some revert message\");";
        let keyword = "require";
        let expected = Some("var_c.length == var_c.length".to_string());
        assert_eq!(extract_condition(s, keyword), expected);
    }

    #[test]
    fn test_extract_condition_empty_string() {
        let s = "";
        let keyword = "require";
        let expected = None;
        assert_eq!(extract_condition(s, keyword), expected);
    }

    #[test]
    fn test_tokenize_basic_operators() {
        let tokens = tokenize("arg0 + arg1");
        assert_eq!(tokens, vec!["arg0", "+", "arg1"]);
    }

    #[test]
    fn test_tokenize_parentheses_and_operators() {
        let tokens = tokenize("(arg0 + arg1) > (msg.value + 1)");
        assert_eq!(
            tokens,
            vec!["(", "arg0", "+", "arg1", ")", ">", "(", "msg.value", "+", "1", ")"]
        );
    }

    #[test]
    fn test_tokenize_multiple_operators() {
        let tokens = tokenize("a >= b && c != d");
        assert_eq!(tokens, vec!["a", ">=", "b", "&&", "c", "!=", "d"]);
    }

    #[test]
    fn test_tokenize_no_spaces() {
        let tokens = tokenize("a+b-c*d/e");
        assert_eq!(tokens, vec!["a", "+", "b", "-", "c", "*", "d", "/", "e"]);
    }

    #[test]
    fn test_tokenize_whitespace_only() {
        let tokens = tokenize("    ");
        assert_eq!(tokens, Vec::<String>::new());
    }

    #[test]
    fn test_tokenize_empty_string() {
        let tokens = tokenize("");
        assert_eq!(tokens, Vec::<String>::new());
    }

    #[test]
    fn test_tokenize_complex_expression() {
        let tokens = tokenize("if (x > 10 && y < 20) || z == 0 { a = b + c }");
        assert_eq!(
            tokens,
            vec![
                "if", "(", "x", ">", "10", "&&", "y", "<", "20", ")", "||", "z", "==", "0", "{",
                "a", "=", "b", "+", "c", "}"
            ]
        );
    }

    #[test]
    fn test_tokenize_separators_at_start_and_end() {
        let tokens = tokenize("==text==");
        assert_eq!(tokens, vec!["==", "text", "=="]);
    }

    #[test]
    fn test_classify_token_parenthesis() {
        let classification = classify_token("(");
        assert_eq!(classification, TokenType::Control);

        let classification = classify_token(")");
        assert_eq!(classification, TokenType::Control);
    }

    #[test]
    fn test_classify_token_operators_precedence_1() {
        for operator in ["+", "-"].iter() {
            let classification = classify_token(operator);
            assert_eq!(classification, TokenType::Operator);
        }
    }

    #[test]
    fn test_classify_token_operators_precedence_2() {
        for operator in
            ["*", "/", "%", "|", "&", "^", "==", ">=", "<=", "!=", "!", "&&", "||"].iter()
        {
            let classification = classify_token(operator);
            assert_eq!(classification, TokenType::Operator);
        }
    }

    #[test]
    fn test_classify_token_constant() {
        let classification = classify_token("0x001234567890");
        assert_eq!(classification, TokenType::Constant);
    }

    #[test]
    fn test_classify_token_variable() {
        for variable in [
            "memory[0x01]",
            "storage",
            "var",
            "msg.value",
            "block.timestamp",
            "this.balance",
            "tx.origin",
            "arg0",
            "ret",
            "calldata",
            "abi.encode",
        ]
        .iter()
        {
            let classification = classify_token(variable);
            assert_eq!(classification, TokenType::Variable);
        }
    }

    #[test]
    fn test_classify_token_function() {
        for function in ["uint256", "address", "ecrecover", "if"].iter() {
            let classification = classify_token(function);
            assert_eq!(classification, TokenType::Function);
        }
    }

    #[test]
    fn test_shorten_long_target() {
        let long_target = "0".repeat(80);
        let shortened_target = get_shortned_target(&long_target);

        assert_eq!(shortened_target.len(), 85);
    }

    #[test]
    fn test_shorten_short_target() {
        let short_target = "0".repeat(66);
        let shortened_target = get_shortned_target(&short_target);

        assert_eq!(shortened_target.len(), 66);
    }
}
