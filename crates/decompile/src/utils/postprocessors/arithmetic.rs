use heimdall_common::ether::lexers::cleanup::simplify_parentheses;

use crate::{core::postprocess::PostprocessorState, Error};

/// Handles simplifying arithmetic operations. For example:
/// - `x + 0` would become `x`
/// - `x * 1` would become `x`
/// - `x - 0` would become `x`
/// - `x / 1` would become `x`
/// - `!!x` would become `x`
///
/// Additionally, this postprocessor will simplify parentheses within the
/// arithmetic operations.
pub fn arithmetic_postprocessor(
    line: &mut String,
    _state: &mut PostprocessorState,
) -> Result<(), Error> {
    // 1. Simplify parentheses
    *line = simplify_parentheses(line, 0).unwrap_or(line.clone());

    // 2. Simplify arithmetic operations
    while let Some(negation) = line.find("!!") {
        line.replace_range(negation..negation + 2, "");
    }

    Ok(())
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
