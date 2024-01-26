use crate::{
    constants::{
        AND_BITMASK_REGEX, AND_BITMASK_REGEX_2, DIV_BY_ONE_REGEX, ENCLOSED_EXPRESSION_REGEX,
        MUL_BY_ONE_REGEX, NON_ZERO_BYTE_REGEX,
    },
    error::Error,
    ether::evm::core::types::{byte_size_to_type, find_cast},
    utils::strings::{
        classify_token, find_balanced_encapsulator, find_balanced_encapsulator_backwards, tokenize,
        TokenType,
    },
};

// TODO: decompile should also use the Cleanup trait rather than implement its own cleanup
// TODO: optimize Cleanup trait
pub trait Cleanup {
    fn cleanup(self) -> Self;
}

/// Convert bitwise operations to a variable type cast
pub fn convert_bitmask_to_casting(line: &str) -> Result<String, Error> {
    let mut cleaned = line.to_owned();

    match AND_BITMASK_REGEX
        .find(&cleaned)
        .map_err(|e| Error::Generic(format!("failed to find bitmask: {}", e)))?
    {
        Some(bitmask) => {
            let cast = bitmask.as_str();
            let cast_size = NON_ZERO_BYTE_REGEX.find_iter(cast).count();
            let (_, cast_types) = byte_size_to_type(cast_size);

            // get the cast subject
            let mut subject = cleaned
                .get(bitmask.end()..)
                .ok_or(Error::Generic(format!("failed to get cast subject: {}", bitmask.end())))?
                .replace(';', "");

            // attempt to find matching parentheses
            let subject_range = find_balanced_encapsulator(&subject, ('(', ')'))
                .expect("impossible case: unbalanced parentheses found in balanced expression. please report this bug.");

            subject = subject[subject_range.start - 1..subject_range.end + 1].to_string();

            // if the cast is a bool, check if the line is a conditional
            let solidity_type = match cast_types[0].as_str() {
                "bool" => {
                    if cleaned.contains("if") {
                        String::new()
                    } else {
                        "bytes1".to_string()
                    }
                }
                _ => cast_types[0].to_owned(),
            };

            // apply the cast to the subject
            cleaned =
                cleaned.replace(&format!("{cast}{subject}"), &format!("{solidity_type}{subject}"));

            // attempt to cast again
            cleaned = convert_bitmask_to_casting(&cleaned)?;
        }
        None => {
            if let Some(bitmask) = AND_BITMASK_REGEX_2
                .find(&cleaned)
                .map_err(|e| Error::Generic(format!("failed to find bitmask: {}", e)))?
            {
                let cast = bitmask.as_str();
                let cast_size = NON_ZERO_BYTE_REGEX.find_iter(cast).count();
                let (_, cast_types) = byte_size_to_type(cast_size);

                // get the cast subject
                let mut subject = match cleaned
                    .get(0..bitmask.start())
                    .ok_or(Error::Generic(format!(
                        "failed to get cast subject: {}",
                        bitmask.start()
                    )))?
                    .replace(';', "")
                    .split('=')
                    .collect::<Vec<&str>>()
                    .last()
                {
                    Some(subject) => subject.to_string(),
                    None => cleaned
                        .get(0..bitmask.start())
                        .ok_or(Error::Generic(format!(
                            "failed to get cast subject: {}",
                            bitmask.start()
                        )))?
                        .to_string()
                        .replace(';', ""),
                };

                // attempt to find matching parentheses
                let subject_range = find_balanced_encapsulator_backwards(&subject, ('(', ')'))
                    .expect("impossible case: unbalanced parentheses found in balanced expression. please report this bug.");

                subject = subject[subject_range.start - 1..subject_range.end + 1].to_string();

                // if the cast is a bool, check if the line is a conditional
                let solidity_type = match cast_types[0].as_str() {
                    "bool" => {
                        if cleaned.contains("if") {
                            String::new()
                        } else {
                            "bytes1".to_string()
                        }
                    }
                    _ => cast_types[0].to_owned(),
                };

                // apply the cast to the subject
                cleaned = cleaned
                    .replace(&format!("{subject}{cast}"), &format!("{solidity_type}{subject}"));

                // attempt to cast again
                cleaned = convert_bitmask_to_casting(&cleaned)?;
            }
        }
    }

    Ok(cleaned)
}

