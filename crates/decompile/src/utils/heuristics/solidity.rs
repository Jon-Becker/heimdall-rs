use alloy::primitives::U256;
use alloy_dyn_abi::{DynSolType, DynSolValue};
use futures::future::BoxFuture;
use heimdall_common::utils::strings::encode_hex_reduced;
use heimdall_vm::core::vm::State;

use crate::{
    core::{
        analyze::AnalyzerState,
        ir::{BinaryOp, Expr, Statement, StoragePath},
    },
    interfaces::{AnalyzedFunction, StorageFrame},
    utils::constants::VARIABLE_SIZE_CHECK_REGEX,
    Error,
};

pub(crate) fn solidity_heuristic<'a>(
    function: &'a mut AnalyzedFunction,
    state: &'a State,
    analyzer_state: &'a mut AnalyzerState,
) -> BoxFuture<'a, Result<(), Error>> {
    Box::pin(async move {
        let instruction = &state.last_instruction;

        match instruction.opcode {
            // SHA3: retain a point-in-time memory snapshot for storage-path recovery.
            0x20 => {
                function.push_statement(Statement::KeccakSnapshot(Expr::Keccak {
                    offset: Box::new(Expr::from_opcode(&instruction.input_operations[0])),
                    size: Box::new(Expr::from_opcode(&instruction.input_operations[1])),
                    preimage: None,
                }));
            }

            // CALLDATACOPY
            0x37 => {
                let source_offset = Expr::from_opcode(&instruction.input_operations[1]);
                let size = Expr::from_opcode(&instruction.input_operations[2]);
                function.push_statement(Statement::Assign {
                    target: Expr::index(
                        "memory",
                        Expr::from_opcode(&instruction.input_operations[0]),
                    ),
                    value: Expr::slice(
                        "msg.data",
                        source_offset.clone(),
                        Expr::binary(BinaryOp::Add, source_offset, size),
                    ),
                });
            }

            // CODECOPY
            0x39 => {
                let source_offset = Expr::from_opcode(&instruction.input_operations[1]);
                let size = Expr::from_opcode(&instruction.input_operations[2]);
                function.push_statement(Statement::Assign {
                    target: Expr::index(
                        "memory",
                        Expr::from_opcode(&instruction.input_operations[0]),
                    ),
                    value: Expr::slice(
                        "this.code",
                        source_offset.clone(),
                        Expr::binary(BinaryOp::Add, source_offset, size),
                    ),
                });
            }

            // EXTCODECOPY
            0x3C => {
                let source_offset = Expr::from_opcode(&instruction.input_operations[2]);
                let size = Expr::from_opcode(&instruction.input_operations[3]);
                function.push_statement(Statement::Assign {
                    target: Expr::index(
                        "memory",
                        Expr::from_opcode(&instruction.input_operations[1]),
                    ),
                    value: Expr::slice(
                        format!(
                            "address({}).code",
                            Expr::from_opcode(&instruction.input_operations[0]).render()
                        ),
                        source_offset.clone(),
                        Expr::binary(BinaryOp::Add, source_offset, size),
                    ),
                });
            }

            // MSTORE / MSTORE8
            0x52 | 0x53 => {
                let key = instruction.inputs[0];
                let value = instruction.inputs[1];
                let operation = instruction.input_operations[1].to_owned();

                // add the mstore to the function's memory map
                function.memory.insert(key, StorageFrame { operation, value });
                function.push_statement(Statement::Assign {
                    target: Expr::index("memory", Expr::Literal(key)),
                    value: Expr::from_opcode(&instruction.input_operations[1]),
                });
            }

            // SSTORE
            0x55 => {
                function.push_statement(Statement::Assign {
                    target: Expr::StorageAccess(Box::new(StoragePath::Slot {
                        slot: Box::new(Expr::from_opcode(&instruction.input_operations[0])),
                    })),
                    value: Expr::from_opcode(&instruction.input_operations[1]),
                });
            }

            // JUMPI
            0x57 => {
                // this is an if conditional for the children branches
                let conditional_expr = Expr::from_opcode(&instruction.input_operations[1]);
                if matches!(conditional_expr, Expr::Bool(_) | Expr::Literal(_)) {
                    return Ok(())
                }
                let conditional = conditional_expr.render();

                // perform a series of checks to determine if the condition
                // is added by the compiler and can be ignored
                if (conditional.contains("msg.data.length") && conditional.contains("0x04")) ||
                    VARIABLE_SIZE_CHECK_REGEX.is_match(&conditional).unwrap_or(false) ||
                    (conditional.replace('!', "") == "success") ||
                    (conditional == "!msg.value")
                {
                    return Ok(());
                }

                function.push_statement(Statement::If { condition: conditional_expr.clone() });

                // save a copy of the conditional and add it to the conditional map
                analyzer_state.jumped_conditional = Some(conditional_expr.clone());
                analyzer_state.conditional_stack.push(conditional_expr);
            }

            // TSTORE
            0x5d => {
                function.push_statement(Statement::Assign {
                    target: Expr::index(
                        "transient",
                        Expr::from_opcode(&instruction.input_operations[0]),
                    ),
                    value: Expr::from_opcode(&instruction.input_operations[1]),
                });
            }

            // CREATE / CREATE2
            0xf0 | 0xf5 => {
                function.push_statement(Statement::AssemblyAssign {
                    target: "addr".to_string(),
                    function: if instruction.opcode == 0xf5 {
                        "create2".to_string()
                    } else {
                        "create".to_string()
                    },
                    args: instruction.input_operations.iter().map(Expr::from_opcode).collect(),
                });
            }

            // REVERT
            0xfd => {
                // Safely convert U256 to usize
                let offset: usize = instruction.inputs[0].try_into().unwrap_or(0);
                let size: usize = instruction.inputs[1].try_into().unwrap_or(0);
                let revert_data = state.memory.read(offset, size);

                // (1) if revert_data starts with 0x08c379a0, the folling is an error string
                // abiencoded (2) if revert_data starts with 0x4e487b71, the
                // following is a compiler panic (3) if revert_data starts with any
                // other 4byte selector, it is a custom error and should
                //     be resolved and added to the generated ABI
                // (4) if revert_data is empty, it is an empty revert. Ex:
                //       - if (true != false) { revert() };
                //       - require(true != false)
                let reason = if revert_data.starts_with(&[0x08, 0xc3, 0x79, 0xa0]) {
                    let revert_string = match revert_data.get(4..) {
                        Some(hex_data) => match DynSolType::String.abi_decode(hex_data) {
                            Ok(DynSolValue::String(revert)) => revert,
                            _ => "decoding error".to_string(),
                        },
                        None => "decoding error".to_string(),
                    };
                    Some(Expr::StringLiteral(revert_string))
                } else if !revert_data.starts_with(&[0x4e, 0x48, 0x7b, 0x71]) {
                    match revert_data.get(0..4) {
                        Some(selector) => {
                            function.errors.insert(U256::from_be_slice(selector));
                            Some(Expr::Call {
                                callee: format!(
                                    "CustomError_{}",
                                    encode_hex_reduced(U256::from_be_slice(selector))
                                        .replacen("0x", "", 1)
                                ),
                                args: vec![],
                            })
                        }
                        None => None,
                    }
                } else {
                    return Ok(());
                };

                let conditional = match analyzer_state.jumped_conditional.take() {
                    Some(condition) => condition,
                    None => match analyzer_state.conditional_stack.pop() {
                        Some(condition) => condition,
                        None => return Ok(()),
                    },
                };
                // A revert may be observed in the child trace after its opening conditional was
                // emitted. Preserve that condition's expression tree when promoting it to a
                // require, rather than falling back to its rendered representation.
                if let Some(statement) = function
                    .statements
                    .iter_mut()
                    .rev()
                    .find(|statement| matches!(statement, Statement::If { .. }))
                {
                    let condition = match statement {
                        Statement::If { condition } => condition.clone(),
                        _ => unreachable!("matched an if statement"),
                    };
                    *statement = Statement::Require { condition, reason };
                } else {
                    function.push_statement(Statement::Require { condition: conditional, reason });
                }
            }

            // SELFDESTRUCT
            0xff => {
                function.push_statement(Statement::Expression(Expr::Call {
                    callee: "selfdestruct".to_string(),
                    args: vec![Expr::from_opcode(&instruction.input_operations[0])],
                }));
            }

            _ => {}
        };

        Ok(())
    })
}
