use eyre::eyre;
use heimdall_common::ether::evm::core::{
    vm::State,
};

use crate::{core::analyze::AnalyzerState, interfaces::AnalyzedFunction, Error};

pub fn yul_heuristic(
    _function: &mut AnalyzedFunction,
    state: &State,
    _analyzer_state: &mut AnalyzerState,
) -> Result<(), Error> {
    let _opcode_name = state
        .last_instruction
        .opcode_details
        .clone()
        .ok_or(Error::Eyre(eyre!("opcode_details is None")))?
        .name;
    let _instruction = state.last_instruction.clone();

    Ok(())
}
