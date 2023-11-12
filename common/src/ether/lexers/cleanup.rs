use crate::{
    ether::evm::core::types::{byte_size_to_type, find_cast},
    utils::strings::{find_balanced_encapsulator, find_balanced_encapsulator_backwards},
};
use fancy_regex::Regex;
use lazy_static::lazy_static;

pub trait Cleanup {
    fn cleanup(self) -> Self;
}

lazy_static! {
    pub static ref AND_BITMASK_REGEX: Regex =
        Regex::new(r"\(0x([a-fA-F0-9]{2}){1,32}\) & ").unwrap();
    pub static ref AND_BITMASK_REGEX_2: Regex =
        Regex::new(r" & \(0x([a-fA-F0-9]{2}){1,32}\)").unwrap();
    pub static ref NON_ZERO_BYTE_REGEX: Regex = Regex::new(r"[a-fA-F0-9][a-fA-F1-9]").unwrap();
    pub static ref DIV_BY_ONE_REGEX: Regex = Regex::new(r" \/ 0x01(?!\d)").unwrap();
    pub static ref MUL_BY_ONE_REGEX: Regex =
        Regex::new(r"\b0x01\b\s*\*\s*| \*\s*\b0x01\b").unwrap();
    pub static ref ENCLOSED_EXPRESSION_REGEX: Regex = Regex::new(r"\(.*\)").unwrap();
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
            let subject_indices = find_balanced_encapsulator(&subject, ('(', ')'));
            subject = match subject_indices.2 {
                true => {
                    // get the subject as hte substring between the balanced parentheses found in
                    // unbalanced subject
                    subject[subject_indices.0..subject_indices.1].to_string()
                }
                false => {
                    // this shouldn't happen, but if it does, just return the subject.
                    //TODO add this to verbose logs
                    subject
                }
            };

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
                let subject_indices = find_balanced_encapsulator_backwards(&subject, ('(', ')'));

                subject = match subject_indices.2 {
                    true => {
                        // get the subject as hte substring between the balanced parentheses found
                        // in unbalanced subject
                        subject[subject_indices.0..subject_indices.1].to_string()
                    }
                    false => {
                        // this shouldn't happen, but if it does, just return the subject.
                        subject
                    }
                };

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
    let (cast_start, cast_end, cast_type) = find_cast(&cleaned);

    if let Some(cast) = cast_type {
        let cleaned_cast_pre = cleaned[0..cast_start].to_string();
        let cleaned_cast_post = cleaned[cast_end..].to_string();
        let cleaned_cast = cleaned[cast_start..cast_end].to_string().replace(&cast, "");

        cleaned = format!("{cleaned_cast_pre}{cleaned_cast}{cleaned_cast_post}");

        // check if there are remaining casts
        let (_, _, remaining_cast_type) = find_cast(&cleaned_cast_post);
        if remaining_cast_type.is_some() {
            // a cast is remaining, simplify it
            cleaned = format!(
                "{}{}{}",
                cleaned_cast_pre,
                cleaned_cast,
                simplify_casts(&cleaned_cast_post)
            );
        }
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
