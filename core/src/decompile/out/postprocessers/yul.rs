use heimdall_common::{
    ether::{
        evm::core::types::find_cast,
        lexers::cleanup::{simplify_casts, simplify_parentheses},
        signatures::ResolvedLog,
    },
    utils::strings::{find_balanced_encapsulator, split_string_by_regex},
};
use indicatif::ProgressBar;
use lazy_static::lazy_static;
use std::{collections::HashMap, sync::Mutex};

use crate::{decompile::constants::ARGS_SPLIT_REGEX, error::Error};

lazy_static! {
    static ref MEM_LOOKUP_MAP: Mutex<HashMap<String, String>> = Mutex::new(HashMap::new());
    static ref VARIABLE_MAP: Mutex<HashMap<String, String>> = Mutex::new(HashMap::new());
    static ref TYPE_MAP: Mutex<HashMap<String, String>> = Mutex::new(HashMap::new());
}

/// Remove double negations from a line
fn remove_double_negation(line: &str) -> String {
    let mut cleaned = line.to_owned();

    if cleaned.contains("iszero(iszero(") {
        // find the indices of the subject
        let subject_indices = cleaned
            .find("iszero(iszero(")
            .expect("impossible case: failed to find double negation after check");
        let subject = cleaned[subject_indices..].to_string();

        // get the indices of the subject's first iszero encapsulator
        let iszero_range = match find_balanced_encapsulator(&subject, ('(', ')')) {
            Ok(range) => range,
            Err(_) => return cleaned,
        };

        // the subject to search is now the subject without the first iszero encapsulator
        let second_subject = subject[iszero_range.start..].to_string();

        // get the indices of the subject's second iszero encapsulator
        let second_subject_range = match find_balanced_encapsulator(&second_subject, ('(', ')')) {
            Ok(range) => range,
            Err(_) => return cleaned,
        };

        // the subject is now the subject without the first and second iszero encapsulators
        let subject = second_subject[second_subject_range].to_string();

        // replace the double negation with the subject
        cleaned = cleaned.replace(&format!("iszero(iszero({subject}))"), &subject);
    }

    cleaned
}

/// Convert bitwise operations to a variable type cast
fn convert_bitmask_to_casting(line: &str) -> String {
    let mut cleaned = line.to_owned();

    // find instances of and(_, _)
    let mut index = 0;
    while let Some(found_index) = cleaned[index..].find("and(") {
        index += found_index;

        // get indices of arguments
        let arg_range = find_balanced_encapsulator(&cleaned[index..], ('(', ')'))
            .expect("impossible case: unbalanced parentheses found in balanced expression. please report this bug.");
        let args = &cleaned[arg_range.start + index..arg_range.end + index];
        let args_vec: Vec<&str> = args.split(", ").collect();
        let arg1 = args_vec[0];
        let arg2 = args_vec[1..].join(", ");

        // check if arg1 or arg2 is a bitmask of all 1's
        let is_lhs_all_ones = arg1.replacen("0x", "", 1).chars().all(|c| c == 'f' || c == 'F');
        let is_rhs_all_ones = arg2.replacen("0x", "", 1).chars().all(|c| c == 'f' || c == 'F');
        if !is_lhs_all_ones && !is_rhs_all_ones {
            index += arg_range.end + 2;
            continue; // skip if LHS and RHS are not bitwise masks
        }

        // determine size of bytes based on argument 1
        let size_bytes = if is_lhs_all_ones {
            (arg1.replacen("0x", "", 1).len() / 2) as u32
        } else {
            (arg2.replacen("0x", "", 1).len() / 2) as u32
        };

        // construct new string with casting
        let new_str = format!("bytes{size_bytes}({arg2})");

        // replace old string with new string
        cleaned.replace_range(index..arg_range.end + 1 + index, &new_str);

        // set index for next iteration of loop
        index += format!("bytes{size_bytes}(").len();
    }

    cleaned
}

/// Removes or replaces casts with helper functions
fn remove_replace_casts(line: &str) -> Result<String, Error> {
    let mut cleaned = line.to_owned();

    // remove casts to bytes32
    cleaned = cleaned.replace("bytes32", "");

    // casts to bytes20 are replaced with the helper castToAddress
    cleaned = cleaned.replace("bytes20", "castToAddress");

    // convert casts to their yul reprs, for example, bytes1(x) -> (x):bytes1
    loop {
        let (cast_start, cast_end, cast_type) = match find_cast(&cleaned) {
            Ok((range, cast)) => (range.start, range.end, cast),
            Err(_) => break,
        };

        let cast_arg = &cleaned[cast_start + 1..cast_end - 1];
        let yul_cast = format!("({cast_arg}) : {cast_type}");

        cleaned.replace_range(cast_start - cast_type.len()..=cast_end - 1, &yul_cast);
    }

    Ok(cleaned)
}

