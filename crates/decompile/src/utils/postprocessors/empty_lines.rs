use crate::{core::postprocess::PostprocessorState, interfaces::AnalyzedFunction, Error};

/// Removes empty lines from the function logic.
///
/// This pass should run last, after all other postprocessors have completed,
/// to clean up lines that were cleared by other passes.
pub(crate) fn remove_empty_lines(
    function: &mut AnalyzedFunction,
    _state: &mut PostprocessorState,
) -> Result<(), Error> {
    function.logic.retain(|line| !line.trim().is_empty());
    Ok(())
}
