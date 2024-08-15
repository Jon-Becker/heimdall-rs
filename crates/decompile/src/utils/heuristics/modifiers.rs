use heimdall_vm::{
    core::{
        opcodes::{OpCodeInfo, JUMPI},
        vm::State,
    },
    w_callvalue, w_iszero,
};
use tracing::debug;

use crate::{core::analyze::AnalyzerState, interfaces::AnalyzedFunction, Error};

use lazy_static::lazy_static;

lazy_static! {
    /// A list of opcodes that are considered non-pure (state accessing)
    pub static ref NON_PURE_OPCODES: Vec<u8> = vec![
        0x31, 0x32, 0x33, 0x3a, 0x3b, 0x3c, 0x40, 0x41, 0x42,
        0x43, 0x44, 0x45, 0x46, 0x47, 0x48, 0x54, 0x55, 0xf0,
        0xf1, 0xf2, 0xf4, 0xf5, 0xfa, 0xff
    ];
    /// A list of opcodes that are considered non-view (state modifying)
    pub static ref NON_VIEW_OPCODES: Vec<u8> = vec![
        0x55, 0xf0, 0xf1, 0xf2, 0xf4, 0xf5, 0xfa, 0xff
    ];
}

pub fn modifier_heuristic(
    function: &mut AnalyzedFunction,
    state: &State,
    _: &mut AnalyzerState,
) -> Result<(), Error> {
    let opcode_name = OpCodeInfo::from(state.last_instruction.opcode).name();

    // if any instruction is non-pure, the function is non-pure
    if function.pure && NON_PURE_OPCODES.contains(&state.last_instruction.opcode) {
        debug!(
            "instruction {} ({}) indicates a non-pure function",
            state.last_instruction.instruction, opcode_name
        );
        function.pure = false;
    }

    // if any instruction is non-view, the function is non-view
    if function.view && NON_VIEW_OPCODES.contains(&state.last_instruction.opcode) {
        debug!(
            "instruction {} ({}) indicates a non-view function",
            state.last_instruction.instruction, opcode_name
        );
        function.view = false;
    }

    // if the instruction is a JUMPI with non-zero CALLVALUE requirement, the function is
    // non-payable exactly: ISZERO(CALLVALUE())
    if function.payable &&
        state.last_instruction.opcode == JUMPI &&
        state.last_instruction.input_operations[1] == w_iszero!(w_callvalue!())
    {
        debug!(
            "conditional at instruction {} indicates a non-payable function",
            state.last_instruction.instruction
        );
        function.payable = false;
    }

    Ok(())
}
