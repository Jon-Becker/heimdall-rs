use super::super::super::constants::MEM_ACCESS_REGEX;
use crate::{
    decompile::constants::{MEM_VAR_REGEX, STORAGE_ACCESS_REGEX},
    error::Error,
};
use heimdall_common::{
    constants::TYPE_CAST_REGEX,
    ether::{
        lexers::cleanup::{
            convert_bitmask_to_casting, simplify_arithmatic, simplify_casts, simplify_parentheses,
        },
        signatures::{ResolvedError, ResolvedLog},
    },
    utils::strings::{base26_encode, find_balanced_encapsulator},
};
use indicatif::ProgressBar;
use lazy_static::lazy_static;
use std::{
    collections::{HashMap, HashSet},
    sync::Mutex,
};

lazy_static! {
    static ref MEM_LOOKUP_MAP: Mutex<HashMap<String, String>> = Mutex::new(HashMap::new());
    static ref STORAGE_LOOKUP_MAP: Mutex<HashMap<String, String>> = Mutex::new(HashMap::new());
    static ref VARIABLE_MAP: Mutex<HashMap<String, String>> = Mutex::new(HashMap::new());
    static ref MEMORY_TYPE_MAP: Mutex<HashMap<String, String>> = Mutex::new(HashMap::new());
    static ref STORAGE_TYPE_MAP: Mutex<HashMap<String, String>> = Mutex::new(HashMap::new());
    static ref MEMORY_TYPE_DECLARATION_SET: Mutex<HashSet<String>> = Mutex::new(HashSet::new());
}

