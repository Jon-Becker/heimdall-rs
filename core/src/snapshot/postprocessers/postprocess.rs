use heimdall_common::utils::strings::{
    classify_token, find_balanced_encapsulator, tokenize, TokenType,
};

pub fn remove_double_negation(line: &str) -> String {
    let mut cleaned = line.to_owned();

    while cleaned.contains("((!((!((") {
        // find the indices of the subject
        let subject_indices = cleaned.find("((!((!((").unwrap();
        let subject = cleaned[subject_indices..].to_string();

        // get the indices of the subject's first negation encapsulator
        let first_subject_indices = find_balanced_encapsulator(&subject, ('(', ')'));
        if first_subject_indices.2 {
            // the subject to search is now the subject without the first negation encapsulator
            let second_subject = subject[first_subject_indices.0 + 1..].to_string();

            // get the indices of the subject's second negation encapsulator
            let second_subject_indices = find_balanced_encapsulator(&second_subject, ('(', ')'));
            if second_subject_indices.2 {
                // the subject is now the subject without the first and second negation encapsulators
                let subject = second_subject
                    [second_subject_indices.0 + 1..second_subject_indices.1 - 1]
                    .to_string();

                // remove the double negation
                cleaned.replace_range(
                    subject_indices
                        ..subject_indices + first_subject_indices.0 + 2 + second_subject_indices.1,
                    &subject,
                );
            }
        }
    }

    cleaned
}

pub fn simplify_parentheses(line: &str, paren_index: usize) -> String {
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

        // handle the inside of the expression
        let inside = match expression.get(2..expression.len() - 2) {
            Some(x) => x.to_string(),
            None => "".to_string(),
        };

        let inner_tokens = tokenize(&inside);
        return !inner_tokens.iter().any(|tk| classify_token(tk) == TokenType::Operator);
    }

    let mut cleaned: String = line.to_owned();

    // skip lines that are defining a function
    if cleaned.contains("function") {
        return cleaned;
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
            cleaned = simplify_parentheses(&cleaned, paren_index);
        } else {
            // remove double negation, if one exists
            if cleaned.contains("!!") {
                cleaned = cleaned.replace("!!", "");
            }

            // recurse into the next set of parentheses
            cleaned = simplify_parentheses(&cleaned, paren_index + 1);
        }
    }

    cleaned
}

pub fn cleanup(line: &str) -> String {
    let line = simplify_parentheses(line, 0);
    remove_double_negation(&line)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simplify_parentheses() {
        let line = "((arg0))";
        assert_eq!(simplify_parentheses(line, 0), "arg0");
    }

    #[test]
    fn test_simplify_parentheses_complex() {
        let line = "if ((cast(((arg0) + 1) / 10))) {";
        assert_eq!(simplify_parentheses(line, 0), "if (cast((arg0 + 1) / 10)) {");
    }

    #[test]
    fn test_simplify_parentheses_complex2() {
        let line = "if (((((((((((((((cast(((((((((((arg0 * (((((arg1))))))))))))) + 1)) / 10)))))))))))))))) {";
        assert_eq!(simplify_parentheses(line, 0), "if (cast(((arg0 * (arg1)) + 1) / 10)) {");
    }

    #[test]
    fn test_remove_double_negation_and_simplify_parenthesis() {
        let line = "if (!(storage [0x08] > (storage [0x08] + ((!((!((argO * storage [0x08]))))) * ( ( ( (arg® * storage [0x08]) - 0x01) / storage [0x021) + 0×01)))))) { .. }";
        let expected = "if (!storage [0x08] > (storage [0x08] + ((argO * storage [0x08]) * ( ( ( (arg® * storage [0x08]) - 0x01) / storage [0x021) + 0×01))))) { .. }";
        assert_eq!(cleanup(line), expected);
    }
}
