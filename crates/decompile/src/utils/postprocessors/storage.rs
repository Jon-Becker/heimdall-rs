use eyre::eyre;
use heimdall_common::utils::strings::{base26_encode, find_balanced_encapsulator};

use crate::{
    core::postprocess::PostprocessorState,
    utils::constants::{MEMORY_VAR_REGEX, STORAGE_ACCESS_REGEX},
    Error,
};

/// Extracts all mapping keys from a storage location containing nested keccak256 calls.
/// For nested mappings like `storage[keccak256(outer . keccak256(inner . slot))]`,
/// this extracts the keys in order: [inner_key, outer_key]
///
/// The storage layout for nested mappings is:
/// - `mapping(K1 => mapping(K2 => V))` at slot S
/// - `m[k1][k2]` is stored at `keccak256(k2 . keccak256(k1 . S))`
///
/// So for `allowances[owner][spender]`:
/// - Inner: keccak256(owner . 0) - the slot for allowances[owner]
/// - Outer: keccak256(spender . inner_result) - the final slot
fn extract_mapping_keys(storage_loc: &str, state: &PostprocessorState) -> Vec<String> {
    let mut keys = Vec::new();

    // First, expand any variables in the storage location to see nested patterns
    // This includes expanding variables that might contain keccak256 results
    let mut expanded = storage_loc.to_string();
    let mut changed = true;
    let mut iterations = 0;
    while changed && iterations < 10 {
        changed = false;
        iterations += 1;
        for (var, value) in state.variable_map.iter() {
            if expanded.contains(var) {
                expanded = expanded.replace(var, value);
                changed = true;
            }
        }
    }

    // Now extract all keccak256 arguments
    extract_keccak_keys_recursive(&expanded, &mut keys);

    keys
}

/// Recursively extracts keys from nested keccak256 calls
fn extract_keccak_keys_recursive(s: &str, keys: &mut Vec<String>) {
    if !s.contains("keccak256") {
        return;
    }

    // Find keccak256( and extract its argument
    if let Some(keccak_start) = s.find("keccak256(") {
        if let Ok(arg_range) = find_balanced_encapsulator(&s[keccak_start..], ('(', ')')) {
            let arg = &s[keccak_start..][arg_range.clone()];

            // Check if this argument contains a nested keccak256
            if arg.contains("keccak256") {
                // Find where the nested keccak256 starts
                if let Some(inner_start) = arg.find("keccak256") {
                    // The key for this level is everything before the inner keccak256
                    // e.g., for "arg0 . keccak256(...)", extract "arg0"
                    let before_inner = arg[..inner_start].trim();

                    // Remove trailing concatenation operators and whitespace
                    let key = before_inner
                        .trim_end_matches(|c: char| c == '.' || c.is_whitespace() || c == '+')
                        .trim();

                    if !key.is_empty() && key != "0" && !key.starts_with("0x0") {
                        keys.push(key.to_string());
                    }

                    // Recursively process the inner keccak256
                    extract_keccak_keys_recursive(&arg[inner_start..], keys);
                }
            } else {
                // This is the innermost keccak256
                // Handle both formats: "memory[offset]" and "memory[offset:size]"
                // Extract the key (excluding the storage slot which is typically 0 or 0x00)
                // Format is usually "key . slot" or "key + slot" or "memory[...]"
                let parts: Vec<&str> = arg.split(|c| c == '.' || c == '+').collect();
                if !parts.is_empty() {
                    let key = parts[0].trim().trim_matches(|c: char| c == '(' || c == ')');
                    // Skip if the key is just a slot number or memory reference
                    if !key.is_empty()
                        && key != "0"
                        && !key.starts_with("0x0")
                        && !key.starts_with("memory[")
                    {
                        keys.push(key.to_string());
                    }
                }
            }
        }
    }
}

/// Counts the nesting depth of keccak256 calls
fn count_keccak_depth(s: &str) -> usize {
    let mut count = 0;
    let mut search = s;
    while let Some(pos) = search.find("keccak256") {
        count += 1;
        search = &search[pos + 9..];
    }
    count
}

/// Checks if a variable contains or is derived from a keccak256 result
fn is_keccak_derived(var: &str, state: &PostprocessorState) -> bool {
    if let Some(value) = state.variable_map.get(var) {
        return value.contains("keccak256");
    }
    false
}

