use std::{fmt::Display, time::Instant};

use tracing::debug;

use crate::{
    interfaces::AnalyzedFunction,
    utils::postprocessors::{arithmetic_postprocessor, bitwise_mask_postprocessor, Postprocessor},
    Error,
};

use super::analyze::AnalyzerType;

/// State shared between postprocessors
#[derive(Debug, Clone)]
pub(crate) struct PostprocessorState {
    /// Tracks which analyzer type was used to generate the code
    pub analyzer_type: AnalyzerType,
}

impl Default for PostprocessorState {
    fn default() -> Self {
        Self { analyzer_type: AnalyzerType::Solidity }
    }
}

/// The [`PostprocessorOrchestrator`] is responsible for managing the cleanup of
/// generated code from [`AnalyzedFunction`]s passed into [`PostprocessorOrchestrator::postprocess`]
///
/// Depending on [`AnalyzerType`], different postprocessors will be registered and run on the
/// [`AnalyzedFunction`]
pub struct PostprocessOrchestrator {
    /// The type of postprocessor to use. this is taken from the analyzer
    typ: AnalyzerType,
    /// A list of registered postprocessors
    postprocessors: Vec<Postprocessor>,
}

impl PostprocessOrchestrator {
    /// Build a new postprocessor with the given analyzer type
    pub fn new(typ: AnalyzerType) -> Result<Self, Error> {
        let mut orchestrator = Self { typ, postprocessors: Vec::new() };
        orchestrator.register_postprocessors()?;
        Ok(orchestrator)
    }

    /// Register heuristics for the given function and trace
    pub fn register_postprocessors(&mut self) -> Result<(), Error> {
        match self.typ {
            AnalyzerType::Solidity => {
                self.postprocessors.push(Postprocessor::new(bitwise_mask_postprocessor));
                self.postprocessors.push(Postprocessor::new(arithmetic_postprocessor));
            }
            AnalyzerType::Yul => {}
            _ => {}
        };

        Ok(())
    }

    /// Performs postprocessing
    pub fn postprocess(&mut self, function: &mut AnalyzedFunction) -> Result<(), Error> {
        debug!(
            "postprocessing decompiled logic for '{}' with the {} postprocessor",
            function.selector, self.typ
        );
        let start_postprocess_time = Instant::now();

        // get postprocessor state
        let mut state = PostprocessorState { analyzer_type: self.typ.clone() };

        // for each line in the function, run the postprocessors
        function.logic.iter_mut().for_each(|line| {
            self.postprocessors.iter().for_each(|heuristic| {
                heuristic.run(line, &mut state).unwrap();
            });
        });

        debug!(
            "postprocessing for '{}' completed in {:?}",
            function.selector,
            start_postprocess_time.elapsed()
        );

        Ok(())
    }
}
