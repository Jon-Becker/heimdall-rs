use heimdall_common::{
    ether::{evm::types::find_cast, signatures::ResolvedLog},
    utils::strings::{find_balanced_encapsulator, split_string_by_regex},
};
use indicatif::ProgressBar;
use lazy_static::lazy_static;
use std::{collections::HashMap, sync::Mutex};

use crate::decompile::constants::{ARGS_SPLIT_REGEX, ENCLOSED_EXPRESSION_REGEX};

lazy_static! {
    static ref MEM_LOOKUP_MAP: Mutex<HashMap<String, String>> = Mutex::new(HashMap::new());
    static ref VARIABLE_MAP: Mutex<HashMap<String, String>> = Mutex::new(HashMap::new());
    static ref TYPE_MAP: Mutex<HashMap<String, String>> = Mutex::new(HashMap::new());
}

fn remove_double_negation(line: String) -> String {
    let mut cleaned = line;

    if cleaned.contains("iszero(iszero(") {
        // find the indices of the subject
        let subject_indices = cleaned.find("iszero(iszero(").unwrap();
        let subject = cleaned[subject_indices..].to_string();

        // get the indices of the subject's first iszero encapsulator
        let first_subject_indices = find_balanced_encapsulator(&subject, ('(', ')'));
        if first_subject_indices.2 {
            // the subject to search is now the subject without the first iszero encapsulator
            let second_subject = subject[first_subject_indices.0 + 1..].to_string();

            // get the indices of the subject's second iszero encapsulator
            let second_subject_indices = find_balanced_encapsulator(&second_subject, ('(', ')'));
            if second_subject_indices.2 {
                // the subject is now the subject without the first and second iszero encapsulators
                let subject = second_subject
                    [second_subject_indices.0 + 1..second_subject_indices.1 - 1]
                    .to_string();

                // replace the double negation with the subject
                cleaned = cleaned.replace(&format!("iszero(iszero({subject}))"), &subject);
            }
        }
    }

    cleaned
}

