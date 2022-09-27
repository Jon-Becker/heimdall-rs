use super::constants::AND_BITMASK_REGEX;

fn convert_bitmask_to_casting(line: String) -> String {
    let mut cleaned = line;

    match AND_BITMASK_REGEX.find(&cleaned) {
        Some(bitmask) => {
            let cast = bitmask.as_str();

            cleaned = cleaned.replace(cast, "");

            // attempt to cast again
            cleaned = convert_bitmask_to_casting(cleaned);
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

    // Find and flip == / != signs for all instances of ISZERO
    cleaned = convert_iszero_logic_flip(cleaned);

    cleaned
}