/// Removes unnecessary casts
pub fn simplify_casts(line: &str) -> String {
    let mut cleaned = line.to_owned();

    // remove unnecessary casts
    let (cast_range, cast) = match find_cast(&cleaned) {
        Ok((cast_range, cast_type)) => (cast_range, cast_type),
        _ => return cleaned,
    };

    let cleaned_cast_pre = cleaned[0..cast_range.start - 1].to_string();
    let cleaned_cast_post = cleaned[cast_range.end + 1..].to_string();
    let cleaned_cast = cleaned[cast_range.start - 1..cast_range.end + 1].to_string().replace(&cast, "");

    cleaned = format!("{cleaned_cast_pre}{cleaned_cast}{cleaned_cast_post}");

    // check if there are remaining casts
    if find_cast(&cleaned_cast_post).is_ok() {
        // a cast is remaining, simplify it
        cleaned =
            format!("{}{}{}", cleaned_cast_pre, cleaned_cast, simplify_casts(&cleaned_cast_post));
    }

    cleaned
}

/// Simplifies arithmatic by removing unnecessary operations
pub fn simplify_arithmatic(line: &str) -> String {
    let cleaned = DIV_BY_ONE_REGEX.replace_all(line, "");
    let cleaned = MUL_BY_ONE_REGEX.replace_all(&cleaned, "");

    // remove double negation
    cleaned.replace("!!", "")
}

/// Simplifies expressions by removing unnecessary parentheses
// TODO: implement simplify_parentheses correctly with a tokenier
pub fn simplify_parentheses(line: &str, paren_index: usize) -> Result<String, Error> {
    // helper function to determine if parentheses are necessary
    fn are_parentheses_unnecessary(expression: &str) -> bool {
        // safely grab the first and last chars
        let first_char = expression.get(0..1).unwrap_or("");
        let last_char = expression.get(expression.len() - 1..expression.len()).unwrap_or("");

        // if there is a negation of an expression, remove the parentheses
        // helps with double negation
        if first_char == "!" && last_char == ")" {
            return true;
        }

        // remove the parentheses if the expression is within brackets
        if first_char == "[" && last_char == "]" {
            return true;
        }

        // parens required if:
        //  - expression is a cast
        //  - expression is a function call
        //  - expression is the surrounding parens of a conditional
        if first_char != "(" {
            return false;
        } else if last_char == ")" {
            return true;
        }

        // don't include instantiations
        if expression.contains("memory ret") {
            return false;
        }

        // handle the inside of the expression
        let inside = match expression.get(2..expression.len() - 2) {
            Some(x) => ENCLOSED_EXPRESSION_REGEX.replace(x, "x").to_string(),
            None => "".to_string(),
        };

        let inner_tokens = tokenize(&inside);
        return !inner_tokens.iter().any(|tk| classify_token(tk) == TokenType::Operator);
    }

    let mut cleaned: String = line.to_owned();

    // skip lines that are defining a function
    if cleaned.contains("function") {
        return Ok(cleaned);
    }

    // get the nth index of the first open paren
    let nth_paren_index = match cleaned.match_indices('(').nth(paren_index) {
        Some(x) => x.0,
        None => return Ok(cleaned),
    };

    //find it's matching close paren
    let paren_range = match find_balanced_encapsulator(&cleaned[nth_paren_index..], ('(', ')')) {
        Ok(range) => range,
        Err(_) => return Ok(cleaned),
    };

    // add the nth open paren to the start of the paren_start
    let paren_start = paren_range.start - 1 + nth_paren_index;
    let paren_end = paren_range.end + 1 + nth_paren_index;

    // get the logical expression including the char before the parentheses (to detect casts)
    let logical_expression = match paren_start {
        0 => match cleaned.get(paren_start..paren_end + 1) {
            Some(expression) => expression.to_string(),
            None => cleaned[paren_start..paren_end].to_string(),
        },
        _ => match cleaned.get(paren_start - 1..paren_end + 1) {
            Some(expression) => expression.to_string(),
            None => cleaned[paren_start - 1..paren_end].to_string(),
        },
    };

    // check if the parentheses are unnecessary and remove them if so
    if are_parentheses_unnecessary(&logical_expression) {
        cleaned.replace_range(
            paren_start..paren_end,
            match logical_expression.get(2..logical_expression.len() - 2) {
                Some(x) => x,
                None => "",
            },
        );

        // remove double negation, if one was created
        if cleaned.contains("!!") {
            cleaned = cleaned.replace("!!", "");
        }

        // recurse into the next set of parentheses
        // don't increment the paren_index because we just removed a set
        cleaned = simplify_parentheses(&cleaned, paren_index)?;
    } else {
        // remove double negation, if one exists
        if cleaned.contains("!!") {
            cleaned = cleaned.replace("!!", "");
        }

        // recurse into the next set of parentheses
        cleaned = simplify_parentheses(&cleaned, paren_index + 1)?;
    }

    Ok(cleaned)
}

