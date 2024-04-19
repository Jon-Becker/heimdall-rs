use std::collections::HashMap;

use heimdall_common::ether::signatures::{ResolvedError, ResolvedLog};

use crate::{interfaces::AnalyzedFunction, Error};

// import postprocessors

// re-export postprocessors

/// A heuristic is a function that takes a function and a state and modifies the function based on
/// the state
pub(crate) struct Postprocessor {
    implementation: fn(
        &mut AnalyzedFunction,
        &HashMap<String, ResolvedError>,
        &HashMap<String, ResolvedLog>,
    ) -> Result<(), Error>,
}

impl Postprocessor {
    pub fn new(
        implementation: fn(
            &mut AnalyzedFunction,
            &HashMap<String, ResolvedError>,
            &HashMap<String, ResolvedLog>,
        ) -> Result<(), Error>,
    ) -> Self {
        Self { implementation }
    }

    /// Run the postprocessor implementation on the given function
    pub fn run(
        &self,
        function: &mut AnalyzedFunction,
        all_resolved_errors: &HashMap<String, ResolvedError>,
        all_resolved_logs: &HashMap<String, ResolvedLog>,
    ) -> Result<(), Error> {
        (self.implementation)(function, all_resolved_errors, all_resolved_logs)
    }
}
