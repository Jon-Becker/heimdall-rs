use futures::future::BoxFuture;
use heimdall_vm::core::vm::State;

use crate::{core::analyze::AnalyzerState, interfaces::AnalyzedFunction, Error};

// import heuristics
mod arguments;
mod events;
mod extcall;
mod modifiers;
mod solidity;
mod yul;

// re-export heuristics
pub use arguments::argument_heuristic;
pub use events::event_heuristic;
pub use extcall::extcall_heuristic;
pub use modifiers::modifier_heuristic;
pub use solidity::solidity_heuristic;
pub use yul::yul_heuristic;

/// A heuristic is a function that takes a function and a state and modifies the function based on
/// the state
type HeuristicFn = for<'a> fn(
    &'a mut AnalyzedFunction,
    &'a State,
    &'a mut AnalyzerState,
) -> BoxFuture<'a, Result<(), Error>>;

pub(crate) struct Heuristic {
    implementation: HeuristicFn,
}

impl Heuristic {
    pub fn new(implementation: HeuristicFn) -> Self {
        Self { implementation }
    }

    pub async fn run<'a>(
        &self,
        function: &'a mut AnalyzedFunction,
        state: &'a State,
        analyzer_state: &'a mut AnalyzerState,
    ) -> Result<(), Error> {
        (self.implementation)(function, state, analyzer_state).await
    }
}
