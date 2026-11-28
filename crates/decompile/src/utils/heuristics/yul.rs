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
    _analyzer_state: &'a mut AnalyzerState,
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

            // JUMPI is reconstructed from trace children by the analyzer. Keep all reverts,
            // including compiler panics, at their original position after the error-data writes.

            // STATICCALL, CALL, CALLCODE, DELEGATECALL, CREATE, CREATE2
            // CALLDATACOPY, CODECOPY, EXTCODECOPY, RETURNDATACOPY, TSTORE,
            // SSTORE, RETURN, REVERT, STOP, INVALID, SELFDESTRUCT, LOG0, LOG1, LOG2, LOG3, LOG4
            // we simply want to add the operation to the function's logic
            0x00 | 0xfd | 0xfe | 0x37 | 0x39 | 0x3c | 0x3e | 0x55 | 0x5d | 0xf0 | 0xf1 | 0xf2 |
            0xf4 | 0xf5 | 0xfa | 0xff | 0xA0 | 0xA1 | 0xA2 | 0xA3 | 0xA4 => {
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
