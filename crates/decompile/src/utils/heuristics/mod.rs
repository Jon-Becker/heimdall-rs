use heimdall_vm::core::vm::State;

use crate::{core::analyze::AnalyzerState, interfaces::AnalyzedFunction, Error};

// import heuristics
mod arguments;
mod events;
mod modifiers;
mod solidity;
mod yul;

// re-export heuristics
pub use arguments::argument_heuristic;
pub use events::event_heuristic;
pub use modifiers::modifier_heuristic;
pub use solidity::solidity_heuristic;
pub use yul::yul_heuristic;

/// A heuristic is a function that takes a function and a state and modifies the function based on
/// the state
pub(crate) struct Heuristic {
    implementation: fn(&mut AnalyzedFunction, &State, &mut AnalyzerState) -> Result<(), Error>,
}

impl Heuristic {
    pub fn new(
        implementation: fn(&mut AnalyzedFunction, &State, &mut AnalyzerState) -> Result<(), Error>,
    ) -> Self {
        Self { implementation }
    }

    /// Run the heuristic implementation on the given state
    pub fn run(
        &self,
        function: &mut AnalyzedFunction,
        state: &State,
        analyzer_state: &mut AnalyzerState,
    ) -> Result<(), Error> {
        (self.implementation)(function, state, analyzer_state)
    }
}
