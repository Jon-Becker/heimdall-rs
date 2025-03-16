use futures::future::BoxFuture;
use heimdall_vm::{
    core::{
        opcodes::{OpCodeInfo, JUMPI},
        vm::State,
    },
    w_callvalue, w_iszero,
};
use tracing::debug;

use crate::{core::analyze::AnalyzerState, interfaces::AnalyzedFunction, Error};

pub(crate) fn modifier_heuristic<'a>(
    function: &'a mut AnalyzedFunction,
    state: &'a State,
    _: &'a mut AnalyzerState,
) -> BoxFuture<'a, Result<(), Error>> {
    Box::pin(async move {
        let opcode_info = OpCodeInfo::from(state.last_instruction.opcode);

        // if any instruction is non-pure, the function is non-pure
        if function.pure && !opcode_info.is_pure() {
            debug!(
                "instruction {} ({}) indicates a non-pure function",
                state.last_instruction.instruction,
                opcode_info.name()
            );
            function.pure = false;
        }

        // if any instruction is non-view, the function is non-view
        if function.view && !opcode_info.is_view() {
            debug!(
                "instruction {} ({}) indicates a non-view function",
                state.last_instruction.instruction,
                opcode_info.name()
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
    })
}
