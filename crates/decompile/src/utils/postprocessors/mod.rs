use crate::{core::postprocess::PostprocessorState, Error};

// import postprocessors
mod arithmetic;
mod bitwise;
mod memory;
mod storage;
mod transient;
mod variable;

// re-export postprocessors
pub(crate) use arithmetic::arithmetic_postprocessor;
pub(crate) use bitwise::bitwise_mask_postprocessor;
pub(crate) use memory::memory_postprocessor;
pub(crate) use storage::storage_postprocessor;
pub(crate) use transient::transient_postprocessor;
pub(crate) use variable::variable_postprocessor;

/// A heuristic is a function that takes a function and a state and modifies the function based on
/// the state
pub(crate) struct Postprocessor {
    implementation: fn(&mut String, &mut PostprocessorState) -> Result<(), Error>,
}

impl Postprocessor {
    pub(crate) fn new(
        implementation: fn(&mut String, &mut PostprocessorState) -> Result<(), Error>,
    ) -> Self {
        Self { implementation }
    }

    /// Run the postprocessor implementation on the given function
    pub(crate) fn run(
        &self,
        line: &mut String,
        state: &mut PostprocessorState,
    ) -> Result<(), Error> {
        (self.implementation)(line, state)
    }
}
