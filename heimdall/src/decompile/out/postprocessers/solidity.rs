use super::super::super::constants::{
    AND_BITMASK_REGEX, AND_BITMASK_REGEX_2, DIV_BY_ONE_REGEX, MEM_ACCESS_REGEX, MUL_BY_ONE_REGEX,
    NON_ZERO_BYTE_REGEX,
};
use crate::decompile::constants::{ENCLOSED_EXPRESSION_REGEX, MEM_VAR_REGEX, STORAGE_ACCESS_REGEX};
use heimdall_common::{
    constants::TYPE_CAST_REGEX,
    ether::{
        evm::types::{byte_size_to_type, find_cast},
        signatures::{ResolvedError, ResolvedLog},
    },
    utils::strings::{
        base26_encode, classify_token, find_balanced_encapsulator,
        find_balanced_encapsulator_backwards, tokenize, TokenType,
    },
};
use indicatif::ProgressBar;
use lazy_static::lazy_static;
use std::{collections::HashMap, sync::Mutex};

lazy_static! {
    static ref MEM_LOOKUP_MAP: Mutex<HashMap<String, String>> = Mutex::new(HashMap::new());
    static ref STORAGE_LOOKUP_MAP: Mutex<HashMap<String, String>> = Mutex::new(HashMap::new());
    static ref VARIABLE_MAP: Mutex<HashMap<String, String>> = Mutex::new(HashMap::new());
    static ref MEMORY_TYPE_MAP: Mutex<HashMap<String, String>> = Mutex::new(HashMap::new());
    static ref STORAGE_TYPE_MAP: Mutex<HashMap<String, String>> = Mutex::new(HashMap::new());
}

/// Convert bitwise operations to a variable type cast
///
/// # Arguments
/// line: String - the line to convert
///
/// # Returns
/// String - the converted line
///
/// # Example
/// ```no_run
/// let line = "(0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff) & (arg0);".to_string();
/// let converted = convert_bitmask_to_casting(line);
/// assert_eq!(converted, "uint256(arg0);");
///
/// let line = "(arg0) & (0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff);".to_string();
/// let converted = convert_bitmask_to_casting(line);
/// assert_eq!(converted, "uint256(arg0);");
/// ```
fn convert_bitmask_to_casting(line: String) -> String {
    let mut cleaned = line;

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
            cleaned = convert_bitmask_to_casting(cleaned);
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
                cleaned = convert_bitmask_to_casting(cleaned);
            }
        }
    }

    cleaned
}

/// Removes unnecessary casts
///
/// # Arguments
/// line: String - the line to simplify
///
/// # Returns
/// String - the simplified line
///
/// # Example
/// ```no_run
/// let line = "uint256(uint256(arg0))".to_string();
/// let simplified = simplify_casts(line);
/// assert_eq!(simplified, "uint256(arg0)");
/// ```
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

