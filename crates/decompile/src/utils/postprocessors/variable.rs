use crate::{core::postprocess::PostprocessorState, Error};

/// Handles simplifying expressions by replacing equivalent expressions with variables.
pub fn variable_postprocessor(
    line: &mut String,
    state: &mut PostprocessorState,
) -> Result<(), Error> {
    state.variable_map.iter().for_each(|(variable, expr)| {
        // skip exprs that are already variables
        if !expr.contains(' ') &&
            ["store", "tstore", "transient", "storage", "var"]
                .iter()
                .any(|x| expr.starts_with(x))
        {
            return;
        }

        if line.contains(expr) && !line.trim().contains(variable) {
            *line = line.replace(expr, variable);
        }
    });

    state.storage_map.iter().for_each(|(variable, expr)| {
        // skip exprs that are already variables
        if !expr.contains(' ') &&
            ["store", "tstore", "transient", "storage", "var"]
                .iter()
                .any(|x| expr.starts_with(x))
        {
            return;
        }

        if line.contains(expr) && !line.trim().contains(variable) {
            *line = line.replace(expr, variable);
        }
    });

    state.transient_map.iter().for_each(|(variable, expr)| {
        // skip exprs that are already variables
        if !expr.contains(' ') &&
            ["store", "tstore", "transient", "storage", "var"]
                .iter()
                .any(|x| expr.starts_with(x))
        {
            return;
        }

        if line.contains(expr) && !line.trim().contains(variable) {
            *line = line.replace(expr, variable);
        }
    });

    Ok(())
}

#[cfg(test)]
mod tests {}
