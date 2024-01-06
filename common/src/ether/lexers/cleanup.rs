use crate::{
    constants::{
        AND_BITMASK_REGEX, AND_BITMASK_REGEX_2, DIV_BY_ONE_REGEX, MUL_BY_ONE_REGEX,
        NON_ZERO_BYTE_REGEX,
    },
    ether::evm::core::types::{byte_size_to_type, find_cast},
    utils::strings::{find_balanced_encapsulator, find_balanced_encapsulator_backwards},
};

// TODO: decompile should also use the Cleanup trait rather than implement its own cleanup
// TODO: optimize Cleanup trait
pub trait Cleanup {
    fn cleanup(self) -> Self;
}

/// Convert bitwise operations to a variable type cast
fn convert_bitmask_to_casting(line: &str) -> String {
    let mut cleaned = line.to_owned();

    match AND_BITMASK_REGEX.find(&cleaned).unwrap() {
        Some(bitmask) => {
            let cast = bitmask.as_str();
            let cast_size = NON_ZERO_BYTE_REGEX.find_iter(cast).count();
            let (_, cast_types) = byte_size_to_type(cast_size);

            // get the cast subject
            let mut subject = cleaned.get(bitmask.end()..).unwrap().replace(';', "");

            // attempt to find matching parentheses
            let subject_range = find_balanced_encapsulator(&subject, ('(', ')'))
                .expect("impossible case: unbalanced parentheses found in balanced expression. please report this bug.");

            subject = subject[subject_range.start - 1..subject_range.end + 1].to_string();

            println!("SOME: subject: {}", subject);
            println!("cast types: {:?}", cast_types);

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
            cleaned = convert_bitmask_to_casting(&cleaned);
        }
        None => {
            if let Some(bitmask) = AND_BITMASK_REGEX_2.find(&cleaned).unwrap() {
                let cast = bitmask.as_str();
                let cast_size = NON_ZERO_BYTE_REGEX.find_iter(cast).count();
                let (_, cast_types) = byte_size_to_type(cast_size);

                // get the cast subject
                let mut subject = match cleaned
                    .get(0..bitmask.start())
                    .unwrap()
                    .replace(';', "")
                    .split('=')
                    .collect::<Vec<&str>>()
                    .last()
                {
                    Some(subject) => subject.to_string(),
                    None => cleaned.get(0..bitmask.start()).unwrap().replace(';', ""),
                };

                // attempt to find matching parentheses
                let subject_range = find_balanced_encapsulator_backwards(&subject, ('(', ')'))
                    .expect("impossible case: unbalanced parentheses found in balanced expression. please report this bug.");

                subject = subject[subject_range.start - 1..subject_range.end + 1].to_string();

                println!("NONE: subject: {}", subject);
                println!("cast types: {:?}", cast_types);

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
                cleaned = convert_bitmask_to_casting(&cleaned);
            }
        }
    }

    cleaned
}

/// Removes unnecessary casts
fn simplify_casts(line: &str) -> String {
    let mut cleaned = line.to_owned();

    // remove unnecessary casts
    let (cast_range, cast) = match find_cast(&cleaned) {
        Ok((cast_range, cast_type)) => (cast_range, cast_type),
        _ => return cleaned,
    };

    let cleaned_cast_pre = cleaned[0..cast_range.start - 1].to_string();
    let cleaned_cast_post = cleaned[cast_range.end + 1..].to_string();
    let cleaned_cast = cleaned[cast_range.start..cast_range.end].to_string().replace(&cast, "");

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
fn simplify_arithmatic(line: &str) -> String {
    let cleaned = DIV_BY_ONE_REGEX.replace_all(line, "");
    let cleaned = MUL_BY_ONE_REGEX.replace_all(&cleaned, "");

    // remove double negation
    cleaned.replace("!!", "")
}



impl Cleanup for String {
    fn cleanup(mut self) -> Self {
        // remove unnecessary casts
        self = simplify_casts(&self);

        // convert bitmasks to casts
        self = convert_bitmask_to_casting(&self);

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

        assert_eq!(convert_bitmask_to_casting(&line), String::from("uint256(arg0);"));
    }

    #[test]
    fn test_bitmask_conversion_mask_after() {
        let line = String::from(
            "(arg0) & (0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff);",
        );

        assert_eq!(convert_bitmask_to_casting(&line), String::from("uint256(arg0);"));
    }

    #[test]
    fn test_bitmask_conversion_unusual_mask() {
        let line = String::from(
            "(arg0) & (0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff00);",
        );

        assert_eq!(convert_bitmask_to_casting(&line), String::from("uint248(arg0);"));
    }

    #[test]
    fn test_simplify_casts_simple() {
        let line = String::from("uint256(uint256(arg0));");

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

        assert_eq!(
            simplify_arithmatic(&line),
            String::from("uint256(arg0) + 0x01;")
        );
    }
}
