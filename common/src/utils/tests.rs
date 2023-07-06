#[cfg(test)]
mod test_integers {
    use crate::utils::integers::ToLocaleString;

    #[test]
    fn test_to_locale_string() {
        // Test case: Single-digit number
        let num = 5;
        let expected = "5".to_string();
        assert_eq!(num.to_locale_string(), expected);

        // Test case: Three-digit number
        let num = 123;
        let expected = "123".to_string();
        assert_eq!(num.to_locale_string(), expected);

        // Test case: Four-digit number
        let num = 1234;
        let expected = "1,234".to_string();
        assert_eq!(num.to_locale_string(), expected);

        // Test case: Five-digit number
        let num = 12345;
        let expected = "12,345".to_string();
        assert_eq!(num.to_locale_string(), expected);

        // Test case: Six-digit number
        let num = 123456;
        let expected = "123,456".to_string();
        assert_eq!(num.to_locale_string(), expected);

        // Test case: Seven-digit number
        let num = 1234567;
        let expected = "1,234,567".to_string();
        assert_eq!(num.to_locale_string(), expected);

        // Test case: Eight-digit number
        let num = 12345678;
        let expected = "12,345,678".to_string();
        assert_eq!(num.to_locale_string(), expected);

        // Test case: Nine-digit number
        let num = 123456789;
        let expected = "123,456,789".to_string();
        assert_eq!(num.to_locale_string(), expected);
    }
}

#[cfg(test)]
mod test_strings {
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
        let (start, end, is_balanced) = find_balanced_encapsulator(&s, encap);
        assert_eq!(start, 8);
        assert_eq!(end, 20);
        assert_eq!(is_balanced, true);

        let s = String::from("This is an example) string.");
        let encap = ('(', ')');
        let (start, end, is_balanced) = find_balanced_encapsulator(&s, encap);
        assert_eq!(start, 0);
        assert_eq!(end, 1);
        assert_eq!(is_balanced, false);

        let s = String::from("This is (an example string.");
        let encap = ('(', ')');
        let (start, end, is_balanced) = find_balanced_encapsulator(&s, encap);
        assert_eq!(start, 8);
        assert_eq!(end, 1);
        assert_eq!(is_balanced, false);
    }

    #[test]
    fn test_find_balanced_encapsulator_backwards() {
        let s = String::from("This is (an example) string.");
        let encap = ('(', ')');
        let (start, end, is_balanced) = find_balanced_encapsulator_backwards(&s, encap);
        assert_eq!(start, 8);
        assert_eq!(end, 20);
        assert_eq!(is_balanced, true);

        let s = String::from("This is an example) string.");
        let encap = ('(', ')');
        let (_, _, is_balanced) = find_balanced_encapsulator_backwards(&s, encap);
        assert_eq!(is_balanced, false);

        let s = String::from("This is (an example string.");
        let encap = ('(', ')');
        let (_, _, is_balanced) = find_balanced_encapsulator_backwards(&s, encap);
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
}

#[cfg(test)]
mod test_threading {
    use crate::utils::threading::*;

    #[test]
    fn test_task_pool_with_single_thread() {
        // Test case with a single thread
        let items = vec![1, 2, 3, 4, 5];
        let num_threads = 1;
        let expected_results = vec![2, 4, 6, 8, 10];

        // Define a simple function to double the input
        let f = |x: i32| x * 2;

        let mut results = task_pool(items, num_threads, f);
        results.sort();
        assert_eq!(results, expected_results);
    }

    #[test]
    fn test_task_pool_with_multiple_threads() {
        // Test case with multiple threads
        let items = vec![1, 2, 3, 4, 5];
        let num_threads = 3;
        let expected_results = vec![2, 4, 6, 8, 10];

        // Define a simple function to double the input
        let f = |x: i32| x * 2;

        let mut results = task_pool(items, num_threads, f);
        results.sort();
        assert_eq!(results, expected_results);
    }

    #[test]
    fn test_task_pool_with_empty_items() {
        // Test case with empty items vector
        let items: Vec<i32> = Vec::new();
        let num_threads = 2;

        // Define a simple function to double the input
        let f = |x: i32| x * 2;

        let results = task_pool(items, num_threads, f);
        assert!(results.len() == 0);
    }
}

