use eyre::{eyre, OptionExt};
use heimdall_common::{
    ether::evm::core::types::{byte_size_to_type, find_cast},
    utils::strings::{find_balanced_encapsulator, find_balanced_encapsulator_backwards},
};

use crate::{
    core::postprocess::PostprocessorState,
    utils::constants::{AND_BITMASK_REGEX, AND_BITMASK_REGEX_2, NON_ZERO_BYTE_REGEX},
    Error,
};

/// Converts bitwise masking operations to type casts. For example
/// `x & 0xffffffff00000000...` would become `bytes4(x)`.
///
/// This postprocessor also handles cast cleanup. For example,
/// `bytes4(bytes4(x))` would become `bytes4(x)`.
///
/// Note: We cant handle this in the analyzer / lexer itself due to the
/// nature of [`WrappedOpcode`]s. Essentially pattern matching on
/// `WrappedOpcode::Raw(_)` and `WrappedOpcode::Opcode(_)` is not possible
/// for complicated reasons. If you want to know more about why, ask @Jon-Becker.
pub fn bitwise_mask_postprocessor(
    line: &mut String,
    _: &mut PostprocessorState,
) -> Result<(), Error> {
    loop {
        let mut found_bitmask = false;

        if let Some(bitmask) =
            AND_BITMASK_REGEX.find(line).map_err(|e| eyre!("regex error: {}", e))?
        {
            let cast = bitmask.as_str();
            let cast_size = NON_ZERO_BYTE_REGEX.find_iter(cast).count();
            let (_, cast_types) = byte_size_to_type(cast_size);

            // get the cast subject
            let mut subject = line
                .get(bitmask.end()..)
                .ok_or_eyre("failed to get cast subject")?
                .replace(';', "");

            // attempt to find matching parentheses
            let subject_range = find_balanced_encapsulator(&subject, ('(', ')'))
                    .expect("impossible case: unbalanced parentheses found in balanced expression. please report this bug.");

            subject = subject[subject_range.start - 1..subject_range.end + 1].to_string();

            // if the cast is a bool, check if the line is a conditional
            let solidity_type = match cast_types[0].as_str() {
                "bool" => {
                    if line.contains("if") {
                        String::new()
                    } else {
                        "bytes1".to_string()
                    }
                }
                _ => cast_types[0].to_owned(),
            };

            // apply the cast to the subject
            *line = line.replace(&format!("{cast}{subject}"), &format!("{solidity_type}{subject}"));

            found_bitmask = true;
        } else if let Some(bitmask) =
            AND_BITMASK_REGEX_2.find(line).map_err(|e| eyre!("regex error: {}", e))?
        {
            let cast = bitmask.as_str();
            let cast_size = NON_ZERO_BYTE_REGEX.find_iter(cast).count();
            let (_, cast_types) = byte_size_to_type(cast_size);

            // get the cast subject
            let mut subject = line
                .get(0..bitmask.start())
                .ok_or_eyre("failed to get cast subject")?
                .replace(';', "")
                .split('=')
                .collect::<Vec<&str>>()
                .last()
                .unwrap()
                .to_string();

            // attempt to find matching parentheses
            let subject_range = find_balanced_encapsulator_backwards(&subject, ('(', ')'))
                    .expect("impossible case: unbalanced parentheses found in balanced expression. please report this bug.");

            subject = subject[subject_range.start - 1..subject_range.end + 1].to_string();

            // if the cast is a bool, check if the line is a conditional
            let solidity_type = match cast_types[0].as_str() {
                "bool" => {
                    if line.contains("if") {
                        String::new()
                    } else {
                        "bytes1".to_string()
                    }
                }
                _ => cast_types[0].to_owned(),
            };

            // apply the cast to the subject
            *line = line.replace(&format!("{subject}{cast}"), &format!("{solidity_type}{subject}"));

            found_bitmask = true;
        }

        // If no replacements were made, exit the loop
        if !found_bitmask {
            break;
        }
    }

    // 2. simplify casts
    *line = simplify_casts(line);

    Ok(())
}

/// helper function which recursively simplifies casts
///
/// note: this function clones the input string, but hopefully
/// in the future ill be able to avoid that
pub fn simplify_casts(line: &str) -> String {
    let mut cleaned = line.to_owned();

    // remove unnecessary casts
    let (cast_range, cast) = match find_cast(&cleaned) {
        Ok((cast_range, cast_type)) => (cast_range, cast_type),
        _ => return cleaned,
    };

    let cleaned_cast_pre = cleaned[0..cast_range.start - 1].to_string();
    let cleaned_cast_post = cleaned[cast_range.end + 1..].to_string();
    let cleaned_cast =
        cleaned[cast_range.start - 1..cast_range.end + 1].to_string().replace(&cast, "");

    cleaned = format!("{cleaned_cast_pre}{cleaned_cast}{cleaned_cast_post}");

    // check if there are remaining casts
    if find_cast(&cleaned_cast_post).is_ok() {
        // a cast is remaining, simplify it
        cleaned =
            format!("{}{}{}", cleaned_cast_pre, cleaned_cast, simplify_casts(&cleaned_cast_post));
    }

    cleaned
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bitmask_conversion() {
        let mut line = String::from(
            "(0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff) & (arg0);",
        );
        let mut state = PostprocessorState::default();

        bitwise_mask_postprocessor(&mut line, &mut state)
            .expect("failed to convert bitmask to casting");

        assert_eq!(line, String::from("uint256(arg0);"));
    }

    #[test]
    fn test_bitmask_conversion_mask_after() {
        let mut line = String::from(
            "(arg0) & (0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff);",
        );
        let mut state = PostprocessorState::default();

        bitwise_mask_postprocessor(&mut line, &mut state)
            .expect("failed to convert bitmask to casting");

        assert_eq!(line, String::from("uint256(arg0);"));
    }

    #[test]
    fn test_bitmask_conversion_unusual_mask() {
        let mut line = String::from(
            "(arg0) & (0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff00);",
        );
        let mut state = PostprocessorState::default();

        bitwise_mask_postprocessor(&mut line, &mut state)
            .expect("failed to convert bitmask to casting");

        assert_eq!(line, String::from("uint248(arg0);"));
    }

    #[test]
    fn test_simplify_casts_simple() {
        let mut line = String::from("uint256(uint256(arg0));");
        let mut state = PostprocessorState::default();

        bitwise_mask_postprocessor(&mut line, &mut state).expect("failed to simplify casts");

        assert_eq!(line, String::from("uint256((arg0));"));
    }

    #[test]
    fn test_simplify_casts_unnecessary() {
        let mut line = String::from("uint256(arg0);");
        let mut state = PostprocessorState::default();

        bitwise_mask_postprocessor(&mut line, &mut state).expect("failed to simplify casts");

        assert_eq!(line, String::from("uint256(arg0);"));
    }

    #[test]
    fn test_simplify_casts_complex() {
        let mut line = String::from("ecrecover(uint256(uint256(arg0)), uint256(uint256(arg0)), uint256(uint256(uint256(arg0))));");
        let mut state = PostprocessorState::default();

        bitwise_mask_postprocessor(&mut line, &mut state).expect("failed to simplify casts");

        assert_eq!(
            line,
            String::from("ecrecover(uint256((arg0)), uint256((arg0)), uint256(((arg0))));")
        );
    }
}