impl Cleanup for String {
    fn cleanup(mut self) -> Self {
        // remove unnecessary casts
        self = simplify_casts(&self);

        // convert bitmasks to casts
        self = convert_bitmask_to_casting(&self).unwrap_or(self);

        // simplify arithmatic
        self = simplify_arithmatic(&self);

        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bitmask_conversion() {
        let line = String::from(
            "(0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff) & (arg0);",
        );

        assert_eq!(
            convert_bitmask_to_casting(&line).expect("failed to convert bitmask to casting"),
            String::from("uint256(arg0);")
        );
    }

    #[test]
    fn test_bitmask_conversion_mask_after() {
        let line = String::from(
            "(arg0) & (0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff);",
        );

        assert_eq!(
            convert_bitmask_to_casting(&line).expect("failed to convert bitmask to casting"),
            String::from("uint256(arg0);")
        );
    }

    #[test]
    fn test_bitmask_conversion_unusual_mask() {
        let line = String::from(
            "(arg0) & (0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff00);",
        );

        assert_eq!(
            convert_bitmask_to_casting(&line).expect("failed to convert bitmask to casting"),
            String::from("uint248(arg0);")
        );
    }

    #[test]
    fn test_simplify_casts_simple() {
        let line = String::from("uint256(uint256(arg0));");

        assert_eq!(simplify_casts(&line), String::from("uint256(arg0);"));
    }

    #[test]
    fn test_simplify_casts_unnecessary() {
        let line = String::from("uint256(arg0);");

        assert_eq!(simplify_casts(&line), String::from("uint256(arg0);"));
    }

    #[test]
    fn test_simplify_casts_complex() {
        let line = String::from("ecrecover(uint256(uint256(arg0)), uint256(uint256(arg0)), uint256(uint256(uint256(arg0))));");

        assert_eq!(
            simplify_casts(&line),
            String::from("ecrecover(uint256(arg0), uint256(arg0), uint256((arg0)));")
        ); // double parens are expected because we dont simplify_parentheses here
    }

    #[test]
    fn test_simplify_arithmatic() {
        let line = String::from("uint256(arg0) / 0x01;");

        assert_eq!(simplify_arithmatic(&line), String::from("uint256(arg0);"));
    }

    #[test]
    fn test_simplify_arithmatic_complex() {
        let line = String::from("uint256(arg0) / 0x01 + 0x01;");

        assert_eq!(simplify_arithmatic(&line), String::from("uint256(arg0) + 0x01;"));
    }

    #[test]
    fn test_simplify_arithmatic_complex_2() {
        let line = String::from("uint256(arg0) / 0x01 + 0x01 * 0x01;");

        assert_eq!(simplify_arithmatic(&line), String::from("uint256(arg0) + 0x01;"));
    }

    #[test]
    fn test_simplify_parentheses() {
        let line = String::from("((arg0))");

        assert_eq!(
            simplify_parentheses(&line, 0).expect("failed to simplify parentheses"),
            String::from("arg0")
        );
    }

    #[test]
    fn test_simplify_parentheses_unnecessary() {
        let line = String::from("uint256(arg0);");

        assert_eq!(
            simplify_parentheses(&line, 0).expect("failed to simplify parentheses"),
            String::from("uint256(arg0);")
        );
    }

    #[test]
    fn test_simplify_parentheses_complex() {
        let line = String::from("if ((cast(((arg0) + 1) / 10))) {");

        assert_eq!(
            simplify_parentheses(&line, 0).expect("failed to simplify parentheses"),
            String::from("if (cast((arg0 + 1) / 10)) {")
        );
    }

    #[test]
    fn test_simplify_parentheses_complex2() {
        let line = String::from("if (((((((((((((((cast(((((((((((arg0 * (((((arg1))))))))))))) + 1)) / 10)))))))))))))))) {");

        assert_eq!(
            simplify_parentheses(&line, 0).expect("failed to simplify parentheses"),
            String::from("if (cast(((arg0 * (arg1)) + 1) / 10)) {")
        );
    }
}
