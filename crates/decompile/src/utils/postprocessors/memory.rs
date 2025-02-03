use eyre::eyre;
use heimdall_common::{
    constants::TYPE_CAST_REGEX,
    utils::strings::{base26_encode, find_balanced_encapsulator},
};

use crate::{core::postprocess::PostprocessorState, utils::constants::MEMORY_ACCESS_REGEX, Error};

/// Handles converting memory operations to variables. For example:
/// - `memory[0x20]` would become `var_a`, and so on.
pub(crate) fn memory_postprocessor(
    line: &mut String,
    state: &mut PostprocessorState,
) -> Result<(), Error> {
    // find a memory access
    let memory_access = match MEMORY_ACCESS_REGEX.find(line).unwrap_or(None) {
        Some(x) => x.as_str(),
        None => "",
    };

    // handle a single memory access
    if let Ok(memory_range) = find_balanced_encapsulator(memory_access, ('[', ']')) {
        let memory_loc = format!(
            "memory[{}]",
            memory_access
                .get(memory_range)
                .ok_or_else(|| eyre!("failed to extract memory location"))?
        );

        let variable_name = match state.memory_map.get(&memory_loc) {
            Some(loc) => loc.to_owned(),
            None => {
                // add the variable to the map
                let variable_name = format!("var_{}", base26_encode(state.memory_map.len() + 1));
                state.memory_map.insert(memory_loc.clone(), variable_name.clone());
                variable_name
            }
        };

        // replace the memory location with the new variable name,
        // then recurse until no more memory locations are found
        *line = line.replace(memory_loc.as_str(), &variable_name);
        memory_postprocessor(line, state)?;
    }

    // if there is an assignment to a memory variable, save it to variable_map
    if line.trim().starts_with("var_") && line.contains(" = ") {
        let assignment: Vec<String> =
            line.split(" = ").collect::<Vec<&str>>().iter().map(|x| x.to_string()).collect();
        state.variable_map.insert(assignment[0].clone(), assignment[1].replace(';', ""));
        let var_name = assignment[0].clone();

        // infer the type from args and vars in the expression
        for (var, var_type) in state.memory_type_map.iter() {
            if line.contains(var) &&
                !state.memory_type_map.contains_key(&var_name) &&
                !var_type.is_empty()
            {
                *line = format!("{var_type} {line}");
                state.memory_type_map.insert(var_name.to_string(), var_type.to_string());
                break;
            }
        }

        if !state.memory_type_map.contains_key(&var_name) {
            // if the line contains a cast, we can infer the type from the cast
            if let Some(cast_range) = TYPE_CAST_REGEX.find(&assignment[1]).unwrap_or(None) {
                // get the type of the cast
                let cast_type = assignment[1]
                    .get(cast_range.start()..)
                    .expect("impossible case: failed to get cast type after check")
                    .split('(')
                    .collect::<Vec<&str>>()[0];

                *line = format!("{cast_type} {line}");
                state.memory_type_map.insert(var_name, cast_type.to_string());
                return Ok(());
            }

            // we can do some type inference here
            if ["+", "-", "/", "*", "int", ">=", "<="].iter().any(|op| line.contains(op)) ||
                assignment[1].replace(';', "").parse::<i64>().is_ok()
            {
                *line = format!("uint256 {line}");
                state.memory_type_map.insert(var_name, "uint256".to_string());
            } else if ["&", "~", "byte", ">>", "<<"].iter().any(|op| line.contains(op)) {
                *line = format!("bytes32 {line}");
                state.memory_type_map.insert(var_name, "bytes32".to_string());
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {}
