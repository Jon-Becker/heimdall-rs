use heimdall_common::utils::strings::{
    classify_token, find_balanced_encapsulator, tokenize, TokenType,
};

use crate::{
    core::postprocess::PostprocessorState, utils::constants::ENCLOSED_EXPRESSION_REGEX, Error,
};

/// Handles simplifying arithmetic operations. For example:
/// - `x + 0` would become `x`
/// - `x * 1` would become `x`
/// - `x - 0` would become `x`
/// - `x / 1` would become `x`
/// - `!!x` would become `x`
///
/// Additionally, this postprocessor will simplify parentheses within the
/// arithmetic operations.
pub(crate) fn arithmetic_postprocessor(
    line: &mut String,
    _state: &mut PostprocessorState,
) -> Result<(), Error> {
    // 1. Simplify parentheses
    *line = simplify_parentheses(line, 0).unwrap_or_else(|_| line.clone());

    // 2. Simplify arithmetic operations
    while let Some(negation) = line.find("!!") {
        line.replace_range(negation..negation + 2, "");
    }

    Ok(())
}

/// Simplifies expressions by removing unnecessary parentheses
pub(super) fn simplify_parentheses(line: &str, paren_index: usize) -> Result<String, Error> {
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
        !inner_tokens.iter().any(|tk| classify_token(tk) == TokenType::Operator)
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
        0 => match cleaned.get(paren_start..=paren_end) {
            Some(expression) => expression.to_string(),
            None => cleaned.chars().skip(paren_start).take(paren_end - paren_start + 1).collect(),
        },
        _ => match cleaned.get((paren_start - 1)..=paren_end) {
            Some(expression) => expression.to_string(),
            None => {
                cleaned.chars().skip(paren_start - 1).take(paren_end - paren_start + 2).collect()
            }
        },
    };

    // check if the parentheses are unnecessary and remove them if so
    if are_parentheses_unnecessary(&logical_expression) {
        cleaned.replace_range(
            paren_start..paren_end,
            logical_expression.get(2..logical_expression.len() - 2).unwrap_or_default(),
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

// /// Extracts non-overlapping parenthesized expressions from a line.
// fn find_parenthesized_expressions(line: &str) -> Vec<String> {
//     let mut results = Vec::new();
//     let mut stack = Vec::new();

//     for (idx, ch) in line.chars().enumerate() {
//         match ch {
//             '(' => {
//                 stack.push(idx);
//             }
//             ')' => {
//                 if let Some(open_idx) = stack.pop() {
//                     if stack.is_empty() {
//                         // complete expression found when stack is empty
//                         results.push(line[open_idx + 1..idx].to_string());
//                     }
//                 }
//             }
//             _ => {}
//         }
//     }

//     results
// }

#[cfg(test)]
mod tests {}