/// Converts memory and storage accesses to variables
fn convert_access_to_variable(line: &str) -> Result<String, Error> {
    let mut cleaned = line.to_owned();

    // reset the mem_map if the line is a function definition
    if cleaned.contains("function") {
        let mut mem_map = MEM_LOOKUP_MAP.lock().expect("failed to obtain lock on mem_map");
        *mem_map = HashMap::new();
        drop(mem_map);
        let mut var_map = VARIABLE_MAP.lock().expect("failed to obtain lock on var_map");
        *var_map = HashMap::new();
        drop(var_map);
    }

    // find a memory access
    let memory_access = match MEM_ACCESS_REGEX.find(&cleaned).unwrap_or(None) {
        Some(x) => x.as_str(),
        None => "",
    };

    // since the regex is greedy, match the memory brackets
    if let Ok(memory_range) = find_balanced_encapsulator(memory_access, ('[', ']')) {
        let mut mem_map = MEM_LOOKUP_MAP
            .lock()
            .map_err(|_| Error::Generic("failed to obtain lock on mem_map".to_string()))?;

        // safe to unwrap since we know these indices exist
        let memloc = format!(
            "memory[{}]",
            memory_access
                .get(memory_range)
                .expect("impossible case: failed to get memory access after check")
        );

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
        cleaned = convert_access_to_variable(&cleaned)?;
    }

    // find a storage access
    let storage_access = match STORAGE_ACCESS_REGEX.find(&cleaned).unwrap_or(None) {
        Some(x) => x.as_str(),
        None => return Ok(cleaned.to_owned()),
    };

    // since the regex is greedy, match the memory brackets
    if let Ok(storage_range) = find_balanced_encapsulator(storage_access, ('[', ']')) {
        let mut stor_map = STORAGE_LOOKUP_MAP
            .lock()
            .map_err(|_| Error::Generic("failed to obtain lock on stor_map".to_string()))?;

        // safe to unwrap since we know these indices exist
        let memloc = format!(
            "storage{}",
            storage_access
                .get(storage_range)
                .expect("impossible case: failed to get storage access after check")
        );

        let variable_name = match stor_map.get(&memloc) {
            Some(loc) => loc.to_owned(),
            None => {
                // add the memory location to the map
                let idex = stor_map.len() + 1;

                // get the variable name
                if memloc.contains("keccak256") {
                    let keccak_range = find_balanced_encapsulator(&memloc, ('(', ')'))
                        .map_err(|_| Error::Generic("failed to find keccak256 key".to_string()))?;

                    let variable_name = format!(
                        "stor_map_{}[{}]",
                        base26_encode(idex),
                        memloc.get(keccak_range).unwrap_or("?")
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
        cleaned = convert_access_to_variable(&cleaned)?;
    }

    // if the memory access is an instantiation, save it
    if cleaned.contains(" = ") {
        let instantiation: Vec<String> =
            cleaned.split(" = ").collect::<Vec<&str>>().iter().map(|x| x.to_string()).collect();

        let mut var_map = VARIABLE_MAP
            .lock()
            .map_err(|_| Error::Generic("failed to obtain lock on var_map".to_string()))?;
        var_map.insert(instantiation[0].clone(), instantiation[1].replace(';', ""));
        drop(var_map);
    } else {
        // if var_map doesn't contain the variable, add it
        let mut var_map = VARIABLE_MAP
            .lock()
            .map_err(|_| Error::Generic("failed to obtain lock on var_map".to_string()))?;
        if var_map.get(&cleaned).is_none() {
            var_map.insert(cleaned.to_owned(), "".to_string());
            drop(var_map);
        }
    }

    // now we need to check if we should infer types if storage is being assigned
    if line.contains("storage") {
        // infer type of storage slot & add to storage variable map
        inherit_infer_storage_type(line)?;
    }

    Ok(cleaned.to_owned())
}

/// Checks if the current line contains an unnecessary assignment
fn contains_unnecessary_assignment(line: &str, lines: &Vec<&str>) -> bool {
    // skip lines that don't contain an assignment, or contain a return or external calls
    if !line.contains(" = ") || line.contains("bool success") || line.contains("return") {
        return false;
    }

    // get var name
    let var_name = line.split(" = ").collect::<Vec<&str>>()[0].split(' ').collect::<Vec<&str>>()
        [line.split(" = ").collect::<Vec<&str>>()[0].split(' ').collect::<Vec<&str>>().len() - 1];

    // skip lines that contain assignments to storage
    if var_name.contains("stor_") {
        return false;
    }

    //remove unused vars
    for x in lines {
        // break if the line contains a function definition
        if x.contains("function") {
            break;
        }

        if x.contains(" = ") {
            let assignment = x.split(" = ").map(|x| x.trim()).collect::<Vec<&str>>();
            if assignment[1].contains(var_name) {
                return false;
            } else if assignment[0].contains(var_name) {
                return true;
            }
        } else if x.contains(var_name) {
            return false;
        }
    }

    true
}

/// Moves casts to the declaration
fn move_casts_to_declaration(line: &str) -> Result<String, Error> {
    let cleaned = line;
    let mut type_declaration_set = MEMORY_TYPE_DECLARATION_SET
        .lock()
        .map_err(|_| Error::Generic("failed to obtain lock on type_declaration_set".to_string()))?;

    // if line contains "function" wipe the set
    if cleaned.contains("function") {
        type_declaration_set.clear();
        return Ok(cleaned.to_owned());
    }

    // if the line doesn't contain an instantiation, return
    if !cleaned.contains(" = ") {
        return Ok(cleaned.to_owned());
    }

    let instantiation = cleaned.split(" = ").collect::<Vec<&str>>();

    // get the outermost cast
    match TYPE_CAST_REGEX.find(instantiation[1]).unwrap_or(None) {
        Some(x) => {
            // the match must occur at index 0
            if x.start() != 0 {
                return Ok(cleaned.to_owned());
            }

            // find the matching close paren
            let cast_expr_range = find_balanced_encapsulator(instantiation[1], ('(', ')'))
                .map_err(|_| Error::Generic("failed to find cast expression".to_string()))?;

            // the close paren must be at the end of the expression
            if cast_expr_range.end + 1 != instantiation[1].len() - 1 {
                return Ok(cleaned.to_owned());
            }

            // get the inside of the parens
            let cast_expression = instantiation[1]
                .get(cast_expr_range)
                .expect("impossible case: failed to get cast expression after check");

            // build set key
            let set_key = format!("{}.{}", instantiation[0], x.as_str().replace('(', ""));

            // if the set doesn't contain the key, add the cast to the declaration
            if !type_declaration_set.contains(&set_key) {
                // add to set
                type_declaration_set.insert(set_key);
                Ok(format!(
                    "{} {} = {};",
                    x.as_str().replace('(', ""),
                    instantiation[0],
                    cast_expression
                ))
            } else {
                Ok(format!("{} = {};", instantiation[0], cast_expression))
            }
        }
        None => Ok(cleaned.to_owned()),
    }
}

/// Replaces an expression with a variable, if the expression matches an existing variable
fn replace_expression_with_var(line: &str) -> Result<String, Error> {
    let mut cleaned = line.to_owned();

    let var_map = VARIABLE_MAP
        .lock()
        .map_err(|_| Error::Generic("failed to obtain lock on var_map".to_string()))?;

    // skip function definitions
    if cleaned.contains("function") {
        return Ok(cleaned);
    }

    // iterate over variable map
    for (var, expression) in var_map.iter() {
        // skip numeric expressions
        if expression.parse::<u128>().is_ok() {
            continue;
        }

        // skip expressions that are already variables. i.e, check if they contain a space
        if !expression.contains(' ') {
            continue;
        }

        // replace the expression with the variable
        if cleaned.contains(expression) && !cleaned.starts_with(var) {
            cleaned = cleaned.replace(expression, var);
        }
    }

    // drop the mutex
    drop(var_map);

    Ok(cleaned)
}

/// Inherits or infers typings for a memory access
///
/// # Arguments
/// line: String - the line to convert
///
/// # Returns
/// String - the converted line
fn inherit_infer_mem_type(line: &str) -> Result<String, Error> {
    let mut cleaned = line.to_owned();
    let mut type_map = MEMORY_TYPE_MAP
        .lock()
        .map_err(|_| Error::Generic("failed to obtain lock on type_map".to_string()))?;

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
    if !line.contains(" = ") || line.trim().starts_with("stor") {
        return Ok(cleaned);
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
                break;
            }
        }
    }

    Ok(cleaned)
}

/// Inherits or infers typings for a storage access
///
/// # Arguments
/// line: String - the line to convert
///
/// # Returns
/// String - the converted line
fn inherit_infer_storage_type(line: &str) -> Result<(), Error> {
    let type_map = MEMORY_TYPE_MAP
        .lock()
        .map_err(|_| Error::Generic("failed to obtain lock on type_map".to_string()))?;
    let mut storage_map = STORAGE_TYPE_MAP
        .lock()
        .map_err(|_| Error::Generic("failed to obtain lock on storage_map".to_string()))?;
    let storage_lookup_map = STORAGE_LOOKUP_MAP
        .lock()
        .map_err(|_| Error::Generic("failed to obtain lock on storage_lookup_map".to_string()))?;
    let var_map = VARIABLE_MAP
        .lock()
        .map_err(|_| Error::Generic("failed to obtain lock on var_map".to_string()))?;

    let instantiation = line.split(" = ").collect::<Vec<&str>>();
    let var_name = instantiation[0].split(' ').collect::<Vec<&str>>()
        [instantiation[0].split(' ').collect::<Vec<&str>>().len() - 1];

    // inherit infer types for storage
    if var_name.starts_with("storage") {
        // copy the line to a mut
        let mut line = line.to_owned();

        // get the storage slot
        let storage_access = match STORAGE_ACCESS_REGEX.find(instantiation[0]).unwrap_or(None) {
            Some(x) => x.as_str(),
            None => return Ok(()),
        };

        // since the regex is greedy, match the memory brackets
        let matched_range = find_balanced_encapsulator(storage_access, ('[', ']'))
            .map_err(|_| Error::Generic("failed to find storage access".to_string()))?;

        let mut storage_slot = format!(
            "storage[{}]",
            storage_access
                .get(matched_range)
                .expect("impossible case: failed to get storage access after check")
        );

        // get the storage slot name from storage_lookup_map
        let mut var_name = match storage_lookup_map.get(&storage_slot) {
            Some(var_name) => var_name.to_owned(),
            None => return Ok(()),
        };

        // if the storage_slot is a variable, replace it with the value
        // ex: storage[var_b] => storage[keccak256(var_a)]
        // helps with type inference
        if MEM_VAR_REGEX.is_match(&storage_slot).unwrap_or(false) {
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
                        continue;
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

        Ok(())
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

        Ok(())
    }
}

/// Replaces resolved errors and events
fn replace_resolved(
    line: &str,
    all_resolved_errors: HashMap<String, ResolvedError>,
    all_resolved_events: HashMap<String, ResolvedLog>,
) -> String {
    let mut cleaned = line.to_owned();

    // line must contain CustomError_ or Event_
    if !cleaned.contains("CustomError_") && !cleaned.contains("Event_") {
        return cleaned;
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

/// Cleans up a line using postprocessing techniques
fn cleanup(
    line: &str,
    all_resolved_errors: HashMap<String, ResolvedError>,
    all_resolved_events: HashMap<String, ResolvedLog>,
) -> String {
    let mut cleaned = line.to_owned();

    // skip comments
    if cleaned.starts_with('/') {
        return cleaned;
    }

    // Find and convert all castings
    cleaned = convert_bitmask_to_casting(&cleaned).unwrap_or(cleaned);

    // Remove all repetitive casts
    cleaned = simplify_casts(&cleaned);

    // Remove all unnecessary parentheses
    cleaned = simplify_parentheses(&cleaned, 0).unwrap_or(cleaned);

    // Convert all memory[] and storage[] accesses to variables, also removes unused variables
    cleaned = convert_access_to_variable(&cleaned).unwrap_or(cleaned);

    // Use variable names where possible
    cleaned = replace_expression_with_var(&cleaned).unwrap_or(cleaned);

    // Move all outer casts in instantiation to the variable declaration
    cleaned = move_casts_to_declaration(&cleaned).unwrap_or(cleaned);

    // Inherit or infer types from expressions
    cleaned = inherit_infer_mem_type(&cleaned).unwrap_or(cleaned);

    // Replace resolved errors and events
    cleaned = replace_resolved(&cleaned, all_resolved_errors, all_resolved_events);

    // Simplify arithmatic
    cleaned = simplify_arithmatic(&cleaned);

    cleaned
}

/// Finalizes postprocessing by removing unnecessary assignments
fn finalize(lines: Vec<String>, bar: &ProgressBar) -> Result<Vec<String>, Error> {
    let mut cleaned_lines: Vec<String> = Vec::new();
    let mut function_count = 0;

    // remove unused assignments
    for (i, line) in lines.iter().enumerate() {
        // check if we need to insert storage vars
        if cleaned_lines.last().unwrap_or(&"".to_string()).contains("DecompiledContract") {
            let mut storage_var_lines: Vec<String> = vec!["".to_string()];

            // insert storage vars
            for (var_name, var_type) in STORAGE_TYPE_MAP
                .lock()
                .map_err(|_| {
                    Error::Generic("failed to obtain lock on storage_type_map".to_string())
                })?
                .clone()
                .iter()
            {
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
            line.trim(),
            &lines[i + 1..].iter().map(|x| x.as_str()).collect(),
        ) {
            cleaned_lines.push(line.to_string());
        } else {
            continue;
        }
    }

    Ok(cleaned_lines)
}

/// Indents lines
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
        if line
            .split("//")
            .collect::<Vec<&str>>()
            .first()
            .expect("impossible case: failed to get line after split")
            .trim()
            .ends_with('{')
        {
            indentation += 1;
        }
    }

    indented_lines
}

/// Postprocesses a decompiled contract
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
            &line,
            all_resolved_errors.clone(),
            all_resolved_events.clone(),
        ));
    }

    // run finalizing postprocessing, which need to operate on cleaned lines
    indent_lines(finalize(cleaned_lines.clone(), bar).unwrap_or(cleaned_lines))
}

