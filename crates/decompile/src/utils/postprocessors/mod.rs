use crate::{
    core::{ir::Statement, postprocess::PostprocessorState},
    interfaces::AnalyzedFunction,
    Error,
};

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

/// A structured IR postprocessor function signature.
pub(crate) type IrPostprocessor = fn(&mut Statement, &mut PostprocessorState) -> Result<(), Error>;

/// A function-level postprocessor function signature
type FunctionPostprocessor =
    fn(&mut AnalyzedFunction, &mut PostprocessorState) -> Result<(), Error>;

/// A pass operates on the entire function's logic.
///
/// Function-level passes are registered in order and executed after structured IR passes.
pub(crate) enum Pass {
    /// Runs a single function-level transformation.
    FunctionLevel { transform: FunctionPostprocessor },
}

impl Pass {
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
            Pass::FunctionLevel { transform } => transform(function, state),
        }
    }
}
