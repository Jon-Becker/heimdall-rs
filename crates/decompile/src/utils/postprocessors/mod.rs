use std::collections::HashMap;

use heimdall_common::ether::signatures::{ResolvedError, ResolvedLog};

use crate::{core::postprocess::PostprocessorState, interfaces::AnalyzedFunction, Error};

// import postprocessors
mod arithmetic;
mod bitwise;

// re-export postprocessors
pub use arithmetic::arithmetic_postprocessor;
pub use bitwise::bitwise_mask_postprocessor;

/// A heuristic is a function that takes a function and a state and modifies the function based on
/// the state
pub(crate) struct Postprocessor {
    implementation: fn(&mut String, &mut PostprocessorState) -> Result<(), Error>,
}

impl Postprocessor {
    pub fn new(
        implementation: fn(&mut String, &mut PostprocessorState) -> Result<(), Error>,
    ) -> Self {
        Self { implementation }
    }

    /// Run the postprocessor implementation on the given function
    pub fn run(&self, line: &mut String, state: &mut PostprocessorState) -> Result<(), Error> {
        (self.implementation)(line, state)
    }
}