#[cfg(test)]
mod tests {

    use std::collections::HashMap;

    use indicatif::ProgressBar;

    use crate::decompile::out::postprocessers::solidity::postprocess;

    #[test]
    fn test_bitmask_conversion() {
        let lines = vec![String::from(
            "(0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff) & (arg0);",
        )];

        assert_eq!(
            postprocess(lines, HashMap::new(), HashMap::new(), &ProgressBar::new(128)),
            vec![String::from("uint256(arg0);")]
        );
    }

    #[test]
    fn test_bitmask_conversion_mask_after() {
        let lines = vec![String::from(
            "(arg0) & (0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff);",
        )];

        assert_eq!(
            postprocess(lines, HashMap::new(), HashMap::new(), &ProgressBar::new(128)),
            vec![String::from("uint256(arg0);")]
        );
    }

    #[test]
    fn test_bitmask_conversion_unusual_mask() {
        let lines = vec![String::from(
            "(arg0) & (0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff00);",
        )];

        assert_eq!(
            postprocess(lines, HashMap::new(), HashMap::new(), &ProgressBar::new(128)),
            vec![String::from("uint248(arg0);")]
        );
    }

    #[test]
    fn test_simplify_casts() {
        let lines = vec![String::from("uint256(uint256(arg0));")];

        assert_eq!(
            postprocess(lines, HashMap::new(), HashMap::new(), &ProgressBar::new(128)),
            vec![String::from("uint256(arg0);")]
        );
    }