/// Add resolved events as comments
fn add_resolved_events(line: &str, all_resolved_events: HashMap<String, ResolvedLog>) -> String {
    let mut cleaned = line.to_owned();

    // skip lines that not logs
    if !cleaned.contains("log") {
        return cleaned;
    }

    // get the inside of the log statement, then use ARGS_SPLIT_REGEX to split the log into its
    // arguments
    let log_args = match find_balanced_encapsulator(&cleaned, ('(', ')')) {
        Ok(range) => split_string_by_regex(&cleaned[range], ARGS_SPLIT_REGEX.clone()),
        Err(_) => return cleaned,
    };

    // get the event matching the log's selector
    for (selector, resolved_event) in all_resolved_events.iter() {
        if log_args.contains(&format!("0x{selector}")) {
            cleaned = format!(
                "\n/* \"{}({})\" */\n{}",
                resolved_event.name,
                resolved_event.inputs.join(", "),
                cleaned
            )
        }
    }

    cleaned
}

/// Cleans up a line using postprocessing techniques
fn cleanup(line: &str, all_resolved_events: HashMap<String, ResolvedLog>) -> String {
    let mut cleaned = line.to_owned();

    // skip comments
    if cleaned.starts_with('/') {
        return cleaned;
    }

    // remove double negations
    cleaned = remove_double_negation(&cleaned);

    // find and replace casts
    cleaned = convert_bitmask_to_casting(&cleaned);

    // remove unnecessary casts
    cleaned = simplify_casts(&cleaned);

    // remove or replace casts with helper functions
    cleaned = remove_replace_casts(&cleaned).unwrap_or(cleaned);

    // remove unnecessary parentheses
    cleaned = simplify_parentheses(&cleaned, 0).unwrap_or(cleaned);

    // add resolved events as comments
    cleaned = add_resolved_events(&cleaned, all_resolved_events);

    cleaned
}

/// Postprocesses the cleaned lines
pub fn postprocess(
    lines: Vec<String>,
    all_resolved_events: HashMap<String, ResolvedLog>,
    bar: &ProgressBar,
) -> Vec<String> {
    let mut indentation: usize = 0;
    let mut function_count = 0;
    let mut cleaned_lines: Vec<String> = lines;

    // clean up each line using postprocessing techniques
    for line in cleaned_lines.iter_mut() {
        // update progress bar
        if line.contains("function") || line.contains("default") {
            function_count += 1;
            bar.set_message(format!("postprocessed {function_count} functions"));
        }

        // dedent due to closing braces
        if line.starts_with('}') {
            indentation = indentation.saturating_sub(1);
        }

        // cleanup the line
        let cleaned = cleanup(line, all_resolved_events.clone());

        // apply postprocessing and indentation
        *line = format!(
            "{}{}",
            " ".repeat(indentation * 4),
            cleaned.replace('\n', &format!("\n{}", " ".repeat(indentation * 4)))
        );

        // indent due to opening braces
        if line.split("//").collect::<Vec<&str>>().first().unwrap_or(&"").trim().ends_with('{') {
            indentation += 1;
        }
    }

    cleaned_lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_remove_double_negation() {
        let line = "iszero(iszero(add(0x00, 0x01)))";

        let cleaned = remove_double_negation(line);
        assert_eq!(cleaned, "add(0x00, 0x01)");
    }

    #[test]
    fn test_convert_bitmask_to_casting_address() {
        let line = "and(0xffffffffffffffffffffffffffffffffffffffff, calldataload(0x04))";

        let cleaned = convert_bitmask_to_casting(line);
        assert_eq!(cleaned, "bytes20(calldataload(0x04))");
    }

    #[test]
    fn test_convert_bitmask_to_casting_bytes32() {
        let line = "and(0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff, calldataload(0x04))";

        let cleaned = convert_bitmask_to_casting(line);
        assert_eq!(cleaned, "bytes32(calldataload(0x04))");
    }

    #[test]
    fn test_remove_replace_casts() {
        let line = "bytes32(0x00)";

        let cleaned = remove_replace_casts(line).expect("failed to remove replace casts");
        assert_eq!(cleaned, "(0x00)");
    }

    // TODO : more coverage after i get core to compile lol
}
