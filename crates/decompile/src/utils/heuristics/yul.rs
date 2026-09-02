use futures::future::BoxFuture;
use heimdall_common::utils::strings::encode_hex_reduced;
use heimdall_vm::core::{opcodes::opcode_name, vm::State};

use crate::{
    core::{
        analyze::AnalyzerState,
        ir::{Expr, Statement},
    },
    interfaces::{AnalyzedFunction, StorageFrame},
    Error,
};

pub(crate) fn yul_heuristic<'a>(
    function: &'a mut AnalyzedFunction,
    state: &'a State,
    analyzer_state: &'a mut AnalyzerState,
) -> BoxFuture<'a, Result<(), Error>> {
    Box::pin(async move {
        let instruction = &state.last_instruction;

        match instruction.opcode {
            // MSTORE / MSTORE8
            0x52 | 0x53 => {
                let key = instruction.inputs[0];
                let value = instruction.inputs[1];
                let operation = instruction.input_operations[1].clone();

                // add the mstore to the function's memory map
                function.memory.insert(key, StorageFrame { operation, value });
                function.push_raw_statement(format!(
                    "{}({}, {})",
                    opcode_name(instruction.opcode).to_lowercase(),
                    encode_hex_reduced(key),
                    instruction.input_operations[1].yulify()
                ));
            }

            // JUMPI
            0x57 => {
                let conditional = instruction.input_operations[1].yulify();

                function
                    .push_statement(Statement::If { condition: Expr::raw(conditional.clone()) });
                analyzer_state.jumped_conditional = Some(conditional.clone());
                analyzer_state.conditional_stack.push(conditional);
            }

            // REVERT
            0xfd => {
                let revert_data = state.memory.read(
                    instruction.inputs[0].try_into().unwrap_or(0),
                    instruction.inputs[1].try_into().unwrap_or(0),
                );

                // ignore compiler panics, we will reach these due to symbolic execution
                if revert_data.starts_with(&[0x4e, 0x48, 0x7b, 0x71]) {
                    return Ok(());
                }

                // Find the condition that caused this revert and promote it without parsing a
                // rendered Yul line.
                if let Some(statement) = function
                    .statements
                    .iter_mut()
                    .rev()
                    .find(|statement| matches!(statement, Statement::If { .. }))
                {
                    let conditional = match statement {
                        Statement::If { condition } => condition.render(),
                        _ => unreachable!("matched an if statement"),
                    };
                    *statement = Statement::raw(format!(
                        "if {conditional} {{ revert({}, {}); }} else {{",
                        instruction.input_operations[0].yulify(),
                        instruction.input_operations[1].yulify()
                    ));
                }
            }

            // STATICCALL, CALL, CALLCODE, DELEGATECALL, CREATE, CREATE2
            // CALLDATACOPY, CODECOPY, EXTCODECOPY, RETURNDATACOPY, TSTORE,
            // SSTORE, RETURN, SELFDESTRUCT, LOG0, LOG1, LOG2, LOG3, LOG4
            // we simply want to add the operation to the function's logic
            0x37 | 0x39 | 0x3c | 0x3e | 0x55 | 0x5d | 0xf0 | 0xf1 | 0xf2 | 0xf4 | 0xf5 | 0xfa |
            0xff | 0xA0 | 0xA1 | 0xA2 | 0xA3 | 0xA4 => {
                function.push_raw_statement(format!(
                    "{}({})",
                    opcode_name(instruction.opcode).to_lowercase(),
                    instruction
                        .input_operations
                        .iter()
                        .map(|x| x.yulify())
                        .collect::<Vec<String>>()
                        .join(", ")
                ));
            }

            _ => {}
        };

        Ok(())
    })
}
