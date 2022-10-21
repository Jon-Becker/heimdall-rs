use heimdall_common::{ether::evm::types::{byte_size_to_type, find_cast}, utils::strings::{find_balanced_parentheses, find_balanced_parentheses_backwards}};

use super::{constants::{AND_BITMASK_REGEX, AND_BITMASK_REGEX_2, NON_ZERO_BYTE_REGEX}};

fn convert_bitmask_to_casting(line: String) -> String {
    let mut cleaned = line;

    match AND_BITMASK_REGEX.find(&cleaned) {
        Some(bitmask) => {
            let cast = bitmask.as_str();
            let cast_size = NON_ZERO_BYTE_REGEX.find_iter(&cast).count();
            let (_, cast_types) = byte_size_to_type(cast_size);

            // get the cast subject
            let mut subject = cleaned.get(bitmask.end()..).unwrap().replace(";",  "");
            
            // attempt to find matching parentheses
            let subject_indices = find_balanced_parentheses(subject.to_string());
            subject = match subject_indices.2 {
                true => {

                    // get the subject as hte substring between the balanced parentheses found in unbalanced subject
                    subject[subject_indices.0..subject_indices.1].to_string()
                },
                false => {

                    // this shouldn't happen, but if it does, just return the subject.
                    //TODO add this to verbose logs
                    subject
                },
            };

            // apply the cast to the subject
            cleaned = cleaned.replace(
                &format!("{}{}", cast, subject),
                &format!("{}{}", cast_types[0], subject),
            );

            // attempt to cast again
            cleaned = convert_bitmask_to_casting(cleaned);
        },
        None => {

            match AND_BITMASK_REGEX_2.find(&cleaned) {
                Some(bitmask) => {
                    let cast = bitmask.as_str();
                    let cast_size = NON_ZERO_BYTE_REGEX.find_iter(&cast).count();
                    let (_, cast_types) = byte_size_to_type(cast_size);
        
                    // get the cast subject
                    let mut subject = match cleaned.get(0..bitmask.start()).unwrap().replace(";",  "").split("=").collect::<Vec<&str>>().last() {
                        Some(subject) => subject.to_string(),
                        None => cleaned.get(0..bitmask.start()).unwrap().replace(";",  "").to_string(),
                    };

                    // attempt to find matching parentheses
                    let subject_indices = find_balanced_parentheses_backwards(subject.to_string());

                    subject = match subject_indices.2 {
                        true => {
        
                            // get the subject as hte substring between the balanced parentheses found in unbalanced subject
                            subject[subject_indices.0..subject_indices.1].to_string()
                        },
                        false => {
                            
                            // this shouldn't happen, but if it does, just return the subject.
                            //TODO add this to verbose logs
                            subject
                        },
                    };

                    // apply the cast to the subject
                    cleaned = cleaned.replace(
                        &format!("{}{}", subject, cast),
                        &format!("{}{}", cast_types[0], subject),
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

fn simplify_casts(line: String, outer_cast: Option<String>) -> String {
    let mut cleaned = line;

    // remove unnecessary casts
    let (cast_start, cast_end, cast_type) = find_cast(cleaned.to_string());
    
    match cast_type {
        Some(cast) => {
            let cleaned_cast_pre = cleaned[0..cast_start].to_string();
            let cleaned_cast_post = cleaned[cast_end..].to_string();
            let cleaned_cast = cleaned[cast_start..cast_end].to_string().replace(&cast, "");

            cleaned = format!("{}{}{}", cleaned_cast_pre, cleaned_cast, cleaned_cast_post);

            // check if there are remaining casts
            let (_, _, remaining_cast_type) = find_cast(cleaned_cast_post.clone());
            match remaining_cast_type {
                Some(_) => {

                    // a cast is remaining, simplify it
                    let mut recursive_cleaned = format!("{}{}", cleaned_cast_pre, cleaned_cast);
                    recursive_cleaned.push_str(
                        simplify_casts(cleaned_cast_post, None).as_str()
                    );
                    cleaned = recursive_cleaned;
                },
                None => {}
            }
        },
        None => {}
    }

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
    cleaned = simplify_casts(cleaned, None);

    // Find and flip == / != signs for all instances of ISZERO
    cleaned = convert_iszero_logic_flip(cleaned);

    cleaned
}
