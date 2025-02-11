use eyre::eyre;
use heimdall_common::utils::strings::{base26_encode, find_balanced_encapsulator};

use crate::{
    core::postprocess::PostprocessorState,
    utils::constants::{MEMORY_VAR_REGEX, STORAGE_ACCESS_REGEX},
    Error,
};

/// Handles converting storage operations to variables. For example:
/// - `transient[0x20]` would become `tstore_a`, and so on.
pub(crate) fn transient_postprocessor(
    line: &mut String,
    state: &mut PostprocessorState,
) -> Result<(), Error> {
    // find a storage access
    let storage_access = match STORAGE_ACCESS_REGEX.find(line).unwrap_or(None) {
        Some(x) => x.as_str(),
        None => "",
    };

    // handle a single storage access
    if let Ok(storage_range) = find_balanced_encapsulator(storage_access, ('[', ']')) {
        let storage_loc = format!(
            "transient[{}]",
            storage_access
                .get(storage_range)
                .ok_or_else(|| eyre!("failed to extract transient location"))?
        );

        let variable_name = match state.storage_map.get(&storage_loc) {
            Some(loc) => loc.to_owned(),
            None => {
                let i = state.storage_map.len() + 1;

                // get the variable name
                if storage_loc.contains("keccak256") {
                    let keccak_range = find_balanced_encapsulator(&storage_loc, ('(', ')'))
                        .map_err(|_| eyre!("failed to extract keccak256 range"))?;

                    let variable_name = format!(
                        "transient_map_{}[{}]",
                        base26_encode(i),
                        storage_loc.get(keccak_range).unwrap_or("?")
                    );

                    // add the variable to the map
                    state.transient_map.insert(storage_loc.clone(), variable_name.clone());
                    variable_name
                } else {
                    let variable_name = format!("tstore_{}", base26_encode(i));

                    // add the variable to the map
                    state.transient_map.insert(storage_loc.clone(), variable_name.clone());
                    variable_name
                }
            }
        };

        // replace the memory location with the new variable name,
        // then recurse until no more memory locations are found
        *line = line.replace(storage_loc.as_str(), &variable_name);
        transient_postprocessor(line, state)?;
    }

    // if there is an assignment to a memory variable, save it to variable_map
    if (line.trim().starts_with("tstore_") || line.trim().starts_with("transient_map_")) &&
        line.contains(" = ")
    {
        let assignment: Vec<String> =
            line.split(" = ").collect::<Vec<&str>>().iter().map(|x| x.to_string()).collect();
        state.variable_map.insert(assignment[0].clone(), assignment[1].replace(';', ""));

        // storage loc can be found by searching for the key where value = assignment[0]
        let mut storage_loc = state
            .transient_map
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

            // replace the storage slot in rhs with a placeholder
            // this will prevent us from pulling bad types from the rhs
            if assignment.len() > 2 {
                let rhs: String = assignment[1].replace(&storage_loc, "_");

                // find vars in lhs or rhs
                for (var, var_type) in state.memory_type_map.iter() {
                    // check for vars in lhs
                    if storage_loc.contains(var) && !var_type.is_empty() {
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
            state.transient_type_map.insert(var_name, mapping_type);
        } else {
            // this is just a normal storage variable, so we can get the type of the rhs from
            // variable type map inheritance
            for (var, var_type) in state.memory_type_map.iter() {
                if line.contains(var) && !var_type.is_empty() {
                    rhs_type = var_type.to_string();
                }
            }

            // add to type map
            state.transient_type_map.insert(var_name, rhs_type);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {}
