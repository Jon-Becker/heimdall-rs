use eyre::eyre;
use heimdall_common::ether::evm::core::{
    opcodes::{WrappedInput, WrappedOpcode},
    vm::State,
};

use crate::{core::analyze::AnalyzerState, interfaces::AnalyzedFunction, Error};

pub fn yul_heuristic(
    function: &mut AnalyzedFunction,
    state: &State,
    analyzer_state: &mut AnalyzerState,
) -> Result<(), Error> {
    let opcode_name = state
        .last_instruction
        .opcode_details
        .clone()
        .ok_or(Error::Eyre(eyre!("opcode_details is None")))?
        .name;
    let instruction = state.last_instruction.clone();

    Ok(())
}
