use heimdall_common::ether::evm::types::byte_size_to_type;

use crate::decompile::util::find_balanced_parentheses;

use super::constants::{AND_BITMASK_REGEX, AND_BITMASK_REGEX_2};

fn convert_bitmask_to_casting(line: String) -> String {
    let mut cleaned = line;

    match AND_BITMASK_REGEX.find(&cleaned) {
        Some(bitmask) => {
            let cast = bitmask.as_str();
            let cast_size = cast.matches("ff").count();
            let (_, cast_types) = byte_size_to_type(cast_size);

            // get the cast subject
            let mut subject = cleaned.get(bitmask.end()..).unwrap().replace(";",  "");
            
            // attempt to find matching parentheses
            let subject_indices = find_balanced_parentheses(subject.to_string());
            subject = match subject_indices.2 {
                true => {

                    // get the matched subject
                    match subject.get(subject_indices.0..subject_indices.1) {
                        Some(subject) => subject.to_string(),
                        None => match subject.split(")").collect::<Vec<&str>>().first() {
                            Some(subject) => subject.to_string(),
                            None => subject
                        },
                    }
                },
                false => {
                    
                    // subject doesn't contain parentheses, so surround it in some
                    subject.split(")").collect::<Vec<&str>>()[0].to_string()
                },
            };

            // apply the cast to the subject
            cleaned = cleaned.replace(
                &format!("{}{}", cast, subject),
                &format!(
                    "{}({})",
                    cast_types[0],
                    match subject.split(")").collect::<Vec<&str>>().first() {
                        Some(subject) => subject.to_string(),
                        None => subject
                    }
                )
            );

            // attempt to cast again
            cleaned = convert_bitmask_to_casting(cleaned);
        },
        None => {

            match AND_BITMASK_REGEX_2.find(&cleaned) {
                Some(bitmask) => {
                    let cast = bitmask.as_str();
                    let cast_size = cast.matches("ff").count();
                    let (_, cast_types) = byte_size_to_type(cast_size);
        
                    // get the cast subject
                    let mut subject = match cleaned.get(0..bitmask.start()).unwrap().replace(";",  "").split("=").collect::<Vec<&str>>().last() {
                        Some(subject) => subject.to_string(),
                        None => cleaned.get(0..bitmask.start()).unwrap().replace(";",  "").to_string(),
                    };
                    
                    // attempt to find matching parentheses
                    let subject_indices = find_balanced_parentheses(subject.to_string());

                    println!("subject: {}, indices: {:?}", subject, subject_indices);
                    subject = match subject_indices.2 {
                        true => {
        
                            // get the matched subject
                            match subject.get(subject_indices.0..subject_indices.1) {
                                Some(subject) => subject.to_string(),
                                None => match subject.split("(").collect::<Vec<&str>>().last() {
                                    Some(subject) => subject.to_string(),
                                    None => subject
                                },
                            }
                        },
                        false => {
                            
                            // subject doesn't contain parentheses, so surround it in some
                            match subject.split("(").collect::<Vec<&str>>().last() {
                                Some(subject) => subject.to_string(),
                                None => subject
                            }
                        },
                    };
                    println!("{}{}", subject, cast);
                    // apply the cast to the subject
                    cleaned = cleaned.replace(
                        &format!("{}{}", subject, cast),
                        &format!(
                            "{}({})",
                            cast_types[0],
                            match subject.split("(").collect::<Vec<&str>>().last() {
                                Some(subject) => subject.to_string(),
                                None => subject
                            }
                        )
                    );
        
                    // attempt to cast again
                    cleaned = convert_bitmask_to_casting(cleaned);
                },
                None => {}
            }
            
        }
    }

    cleaned
}

fn simplify_casts(line: String) -> String {
    let mut cleaned = line;

    // remove unnecessary casts
    
    
    cleaned
}

fn convert_iszero_logic_flip(line: String) -> String {
    let mut cleaned = line;

    if cleaned.contains("iszero") {
        cleaned = cleaned.replace("iszero", "!");
    }

    cleaned
}

pub fn postprocess(line: String) -> String {
    let mut cleaned = line;

    // Find and convert all castings
    cleaned = convert_bitmask_to_casting(cleaned);

    // Remove all repetitive casts
    cleaned = simplify_casts(cleaned);

    // Find and flip == / != signs for all instances of ISZERO
    cleaned = convert_iszero_logic_flip(cleaned);

    cleaned
}
