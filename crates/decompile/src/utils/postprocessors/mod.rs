use crate::{core::postprocess::PostprocessorState, interfaces::AnalyzedFunction, Error};

// import postprocessors
mod arithmetic;
mod bitwise;
mod deadcode;
mod empty_lines;
mod memory;
mod storage;
mod transient;
mod variable;

// re-export postprocessors
pub(crate) use arithmetic::arithmetic_postprocessor;
pub(crate) use bitwise::bitwise_mask_postprocessor;
pub(crate) use deadcode::eliminate_dead_variables;
pub(crate) use empty_lines::remove_empty_lines;
pub(crate) use memory::memory_postprocessor;
pub(crate) use storage::storage_postprocessor;
pub(crate) use transient::transient_postprocessor;
pub(crate) use variable::variable_postprocessor;

/// A line-level postprocessor function signature
type LinePostprocessor = fn(&mut String, &mut PostprocessorState) -> Result<(), Error>;

/// A function-level postprocessor function signature
type FunctionPostprocessor =
    fn(&mut AnalyzedFunction, &mut PostprocessorState) -> Result<(), Error>;

/// A pass operates on the entire function's logic.
///
/// Passes are registered in order and executed sequentially. There are two types:
/// - `LineLevel`: Runs a set of postprocessors on each line
/// - `FunctionLevel`: Runs a transformation on the entire function
pub(crate) enum Pass {
    /// Runs a set of line-level postprocessors on each line sequentially
    LineLevel { postprocessors: Vec<LinePostprocessor> },
    /// Runs a single function-level transformation
    FunctionLevel { transform: FunctionPostprocessor },
}

impl Pass {
    /// Create a new line-level pass with the given postprocessors
    pub(crate) fn line_level(postprocessors: Vec<LinePostprocessor>) -> Self {
        Self::LineLevel { postprocessors }
    }

    /// Create a new function-level pass with the given transformation
    pub(crate) fn function_level(transform: FunctionPostprocessor) -> Self {
        Self::FunctionLevel { transform }
    }

    /// Run the pass on the given function
    pub(crate) fn run(
        &self,
        function: &mut AnalyzedFunction,
        state: &mut PostprocessorState,
    ) -> Result<(), Error> {
        match self {
            Pass::LineLevel { postprocessors } => {
                for line in function.logic.iter_mut() {
                    for postprocessor in postprocessors {
                        postprocessor(line, state)?;
                    }
                }
                Ok(())
            }
            Pass::FunctionLevel { transform } => transform(function, state),
        }
    }
}