#[cfg(test)]
mod test_time {
    use crate::utils::time::*;

    #[test]
    fn test_calculate_eta() {
        assert_eq!(calculate_eta(2.5, 10), 4);
        assert_eq!(calculate_eta(0.5, 100), 200);
        assert_eq!(calculate_eta(1.0, 0), 0);
        assert_eq!(calculate_eta(0.0, 100), std::u128::MAX);
        assert_eq!(calculate_eta(10.0, std::usize::MAX), 1844674407370955264);
    }

    #[test]
    fn test_format_eta() {
        assert_eq!(format_eta(0), "0s");
        assert_eq!(format_eta(59), "59s ");
        assert_eq!(format_eta(60), "1m 0s");
        assert_eq!(format_eta(3600), "1h 0s");
        assert_eq!(format_eta(3665), "1h 1m 5s ");
        assert_eq!(format_eta(86400), "1d 0s");
        assert_eq!(format_eta(172800), "2d 0s");
        assert_eq!(format_eta(180065), "2d 2h 1m 5s ");
    }
}

#[cfg(test)]
mod test_version {
    use crate::utils::version::*;

    #[test]
    fn test_greater_than() {
        let v1 = Version { major: 2, minor: 3, patch: 4 };
        let v2 = Version { major: 2, minor: 3, patch: 3 };
        let v3 = Version { major: 2, minor: 2, patch: 5 };
        let v4 = Version { major: 1, minor: 4, patch: 4 };

        assert!(v1.gt(&v2));
        assert!(v1.gt(&v3));
        assert!(v1.gt(&v4));
        assert!(!v2.gt(&v1));
        assert!(!v1.gt(&v1));
    }

    #[test]
    fn test_greater_than_or_equal_to() {
        let v1 = Version { major: 2, minor: 3, patch: 4 };
        let v2 = Version { major: 2, minor: 3, patch: 4 };

        assert!(v1.gte(&v2));
        assert!(v2.gte(&v1));
        assert!(v1.gte(&Version { major: 1, minor: 0, patch: 0 }));
    }

    #[test]
    fn test_less_than() {
        let v1 = Version { major: 2, minor: 3, patch: 4 };
        let v2 = Version { major: 2, minor: 3, patch: 5 };
        let v3 = Version { major: 2, minor: 4, patch: 4 };
        let v4 = Version { major: 3, minor: 3, patch: 4 };

        assert!(v1.lt(&v2));
        assert!(v1.lt(&v3));
        assert!(v1.lt(&v4));
        assert!(!v2.lt(&v1));
        assert!(!v1.lt(&v1));
    }

    #[test]
    fn test_less_than_or_equal_to() {
        let v1 = Version { major: 2, minor: 3, patch: 4 };
        let v2 = Version { major: 2, minor: 3, patch: 4 };

        assert!(v1.lte(&v2));
        assert!(v2.lte(&v1));
        assert!(v1.lte(&Version { major: 3, minor: 0, patch: 0 }));
    }

    #[test]
    fn test_equal_to() {
        let v1 = Version { major: 2, minor: 3, patch: 4 };
        let v2 = Version { major: 2, minor: 3, patch: 4 };
        let v3 = Version { major: 2, minor: 3, patch: 5 };

        assert!(v1.eq(&v2));
        assert!(!v1.eq(&v3));
    }

    #[test]
    fn test_not_equal_to() {
        let v1 = Version { major: 2, minor: 3, patch: 4 };
        let v2 = Version { major: 2, minor: 3, patch: 5 };
        let v3 = Version { major: 3, minor: 3, patch: 4 };

        assert!(v1.ne(&v2));
        assert!(v1.ne(&v3));
        assert!(!v1.ne(&Version { major: 2, minor: 3, patch: 4 }));
    }

    #[test]
    fn test_version_display() {
        let version = Version { major: 2, minor: 3, patch: 4 };

        assert_eq!(version.to_string(), "2.3.4");
    }

    #[test]
    fn test_version_current() {
        let version = current_version();

        assert_eq!(version.to_string(), env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn test_version_remote() {
        let version = remote_version();

        assert_eq!(version.minor > 0, true);
        assert_eq!(version.patch > 0, true);
    }
}