fn convert_bitmask_to_casting(line: String) -> String {
    let mut cleaned = line;

    // find instances of and(_, _)
    let mut index = 0;
    while let Some(found_index) = cleaned[index..].find("and(") {
        index += found_index;

        // get indices of arguments
        let (start_index, end_index, _) = find_balanced_encapsulator(&cleaned[index..], ('(', ')'));
        let args = &cleaned[start_index + index + 1..end_index + index - 1];
        let args_vec: Vec<&str> = args.split(", ").collect();
        let arg1 = args_vec[0];
        let arg2 = args_vec[1..].join(", ");

        // check if arg1 or arg2 is a bitmask of all 1's
        let is_lhs_all_ones = arg1.replacen("0x", "", 1).chars().all(|c| c == 'f' || c == 'F');
        let is_rhs_all_ones = arg2.replacen("0x", "", 1).chars().all(|c| c == 'f' || c == 'F');
        if !is_lhs_all_ones && !is_rhs_all_ones {
            index += end_index + 1;
            continue // skip if LHS and RHS are not bitwise masks
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
        cleaned.replace_range(index..end_index + index, &new_str);

        // set index for next iteration of loop
        index += format!("bytes{size_bytes}(").len();
    }

    cleaned
}

fn simplify_casts(line: String) -> String {
    let mut cleaned = line;

    // remove unnecessary casts
    let (cast_start, cast_end, cast_type) = find_cast(cleaned.to_string());

    if let Some(cast) = cast_type {
        let cleaned_cast_pre = cleaned[0..cast_start].to_string();
        let cleaned_cast_post = cleaned[cast_end..].to_string();
        let cleaned_cast = cleaned[cast_start..cast_end].to_string().replace(&cast, "");

        cleaned = format!("{cleaned_cast_pre}{cleaned_cast}{cleaned_cast_post}");

        // check if there are remaining casts
        let (_, _, remaining_cast_type) = find_cast(cleaned_cast_post.clone());
        if remaining_cast_type.is_some() {
            // a cast is remaining, simplify it
            let mut recursive_cleaned = format!("{cleaned_cast_pre}{cleaned_cast}");
            recursive_cleaned.push_str(simplify_casts(cleaned_cast_post).as_str());
            cleaned = recursive_cleaned;
        }
    }

    cleaned
}

fn remove_replace_casts(line: String) -> String {
    let mut cleaned = line;

    // remove casts to bytes32
    cleaned = cleaned.replace("bytes32", "");

    // casts to bytes20 are replaced with the helper castToAddress
    cleaned = cleaned.replace("bytes20", "castToAddress");

    // convert casts to their yul reprs, for example, bytes1(x) -> (x):bytes1
    loop {
        let (cast_start, cast_end, cast_type) = find_cast(cleaned.to_string());
        if let Some(cast_type) = cast_type {
            let cast_arg = &cleaned[cast_start + 1..cast_end - 1];
            let yul_cast = format!("({cast_arg}) : {cast_type}");

            cleaned.replace_range(cast_start - cast_type.len()..=cast_end - 1, &yul_cast);
        } else {
            break
        }
    }

    cleaned
}

fn simplify_parentheses(line: String, paren_index: usize) -> String {
    // helper function to determine if parentheses are necessary
    fn are_parentheses_unnecessary(expression: String) -> bool {
        // safely grab the first and last chars
        let first_char = expression.get(0..1).unwrap_or("");
        let last_char = expression.get(expression.len() - 1..expression.len()).unwrap_or("");

        // if there is a negation of an expression, remove the parentheses
        // helps with double negation
        if first_char == "iszero" && last_char == ")" {
            return true
        }

        // parens required if:
        //  - expression is a cast
        //  - expression is a function call
        //  - expression is the surrounding parens of a conditional
        if first_char != "(" {
            return false
        } else if last_char == ")" {
            return true
        }

        // don't include instantiations
        if expression.contains(":=") {
            return false
        }

        // handle the inside of the expression
        let inside = match expression.get(2..expression.len() - 2) {
            Some(x) => ENCLOSED_EXPRESSION_REGEX.replace(x, "x").to_string(),
            None => "".to_string(),
        };

        if !inside.is_empty() {
            let expression_parts = inside
                .split(|x| ['*', '/', '=', '>', '<', '|', '&', '!'].contains(&x))
                .filter(|x| !x.is_empty())
                .collect::<Vec<&str>>();

            expression_parts.len() == 1
        } else {
            false
        }
    }

    let mut cleaned = line;

    // skip lines that are defining a function
    if cleaned.contains("case") {
        return cleaned
    }

    // get the nth index of the first open paren
    let nth_paren_index = match cleaned.match_indices('(').nth(paren_index) {
        Some(x) => x.0,
        None => return cleaned,
    };

    //find it's matching close paren
    let (paren_start, paren_end, found_match) =
        find_balanced_encapsulator(&cleaned[nth_paren_index..], ('(', ')'));

    // add the nth open paren to the start of the paren_start
    let paren_start = paren_start + nth_paren_index;
    let paren_end = paren_end + nth_paren_index;

    // if a match was found, check if the parens are unnecessary
    if let true = found_match {
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
        if are_parentheses_unnecessary(logical_expression.clone()) {
            cleaned.replace_range(
                paren_start..paren_end,
                match logical_expression.get(2..logical_expression.len() - 2) {
                    Some(x) => x,
                    None => "",
                },
            );

            // recurse into the next set of parentheses
            // don't increment the paren_index because we just removed a set
            cleaned = simplify_parentheses(cleaned, paren_index);
        } else {
            // remove double negation, if one exists
            if cleaned.contains("!!") {
                cleaned = cleaned.replace("!!", "");
            }

            // recurse into the next set of parentheses
            cleaned = simplify_parentheses(cleaned, paren_index + 1);
        }
    }

    cleaned
}

fn add_resolved_events(line: String, all_resolved_events: HashMap<String, ResolvedLog>) -> String {
    let mut cleaned = line;

    // skip lines that not logs
    if !cleaned.contains("log") {
        return cleaned
    }

    // get the inside of the log statement
    let log_statement = find_balanced_encapsulator(&cleaned, ('(', ')'));

    // no balance found, break
    if !log_statement.2 {
        return cleaned
    }

    // use ARGS_SPLIT_REGEX to split the log into its arguments
    let log_args = split_string_by_regex(
        &cleaned[log_statement.0 + 1..log_statement.1 - 1],
        ARGS_SPLIT_REGEX.clone(),
    );

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

fn cleanup(line: String, all_resolved_events: HashMap<String, ResolvedLog>) -> String {
    let mut cleaned = line;

    // skip comments
    if cleaned.starts_with('/') {
        return cleaned
    }

    // remove double negations
    cleaned = remove_double_negation(cleaned);

    // find and replace casts
    cleaned = convert_bitmask_to_casting(cleaned);

    // remove unnecessary casts
    cleaned = simplify_casts(cleaned);

    // remove or replace casts with helper functions
    cleaned = remove_replace_casts(cleaned);

    // remove unnecessary parentheses
    cleaned = simplify_parentheses(cleaned, 0);

    // add resolved events as comments
    cleaned = add_resolved_events(cleaned, all_resolved_events);

    cleaned
}

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
        if line.contains("function") {
            function_count += 1;
            bar.set_message(format!("postprocessed {function_count} functions"));
        }

        // dedent due to closing braces
        if line.starts_with('}') {
            indentation = indentation.saturating_sub(1);
        }

        // cleanup the line
        let cleaned = cleanup(line.to_string(), all_resolved_events.clone());

        // apply postprocessing and indentation
        *line = format!(
            "{}{}",
            " ".repeat(indentation * 4),
            cleaned.replace('\n', &format!("\n{}", " ".repeat(indentation * 4)))
        );

        // indent due to opening braces
        if line.split("//").collect::<Vec<&str>>().first().unwrap().trim().ends_with('{') {
            indentation += 1;
        }
    }

    cleaned_lines
}
