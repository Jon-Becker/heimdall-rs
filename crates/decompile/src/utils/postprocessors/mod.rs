use crate::{
    core::{ir::Statement, postprocess::PostprocessorState},
    interfaces::AnalyzedFunction,
    Error,
};

// import postprocessors
mod arithmetic;
mod bitwise;
mod deadcode;
mod inline;
mod memory;
mod storage;
mod storage_inference;
mod transient;
mod types;
mod variable;

// re-export postprocessors
pub(crate) use arithmetic::arithmetic_postprocessor;
pub(crate) use bitwise::bitwise_mask_postprocessor;
pub(crate) use deadcode::eliminate_dead_variables;
pub(crate) use inline::inline_single_use_variables;
pub(crate) use memory::memory_postprocessor;
pub(crate) use storage::storage_postprocessor;
pub(crate) use storage_inference::storage_inference_postprocessor;
pub(crate) use transient::transient_postprocessor;
pub(crate) use types::{normalize_typed_returns, type_cleanup_postprocessor};
pub(crate) use variable::variable_postprocessor;

/// A structured IR postprocessor function signature.
pub(crate) type IrPostprocessor = fn(&mut Statement, &mut PostprocessorState) -> Result<(), Error>;

/// A function-wide structured IR postprocessor function signature.
pub(crate) type IrFunctionPostprocessor =
    fn(&mut AnalyzedFunction, &mut PostprocessorState) -> Result<(), Error>;