/// Handles converting storage operations to variables. For example:
/// - `storage[0x20]` would become `store_a`, and so on.
/// - `storage[keccak256(key)]` would become `storage_map_a[key]`
/// - `storage[keccak256(k2 . keccak256(k1 . slot))]` would become `storage_map_a[k1][k2]`
pub(crate) fn storage_postprocessor(
    line: &mut String,
    state: &mut PostprocessorState,
) -> Result<(), Error> {
    // Track memory stores for nested mapping detection
    // Pattern: "memory[offset] = value"
    // We use memory_map to track what's stored at each memory location
    if line.contains("memory[") && line.contains(" = ") {
        let parts: Vec<&str> = line.split(" = ").collect();
        if parts.len() >= 2 {
            let lhs = parts[0].trim();
            let rhs = parts[1].trim().trim_end_matches(';');
            if lhs.starts_with("memory[") {
                // Track this memory store
                state.memory_map.insert(lhs.to_string(), rhs.to_string());
            }
        }
    }

    // Track keccak256 variable assignments for nested mapping detection
    // Pattern: "var_X = keccak256(...)" means var_X is a hash result
    if line.contains("keccak256") && line.contains(" = ") && !line.contains("storage") {
        let parts: Vec<&str> = line.split(" = ").collect();
        if parts.len() >= 2 {
            let var_name = parts[0].trim().to_string();
            let value = parts[1].trim().trim_end_matches(';').to_string();
            if value.contains("keccak256") {
                state.variable_map.insert(var_name, value);
            }
        }
    }

    // Track memory stores for nested mapping detection
    // Pattern: "memory[offset] = value" or "var_X = value" after memory postprocessor
    // If value is a keccak256 result variable, track it
    if line.contains(" = ") && !line.contains("storage") && !line.contains("keccak256") {
        let parts: Vec<&str> = line.split(" = ").collect();
        if parts.len() >= 2 {
            let lhs = parts[0].trim();
            let rhs = parts[1].trim().trim_end_matches(';');
            // If rhs is a variable that contains a keccak256 result
            if let Some(keccak_value) = state.variable_map.get(rhs) {
                if keccak_value.contains("keccak256") {
                    // lhs now contains a keccak256 result
                    state.variable_map.insert(lhs.to_string(), keccak_value.clone());
                }
            }
        }
    }

    // find a storage access
    let storage_access = match STORAGE_ACCESS_REGEX.find(line).unwrap_or(None) {
        Some(x) => x.as_str(),
        None => "",
    };

    // handle a single storage access
    if let Ok(storage_range) = find_balanced_encapsulator(storage_access, ('[', ']')) {
        let storage_loc = format!(
            "storage[{}]",
            storage_access
                .get(storage_range)
                .ok_or_else(|| eyre!("failed to extract storage location"))?
        );

        let variable_name = match state.storage_map.get(&storage_loc) {
            Some(loc) => loc.to_owned(),
            None => {
                let i = state.storage_map.len() + 1;

                // get the variable name
                if storage_loc.contains("keccak256") {
                    // Check for nested mappings
                    // For nested mappings like allowances[owner][spender], the storage slot
                    // is computed as: keccak256(spender . keccak256(owner . slot))
                    // In the raw logic, we see:
                    //   memory[0x20] = keccak256(memory[0:0x40])  <- first hash
                    //   storage[keccak256(memory[0:0x40])] = ...  <- second hash
                    // The memory_map tracks which memory locations contain keccak256 results

                    // Check if this is a nested mapping by looking at what's stored in memory
                    // For nested mappings like allowances[owner][spender]:
                    // - memory[0x20] will contain a keccak256 result (the inner hash)
                    // - memory[0] will contain the outer key
                    let mut is_nested = false;
                    let mut inner_key = String::new();
                    let mut outer_key = String::new();

                    // The memory postprocessor runs before storage, so memory[0x20] has been
                    // converted to a variable name. We need to look in memory_map to find the
                    // variable name, then look in variable_map to see if it contains keccak256.
                    //
                    // memory_map: {"memory[0x20]": "var_b", "memory[0]": "var_a"}
                    // variable_map: {"var_b": "keccak256(var_c)", ...}

                    // Check if the variable at memory[0x20] contains a keccak256 result
                    if let Some(var_name) = state.memory_map.get("memory[0x20]") {
                        if let Some(var_value) = state.variable_map.get(var_name) {
                            if var_value.contains("keccak256") {
                                // The variable at memory[0] (var_a) has been assigned multiple times
                                // for nested mappings. The history looks like:
                                //   var_a = msg.sender (first key)
                                //   var_a = arg0       (second key, overwrites first)
                                // We need both keys to reconstruct the nested mapping.
                                if let Some(mem0_var) = state.memory_map.get("memory[0]") {
                                    if let Some(history) = state.variable_history.get(mem0_var) {
                                        if history.len() >= 2 {
                                            is_nested = true;
                                            // First assignment = inner key (e.g., msg.sender)
                                            inner_key = history[0].clone();
                                            // Last assignment = outer key (e.g., arg0)
                                            outer_key = history[history.len() - 1].clone();

                                            // Clean up address() wrapping for cleaner output
                                            if inner_key.starts_with("address(") && inner_key.ends_with(")") {
                                                inner_key = inner_key[8..inner_key.len()-1].to_string();
                                            }
                                            if outer_key.starts_with("address(") && outer_key.ends_with(")") {
                                                outer_key = outer_key[8..outer_key.len()-1].to_string();
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Also check expanded form for nested patterns from variable_map
                    let mut expanded_loc = storage_loc.clone();
                    let mut changed = true;
                    let mut iterations = 0;
                    while changed && iterations < 10 {
                        changed = false;
                        iterations += 1;
                        for (var, value) in state.variable_map.iter() {
                            if expanded_loc.contains(var) {
                                expanded_loc = expanded_loc.replace(var, value);
                                changed = true;
                            }
                        }
                    }

                    let depth = count_keccak_depth(&storage_loc);
                    let expanded_depth = count_keccak_depth(&expanded_loc);

                    let keys = extract_mapping_keys(&storage_loc, state);
                    let mut expanded_keys = if expanded_depth > depth {
                        extract_mapping_keys(&expanded_loc, state)
                    } else {
                        keys.clone()
                    };

                    // If we detected a nested mapping but didn't extract keys, add them
                    if is_nested && expanded_keys.len() < 2 {
                        expanded_keys.clear();
                        if !inner_key.is_empty() {
                            expanded_keys.push(inner_key);
                        }
                        if !outer_key.is_empty() {
                            expanded_keys.push(outer_key);
                        }
                    }

                    let variable_name = if expanded_keys.len() > 1 {
                        // Nested mapping: generate multi-level indexing
                        // Keys are in order [inner_key, outer_key, ...]
                        let indices = expanded_keys
                            .iter()
                            .map(|k| format!("[{}]", k))
                            .collect::<String>();
                        format!("storage_map_{}{}", base26_encode(i), indices)
                    } else if !expanded_keys.is_empty() {
                        // Single mapping
                        format!("storage_map_{}[{}]", base26_encode(i), expanded_keys[0])
                    } else {
                        // Fallback to original behavior
                        let keccak_range = find_balanced_encapsulator(&storage_loc, ('(', ')'))
                            .map_err(|_| eyre!("failed to extract keccak256 range"))?;
                        format!(
                            "storage_map_{}[{}]",
                            base26_encode(i),
                            storage_loc.get(keccak_range).unwrap_or("?")
                        )
                    };

                    // add the variable to the map
                    state.storage_map.insert(storage_loc.clone(), variable_name.clone());
                    variable_name
                } else {
                    let variable_name = format!("store_{}", base26_encode(i));

                    // add the variable to the map
                    state.storage_map.insert(storage_loc.clone(), variable_name.clone());
                    variable_name
                }
            }
        };

        // replace the memory location with the new variable name,
        // then recurse until no more memory locations are found
        *line = line.replace(storage_loc.as_str(), &variable_name);
        storage_postprocessor(line, state)?;
    }

    // if there is an assignment to a memory variable, save it to variable_map
    if (line.trim().starts_with("store_") || line.trim().starts_with("storage_map_")) &&
        line.contains(" = ")
    {
        let assignment: Vec<String> =
            line.split(" = ").collect::<Vec<&str>>().iter().map(|x| x.to_string()).collect();
        state.variable_map.insert(assignment[0].clone(), assignment[1].replace(';', ""));

        // storage loc can be found by searching for the key where value = assignment[0]
        let mut storage_loc = state
            .storage_map
            .iter()
            .find(|(_, value)| value == &&assignment[0])
            .map(|(key, _)| key.clone())
            .unwrap_or(String::new());
        let mut var_name = assignment[0].clone();

        // if the storage_slot is a variable, replace it with the value
        // ex: storage[var_b] => storage[keccak256(var_a)]
        // helps with type inference
        if MEMORY_VAR_REGEX.is_match(&storage_loc).unwrap_or(false) {
            for (var, value) in state.variable_map.iter() {
                if storage_loc.contains(var) {
                    *line = line.replace(var, value);
                    storage_loc = storage_loc.replace(var, value);
                }
            }
        }

        // default type is bytes32, since it technically can hold any type
        let mut lhs_type = "bytes32".to_string();
        let mut rhs_type = "bytes32".to_string();

        // if the storage slot contains a keccak256 call, this is a mapping and we will need to pull
        // types from both the lhs and rhs
        if storage_loc.contains("keccak256") {
            var_name = var_name.split('[').collect::<Vec<&str>>()[0].to_string();

            // Extract keys from the variable name itself (e.g., storage_map_a[msg.sender][arg0])
            // This is more reliable than parsing the storage location for nested mappings
            let var_with_keys = assignment[0].clone();
            let mut keys: Vec<String> = Vec::new();
            let mut remaining = var_with_keys.as_str();

            // Skip the base name (storage_map_a) and extract each [key]
            if let Some(first_bracket) = remaining.find('[') {
                remaining = &remaining[first_bracket..];
                while remaining.starts_with('[') {
                    if let Ok(range) = find_balanced_encapsulator(remaining, ('[', ']')) {
                        if let Some(key) = remaining.get(range.clone()) {
                            keys.push(key.to_string());
                        }
                        remaining = &remaining[range.end + 1..];
                    } else {
                        break;
                    }
                }
            }

            // Fallback to extract_mapping_keys if no keys found from variable name
            if keys.is_empty() {
                keys = extract_mapping_keys(&storage_loc, state);
            }

            let mut key_types: Vec<String> = vec!["bytes32".to_string(); keys.len().max(1)];

            // replace the storage slot in rhs with a placeholder
            // this will prevent us from pulling bad types from the rhs
            if assignment.len() >= 2 {
                let rhs: String = assignment[1].replace(&storage_loc, "_");

                // find types for each key from memory_type_map
                for (i, key) in keys.iter().enumerate() {
                    for (var, var_type) in state.memory_type_map.iter() {
                        if key.contains(var) && !var_type.is_empty() {
                            if i < key_types.len() {
                                key_types[i] = var_type.to_string();
                            }
                            break;
                        }
                    }
                }

                // find type for rhs (the value being stored)
                for (var, var_type) in state.memory_type_map.iter() {
                    if rhs.contains(var) && !var_type.is_empty() {
                        rhs_type = var_type.to_string();
                        break;
                    }
                }
                if rhs_type == "bytes32" &&
                    ["+", "-", "/", "*"].iter().any(|op| rhs.contains(op))
                {
                    rhs_type = "uint256".to_string();
                }
            }

            // Build the mapping type, with nesting for multiple keys
            // For keys [k1, k2], generate: mapping(k1_type => mapping(k2_type => value_type))
            let mapping_type = if key_types.len() > 1 {
                let mut nested = rhs_type.clone();
                for key_type in key_types.iter().rev() {
                    nested = format!("mapping({key_type} => {nested})");
                }
                nested
            } else if !key_types.is_empty() {
                format!("mapping({} => {})", key_types[0], rhs_type)
            } else {
                format!("mapping({lhs_type} => {rhs_type})")
            };

            state.storage_type_map.insert(var_name, mapping_type);
        } else {
            // this is just a normal storage variable, so we can get the type of the rhs from
            // variable type map inheritance
            for (var, var_type) in state.memory_type_map.iter() {
                if line.contains(var) && !var_type.is_empty() {
                    rhs_type = var_type.to_string();
                }
            }

            // add to type map
            state.storage_type_map.insert(var_name, rhs_type);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {}