    #[test]
    fn test_simplify_casts_complex() {
        let lines = vec![
            String::from("ecrecover(uint256(uint256(arg0)), uint256(uint256(arg0)), uint256(uint256(uint256(arg0))));"),
        ];

        assert_eq!(
            postprocess(lines, HashMap::new(), HashMap::new(), &ProgressBar::new(128)),
            vec![String::from("ecrecover(uint256(arg0), uint256(arg0), uint256(arg0));")]
        );
    }

    #[test]
    fn test_iszero_flip() {
        let lines = vec![String::from("if (!(arg0)) {")];

        assert_eq!(
            postprocess(lines, HashMap::new(), HashMap::new(), &ProgressBar::new(128)),
            vec![String::from("if (!arg0) {")]
        );
    }

    #[test]
    fn test_iszero_flip_complex() {
        let lines = vec![String::from("if (!(!(arg0))) {")];

        assert_eq!(
            postprocess(lines, HashMap::new(), HashMap::new(), &ProgressBar::new(128)),
            vec![String::from("if (arg0) {")]
        );
    }

    #[test]
    fn test_iszero_flip_complex2() {
        let lines = vec![String::from("if (!(!(!(arg0)))) {")];

        assert_eq!(
            postprocess(lines, HashMap::new(), HashMap::new(), &ProgressBar::new(128)),
            vec![String::from("if (!arg0) {")]
        );
    }

    #[test]
    fn test_simplify_parentheses() {
        let lines = vec![String::from("((arg0))")];

        assert_eq!(
            postprocess(lines, HashMap::new(), HashMap::new(), &ProgressBar::new(128)),
            vec![String::from("arg0")]
        );
    }

    #[test]
    fn test_simplify_parentheses_complex() {
        let lines = vec![String::from("if ((cast(((arg0) + 1) / 10))) {")];

        assert_eq!(
            postprocess(lines, HashMap::new(), HashMap::new(), &ProgressBar::new(128)),
            vec![String::from("if (cast((arg0 + 1) / 10)) {")]
        );
    }

    #[test]
    fn test_simplify_parentheses_complex2() {
        let lines = vec![
            String::from("if (((((((((((((((cast(((((((((((arg0 * (((((arg1))))))))))))) + 1)) / 10)))))))))))))))) {"),
        ];

        assert_eq!(
            postprocess(lines, HashMap::new(), HashMap::new(), &ProgressBar::new(128)),
            vec![String::from("if (cast(((arg0 * (arg1)) + 1) / 10)) {")]
        );
    }
}
