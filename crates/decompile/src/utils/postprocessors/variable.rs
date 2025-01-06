use crate::{core::postprocess::PostprocessorState, Error};

/// Handles simplifying expressions by replacing equivalent expressions with variables.
pub(crate) fn variable_postprocessor(
    line: &mut String,
    state: &mut PostprocessorState,
) -> Result<(), Error> {
    state
        .variable_map
        .iter()
        .chain(state.storage_map.iter())
        .chain(state.transient_map.iter())
        .for_each(|(variable, expr)| {
            // skip exprs that are already variables
            if !expr.contains(' ') &&
                ["store", "tstore", "transient", "storage", "var"]
                    .iter()
                    .any(|x| expr.starts_with(x))
            {
                return;
            }

            // little short circuit type beat
            if line.contains(expr) && !line.trim().contains(variable) {
                // split line by space,
                let mut line_parts = line.split_whitespace().collect::<Vec<&str>>();

                // iter over line parts, replace only whole words that match expr
                for part in line_parts.iter_mut() {
                    if *part == expr {
                        *part = variable;
                    }
                }
                *line = line_parts.join(" ");
            }
        });

    Ok(())
}

#[cfg(test)]
mod tests {}
