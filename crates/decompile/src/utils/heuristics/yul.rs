use futures::future::BoxFuture;
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
                function.push_statement(Statement::Expression(Expr::Call {
                    callee: opcode_name(instruction.opcode).to_lowercase(),
                    args: vec![
                        Expr::Literal(key),
                        Expr::from_yul_opcode(&instruction.input_operations[1]),
                    ],
                }));
            }

            // JUMPI
            0x57 => {
                let condition = Expr::from_yul_opcode(&instruction.input_operations[1]);

                function.push_statement(Statement::If { condition: condition.clone() });
                analyzer_state.jumped_conditional = Some(condition.clone());
                analyzer_state.conditional_stack.push(condition);
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
                // rendered Yul line. Stop at the nearest control-flow marker, whether If or
                // already-promoted IfRevertElse, so a second revert on a path does not walk back
                // to an outer guard.
                if let Some(statement) = function
                    .statements
                    .iter_mut()
                    .rev()
                    .find(|statement| {
                        matches!(statement, Statement::If { .. }) ||
                            matches!(statement, Statement::IfRevertElse { .. })
                    })
                {
                    if let Statement::If { condition } = statement {
                        let condition = condition.clone();
                        *statement = Statement::IfRevertElse {
                            condition,
                            offset: Expr::from_yul_opcode(&instruction.input_operations[0]),
                            size: Expr::from_yul_opcode(&instruction.input_operations[1]),
                        };
                    }
                }
            }

            // STATICCALL, CALL, CALLCODE, DELEGATECALL, CREATE, CREATE2
            // CALLDATACOPY, CODECOPY, EXTCODECOPY, RETURNDATACOPY, TSTORE,
            // SSTORE, RETURN, SELFDESTRUCT, LOG0, LOG1, LOG2, LOG3, LOG4
            // we simply want to add the operation to the function's logic
            0x37 | 0x39 | 0x3c | 0x3e | 0x55 | 0x5d | 0xf0 | 0xf1 | 0xf2 | 0xf4 | 0xf5 | 0xfa |
            0xff | 0xA0 | 0xA1 | 0xA2 | 0xA3 | 0xA4 => {
                function.push_statement(Statement::Expression(Expr::Call {
                    callee: opcode_name(instruction.opcode).to_lowercase(),
                    args: instruction.input_operations.iter().map(Expr::from_yul_opcode).collect(),
                }));
            }

            _ => {}
        };

        Ok(())
    })
}