/// Simplifies expressions by removing unnecessary parentheses
///
/// # Arguments
/// line: String - the line to simplify
///
/// # Returns
/// String - the simplified line
///
/// # Example
/// ```no_run
/// let line = "((a + b))".to_string();
/// let simplified = simplify_parentheses(line);
/// assert_eq!(simplified, "a + b");
///
/// let line = "((a + b) * (c + d))".to_string();
/// let simplified = simplify_parentheses(line);
/// assert_eq!(simplified, "(a + b) * (c + d)");
///
/// let line = "if ((((a + b) * (c + d))) > ((arg0 * 10000)) {".to_string();
/// let simplified = simplify_parentheses(line);
/// assert_eq!(simplified, "if ((a + b) * (c + d)) > arg0 * 10000 {");
/// ```
fn simplify_parentheses(line: String, paren_index: usize) -> String {
    // helper function to determine if parentheses are necessary
    fn are_parentheses_unnecessary(expression: String) -> bool {
        // safely grab the first and last chars
        let first_char = expression.get(0..1).unwrap_or("");
        let last_char = expression.get(expression.len() - 1..expression.len()).unwrap_or("");

        // if there is a negation of an expression, remove the parentheses
        // helps with double negation
        if first_char == "!" && last_char == ")" {
            return true
        }

        // remove the parentheses if the expression is within brackets
        if first_char == "[" && last_char == "]" {
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
        if expression.contains("memory ret") {
            return false
        }

        // handle the inside of the expression
        let inside = match expression.get(2..expression.len() - 2) {
            Some(x) => ENCLOSED_EXPRESSION_REGEX.replace(x, "x").to_string(),
            None => "".to_string(),
        };

        let inner_tokens = tokenize(&inside);
        return !inner_tokens.iter().any(|tk| classify_token(tk) == TokenType::Operator)
    }

    let mut cleaned = line;

    // skip lines that are defining a function
    if cleaned.contains("function") {
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

            // remove double negation, if one was created
            if cleaned.contains("!!") {
                cleaned = cleaned.replace("!!", "");
            }

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

/// Converts memory and storage accesses to variables
///
/// # Arguments
/// line: String - the line to convert
///
/// # Returns
/// String - the converted line
///
/// # Example
/// ```no_run
/// let line = "memory[0x40] = 0x60;".to_string();
/// let converted = convert_access_to_variable(line);
/// assert_eq!(converted, "var_a = 0x60;");
///
/// let line = "storage[0x0] = 0x0;".to_string();
/// let converted = convert_access_to_variable(line);
/// assert_eq!(converted, "stor_a = 0x0;");
/// ```
fn convert_access_to_variable(line: String) -> String {
    let mut cleaned = line.clone();

    // reset the mem_map if the line is a function definition
    if cleaned.contains("function") {
        let mut mem_map = MEM_LOOKUP_MAP.lock().unwrap();
        *mem_map = HashMap::new();
        drop(mem_map);
        let mut var_map = VARIABLE_MAP.lock().unwrap();
        *var_map = HashMap::new();
        drop(var_map);
    }

    // find a memory access
    let memory_access = match MEM_ACCESS_REGEX.find(&cleaned).unwrap() {
        Some(x) => x.as_str(),
        None => "",
    };

    // since the regex is greedy, match the memory brackets
    let matched_loc = find_balanced_encapsulator(memory_access, ('[', ']'));
    if let true = matched_loc.2 {
        let mut mem_map = MEM_LOOKUP_MAP.lock().unwrap();

        // safe to unwrap since we know these indices exist
        let memloc = format!("memory{}", memory_access.get(matched_loc.0..matched_loc.1).unwrap());

        let variable_name = match mem_map.get(&memloc) {
            Some(loc) => loc.to_owned(),
            None => {
                // add the memory location to the map
                let idex = mem_map.len() + 1;

                // get the variable name
                let variable_name = format!("var_{}", base26_encode(idex));

                // add the variable to the map
                mem_map.insert(memloc.clone(), variable_name.clone());
                variable_name
            }
        };

        // unlock the map
        drop(mem_map);

        // upadte the memory name
        cleaned = cleaned.replace(memloc.as_str(), &variable_name);

        // recurse to replace any other memory accesses
        cleaned = convert_access_to_variable(cleaned);
    }

    // find a storage access
    let storage_access = match STORAGE_ACCESS_REGEX.find(&cleaned).unwrap() {
        Some(x) => x.as_str(),
        None => return cleaned,
    };

    // since the regex is greedy, match the memory brackets
    let matched_loc = find_balanced_encapsulator(storage_access, ('[', ']'));
    if let true = matched_loc.2 {
        let mut stor_map = STORAGE_LOOKUP_MAP.lock().unwrap();

        // safe to unwrap since we know these indices exist
        let memloc =
            format!("storage{}", storage_access.get(matched_loc.0..matched_loc.1).unwrap());

        let variable_name = match stor_map.get(&memloc) {
            Some(loc) => loc.to_owned(),
            None => {
                // add the memory location to the map
                let idex = stor_map.len() + 1;

                // get the variable name
                if memloc.contains("keccak256") {
                    let keccak_key = find_balanced_encapsulator(&memloc, ('(', ')'));

                    let variable_name = format!(
                        "stor_map_{}[{}]",
                        base26_encode(idex),
                        memloc.get(keccak_key.0 + 1..keccak_key.1 - 1).unwrap_or("?")
                    );

                    // add the variable to the map
                    stor_map.insert(memloc.clone(), variable_name.clone());
                    variable_name
                } else {
                    let variable_name = format!("stor_{}", base26_encode(idex));

                    // add the variable to the map
                    stor_map.insert(memloc.clone(), variable_name.clone());
                    variable_name
                }
            }
        };

        // unlock the map
        drop(stor_map);

        // upadte the memory name
        cleaned = cleaned.replace(memloc.as_str(), &variable_name);

        // recurse to replace any other memory accesses
        cleaned = convert_access_to_variable(cleaned);
    }

    // if the memory access is an instantiation, save it
    if cleaned.contains(" = ") {
        let instantiation: Vec<String> =
            cleaned.split(" = ").collect::<Vec<&str>>().iter().map(|x| x.to_string()).collect();

        let mut var_map = VARIABLE_MAP.lock().unwrap();
        var_map.insert(instantiation[0].clone(), instantiation[1].replace(';', ""));
        drop(var_map);
    } else {
        // if var_map doesn't contain the variable, add it
        let mut var_map = VARIABLE_MAP.lock().unwrap();
        if var_map.get(&cleaned).is_none() {
            var_map.insert(cleaned.clone(), "".to_string());
            drop(var_map);
        }
    }

    // now we need to check if we should infer types if storage is being assigned
    if line.contains("storage") {
        // infer type of storage slot & add to storage variable map
        inherit_infer_storage_type(line);
    }

    cleaned
}

/// Checks if the current line contains an unnecessary assignment
///
/// # Arguments
/// line: String - the line to check
/// lines: &Vec<&String> - the lines of the contract. only includes lines after the current line
///
/// # Returns
/// bool - whether or not the line contains an unnecessary assignment
///
/// # Example
/// ```no_run
/// let line = "var_a = arg0;".to_string();
/// let lines = vec![
///     "var_b = var_a;",
///     "var_c = var_b;",
/// ].iter().map(|x| x.to_string()).collect();
///
/// let contains_unnecessary = contains_unnecessary_assignment(line, &lines);
/// assert_eq!(contains_unnecessary, false);
///
/// let line = "var_a = arg0;".to_string();
/// let lines = vec![
///     "var_b = arg1;",
///     "var_c = var_b;",
/// ].iter().map(|x| x.to_string()).collect();
/// assert_eq!(contains_unnecessary_assignment(line, &lines), true);
/// ```
fn contains_unnecessary_assignment(line: String, lines: &Vec<&String>) -> bool {
    // skip lines that don't contain an assignment, or contain a return or external calls
    if !line.contains(" = ") || line.contains("bool success") || line.contains("return") {
        return false
    }

    // get var name
    let var_name = line.split(" = ").collect::<Vec<&str>>()[0].split(' ').collect::<Vec<&str>>()
        [line.split(" = ").collect::<Vec<&str>>()[0].split(' ').collect::<Vec<&str>>().len() - 1];

    // skip lines that contain assignments to storage
    if var_name.contains("stor_") {
        return false
    }

    //remove unused vars
    for x in lines {
        // break if the line contains a function definition
        if x.contains("function") {
            break
        }

        if x.contains(" = ") {
            let assignment = x.split(" = ").map(|x| x.trim()).collect::<Vec<&str>>();
            if assignment[1].contains(var_name) {
                return false
            } else if assignment[0].contains(var_name) {
                return true
            }
        } else if x.contains(var_name) {
            return false
        }
    }

    true
}

/// Moves casts to the declaration
///
/// # Arguments
/// line: String - the line to convert
///
/// # Returns
/// String - the converted line
///
/// # Example
/// ```no_run
/// let line = "var_a = uint256(arg0);".to_string();
/// let converted = move_casts_to_declaration(line);
/// assert_eq!(converted, "uint256 var_a = arg0;");
/// ```
fn move_casts_to_declaration(line: String) -> String {
    let cleaned = line;

    // if the line doesn't contain an instantiation, return
    if !cleaned.contains(" = ") {
        return cleaned
    }

    let instantiation = cleaned.split(" = ").collect::<Vec<&str>>();

    // get the outermost cast
    match TYPE_CAST_REGEX.find(instantiation[1]).unwrap() {
        Some(x) => {
            // the match must occur at index 0
            if x.start() != 0 {
                return cleaned
            }

            // find the matching close paren
            let (paren_start, paren_end, _) =
                find_balanced_encapsulator(&instantiation[1], ('(', ')'));

            // the close paren must be at the end of the expression
            if paren_end != instantiation[1].len() - 1 {
                return cleaned
            }

            // get the inside of the parens
            let cast_expression = instantiation[1].get(paren_start + 1..paren_end - 1).unwrap();

            format!("{} {} = {};", x.as_str().replace('(', ""), instantiation[0], cast_expression)
        }
        None => cleaned,
    }
}

/// Replaces an expression with a variable, if the expression matches an existing variable
///
/// # Arguments
/// line: String - the line to convert
///
/// # Returns
/// String - the converted line
///
/// # Example
/// ```no_run
/// let line = "var_a = arg0 + arg1;".to_string();
/// let converted = replace_expression_with_var(line);
/// assert_eq!(converted, "var_a = var_b + var_c;");
/// ```
fn replace_expression_with_var(line: String) -> String {
    let mut cleaned = line;

    let var_map = VARIABLE_MAP.lock().unwrap();

    // skip function definitions
    if cleaned.contains("function") {
        return cleaned
    }

    // iterate over variable map
    for (var, expression) in var_map.iter() {
        // skip numeric expressions
        if expression.parse::<u128>().is_ok() {
            continue
        }

        // skip expressions that are already variables. i.e, check if they contain a space
        if !expression.contains(' ') {
            continue
        }

        // replace the expression with the variable
        if cleaned.contains(expression) && !cleaned.starts_with(var) {
            cleaned = cleaned.replace(expression, var);
        }
    }

    // drop the mutex
    drop(var_map);

    cleaned
}

/// Inherits or infers typings for a memory access
///
/// # Arguments
/// line: String - the line to convert
///
/// # Returns
/// String - the converted line
fn inherit_infer_mem_type(line: String) -> String {
    let mut cleaned = line.clone();
    let mut type_map = MEMORY_TYPE_MAP.lock().unwrap();

    // if the line contains a function definition, wipe the type map and get arg types
    if line.contains("function") {
        type_map.clear();
        let args = line.split('(').collect::<Vec<&str>>()[1].split(')').collect::<Vec<&str>>()[0]
            .split(',')
            .collect::<Vec<&str>>();
        for arg in args {
            let arg = arg.trim();

            // get type and name
            let arg_type = arg.split(' ').collect::<Vec<&str>>()
                [..arg.split(' ').collect::<Vec<&str>>().len() - 1]
                .join(" ");
            let arg_name = arg.split(' ').collect::<Vec<&str>>()
                [arg.split(' ').collect::<Vec<&str>>().len() - 1];

            // add to type map
            type_map.insert(arg_name.to_string(), arg_type.to_string());
        }
    }

    // if the line does not contains an instantiation, return
    if !line.contains(" = ") {
        return cleaned
    }

    let instantiation = line.split(" = ").collect::<Vec<&str>>();
    let var_type = instantiation[0].split(' ').collect::<Vec<&str>>()
        [..instantiation[0].split(' ').collect::<Vec<&str>>().len() - 1]
        .join(" ");
    let var_name = instantiation[0].split(' ').collect::<Vec<&str>>()
        [instantiation[0].split(' ').collect::<Vec<&str>>().len() - 1];

    // add to type map, if the variable is typed
    if !var_type.is_empty() {
        type_map.insert(var_name.to_string(), var_type);
    }
    // inherit infer types for memory
    else if !line.starts_with("storage") {
        // infer the type from args and vars in the expression
        for (var, var_type) in type_map.clone().iter() {
            if cleaned.contains(var) && !type_map.contains_key(var_name) && !var_type.is_empty() {
                cleaned = format!("{var_type} {cleaned}");
                type_map.insert(var_name.to_string(), var_type.to_string());
                break
            }
        }
    }

    cleaned
}

/// Inherits or infers typings for a storage access
///
/// # Arguments
/// line: String - the line to convert
///
/// # Returns
/// String - the converted line
fn inherit_infer_storage_type(line: String) {
    let type_map = MEMORY_TYPE_MAP.lock().unwrap();
    let mut storage_map = STORAGE_TYPE_MAP.lock().unwrap();
    let storage_lookup_map = STORAGE_LOOKUP_MAP.lock().unwrap();
    let var_map = VARIABLE_MAP.lock().unwrap();

    let instantiation = line.split(" = ").collect::<Vec<&str>>();
    let var_name = instantiation[0].split(' ').collect::<Vec<&str>>()
        [instantiation[0].split(' ').collect::<Vec<&str>>().len() - 1];

    // inherit infer types for storage
    if var_name.starts_with("storage") {
        // copy the line to a mut
        let mut line = line.clone();

        // get the storage slot
        let storage_access = match STORAGE_ACCESS_REGEX.find(instantiation[0]).unwrap() {
            Some(x) => x.as_str(),
            None => return,
        };

        // since the regex is greedy, match the memory brackets
        let matched_loc = find_balanced_encapsulator(&storage_access, ('[', ']'));
        if !matched_loc.2 {
            return
        }
        let mut storage_slot =
            format!("storage{}", storage_access.get(matched_loc.0..matched_loc.1).unwrap());

        // get the storage slot name from storage_lookup_map
        let mut var_name = match storage_lookup_map.get(&storage_slot) {
            Some(var_name) => var_name.to_owned(),
            None => return,
        };

        // if the storage_slot is a variable, replace it with the value
        // ex: storage[var_b] => storage[keccak256(var_a)]
        // helps with type inference
        if MEM_VAR_REGEX.is_match(&storage_slot).unwrap() {
            for (var, value) in var_map.clone().iter() {
                if storage_slot.contains(var) {
                    line = line.replace(var, value);
                    storage_slot = storage_slot.replace(var, value);
                }
            }
        }

        // default type is bytes32
        let mut lhs_type = "bytes32".to_string();
        let mut rhs_type = "bytes32".to_string();

        // if the storage slot contains a keccak256 call, this is a mapping and we will need to pull
        // types from both the lhs and rhs
        if storage_slot.contains("keccak256") {
            var_name = var_name.split('[').collect::<Vec<&str>>()[0].to_string();

            // replace the storage slot in rhs with a placeholder
            // this will prevent us from pulling bad types from the rhs
            if instantiation.len() > 2 {
                let rhs: String = instantiation[1].replace(&storage_slot, "_");

                // find vars in lhs or rhs
                for (var, var_type) in type_map.clone().iter() {
                    // check for vars in lhs
                    if storage_slot.contains(var) && !var_type.is_empty() {
                        lhs_type = var_type.to_string();

                        // continue, so we cannot use this var in rhs
                        continue
                    }

                    // check for vars in rhs
                    if rhs.contains(var) && !var_type.is_empty() {
                        rhs_type = var_type.to_string();
                    }
                }
            }

            // add to type map
            let mapping_type = format!("mapping({lhs_type} => {rhs_type})");
            storage_map.insert(var_name, mapping_type);
        } else {
            // get the type of the rhs
            for (var, var_type) in type_map.clone().iter() {
                if line.contains(var) && !var_type.is_empty() {
                    rhs_type = var_type.to_string();
                }
            }

            // add to type map
            storage_map.insert(var_name, rhs_type);
        }
    } else {
        for (access, var_name) in storage_lookup_map.iter() {
            if line.contains(access) {
                let var_name = var_name.split('[').collect::<Vec<&str>>()[0].to_string();

                // handle mappings differently
                if access.contains("keccak") {
                    let mut lhs_type = String::from("bytes32");
                    let mut rhs_type = String::from("bytes32");

                    // get the type of the access
                    for (var, var_type) in type_map.clone().iter() {
                        if access.contains(var) && !var_type.is_empty() {
                            lhs_type = var_type.to_string();
                        }
                    }

                    // replace the access in rhs with a placeholder
                    // this will prevent us from pulling bad types from the rhs
                    let rhs: String = line.replace(access, "_");

                    // get the type of the rhs
                    for (var, var_type) in type_map.clone().iter() {
                        if rhs.contains(var) && !var_type.is_empty() {
                            rhs_type = var_type.to_string();
                        }
                    }

                    // add to type map
                    let mapping_type = format!("mapping({lhs_type} => {rhs_type})");
                    storage_map.insert(var_name.to_string(), mapping_type);
                } else {
                    let mut handled = false;

                    // get the type of the rhs
                    for (var, var_type) in type_map.clone().iter() {
                        if line.contains(var) && !var_type.is_empty() {
                            storage_map.insert(var_name.to_string(), var_type.to_string());
                            handled = true;
                        }
                    }

                    if !handled {
                        storage_map.insert(var_name.to_string(), "bytes32".to_string());
                    }
                }
            }
        }
    }
}

/// Replaces resolved errors and events
///
/// # Arguments
/// line: String - the line to convert
///
/// # Returns
/// String - the converted line
///
/// # Example
/// ```no_run
/// let line = "revert CustomError_00000000(arg0);".to_string();
/// let converted = replace_resolved(line);
/// assert_eq!(converted, "CustomError_00000000(arg0);");
///
/// let line = "revert CustomError_00000001(arg0);".to_string();
/// let converted = replace_resolved(line);
/// assert_eq!(converted, "UrMom(arg0);");
fn replace_resolved(
    line: String,
    all_resolved_errors: HashMap<String, ResolvedError>,
    all_resolved_events: HashMap<String, ResolvedLog>,
) -> String {
    let mut cleaned = line;

    // line must contain CustomError_ or Event_
    if !cleaned.contains("CustomError_") && !cleaned.contains("Event_") {
        return cleaned
    }

    // not the best way to do it, can perf later
    for (selector, error) in all_resolved_errors.iter() {
        let selector = selector.get(0..8).unwrap_or("00000000");
        if cleaned.contains(selector) {
            cleaned = cleaned.replace(&format!("CustomError_{selector}"), &error.name);
        }
    }

    for (selector, event) in all_resolved_events.iter() {
        let selector = selector.get(0..8).unwrap_or("00000000");
        if cleaned.contains(selector) {
            cleaned = cleaned.replace(&format!("Event_{selector}"), &event.name);
        }
    }

    cleaned
}

/// Simplifies arithmatic by removing unnecessary operations
///
/// # Arguments
/// line: String - the line to convert
///
/// # Returns
/// String - the converted line
///
/// # Example
/// ```no_run
/// let line = "var_a = 1 * 2;".to_string();
/// let converted = simplify_arithmatic(line);
/// assert_eq!(converted, "var_a = 2;");
/// ```
fn simplify_arithmatic(line: String) -> String {
    let cleaned = DIV_BY_ONE_REGEX.replace_all(&line, "");
    let cleaned = MUL_BY_ONE_REGEX.replace_all(&cleaned, "");

    // remove double negation
    cleaned.replace("!!", "")
}

fn cleanup(
    line: String,
    all_resolved_errors: HashMap<String, ResolvedError>,
    all_resolved_events: HashMap<String, ResolvedLog>,
) -> String {
    let mut cleaned = line;

    // skip comments
    if cleaned.starts_with('/') {
        return cleaned
    }

    // Find and convert all castings
    cleaned = convert_bitmask_to_casting(cleaned);

    // Remove all repetitive casts
    cleaned = simplify_casts(cleaned);

    // Remove all unnecessary parentheses
    cleaned = simplify_parentheses(cleaned, 0);

    // Convert all memory[] and storage[] accesses to variables, also removes unused variables
    cleaned = convert_access_to_variable(cleaned);

    // Use variable names where possible
    cleaned = replace_expression_with_var(cleaned);

    // Move all outer casts in instantiation to the variable declaration
    cleaned = move_casts_to_declaration(cleaned);

    // Inherit or infer types from expressions
    cleaned = inherit_infer_mem_type(cleaned);

    // Replace resolved errors and events
    cleaned = replace_resolved(cleaned, all_resolved_errors, all_resolved_events);

    // Simplify arithmatic
    cleaned = simplify_arithmatic(cleaned);

    cleaned
}

fn finalize(lines: Vec<String>, bar: &ProgressBar) -> Vec<String> {
    let mut cleaned_lines: Vec<String> = Vec::new();
    let mut function_count = 0;

    // remove unused assignments
    for (i, line) in lines.iter().enumerate() {
        // check if we need to insert storage vars
        if cleaned_lines.last().unwrap_or(&"".to_string()).contains("DecompiledContract") {
            let mut storage_var_lines: Vec<String> = vec!["".to_string()];

            // insert storage vars
            for (var_name, var_type) in STORAGE_TYPE_MAP.lock().unwrap().clone().iter() {
                storage_var_lines.push(format!(
                    "{} public {};",
                    var_type.replace(" memory", ""),
                    var_name
                ));
            }

            // sort storage vars by length, shortest first, then alphabetically
            storage_var_lines.sort_by(|a, b| a.len().cmp(&b.len()).then(a.cmp(b)));

            // if we have storage vars, push to cleaned lines
            if storage_var_lines.len() > 1 {
                cleaned_lines.append(&mut storage_var_lines);
            }
        }

        // update progress bar
        if line.contains("function") {
            function_count += 1;
            bar.set_message(format!("postprocessed {function_count} functions"));
        }

        // cleaned_lines.push(line.to_string());
        if !contains_unnecessary_assignment(
            line.trim().to_string(),
            &lines[i + 1..].iter().collect::<Vec<_>>(),
        ) {
            cleaned_lines.push(line.to_string());
        } else {
            continue
        }
    }

    cleaned_lines
}

fn indent_lines(lines: Vec<String>) -> Vec<String> {
    let mut indentation: usize = 0;
    let mut indented_lines: Vec<String> = Vec::new();

    for line in lines {
        // dedent due to closing braces
        if line.starts_with('}') {
            indentation = indentation.saturating_sub(1);
        }

        // apply postprocessing and indentation
        indented_lines.push(format!("{}{}", " ".repeat(indentation * 4), line));

        // indent due to opening braces
        if line.split("//").collect::<Vec<&str>>().first().unwrap().trim().ends_with('{') {
            indentation += 1;
        }
    }

    indented_lines
}

pub fn postprocess(
    lines: Vec<String>,
    all_resolved_errors: HashMap<String, ResolvedError>,
    all_resolved_events: HashMap<String, ResolvedLog>,
    bar: &ProgressBar,
) -> Vec<String> {
    let mut function_count = 0;
    let mut cleaned_lines: Vec<String> = Vec::new();

    // clean up each line using postprocessing techniques
    for line in lines {
        // update progress bar
        if line.contains("function") {
            function_count += 1;
            bar.set_message(format!("postprocessed {function_count} functions"));
        }

        cleaned_lines.push(cleanup(
            line.to_string(),
            all_resolved_errors.clone(),
            all_resolved_events.clone(),
        ));
    }

    // run finalizing postprocessing, which need to operate on cleaned lines
    indent_lines(finalize(cleaned_lines, bar))
}
