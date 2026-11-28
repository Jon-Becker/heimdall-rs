use alloy::primitives::U256;
use futures::future::BoxFuture;
use heimdall_common::utils::hex::ToLowerHex;
use heimdall_decoder::{decode, DecodeArgsBuilder};
use heimdall_vm::{
    core::{opcodes::opcode_name, vm::State},
    w_gas, w_push0,
};
use tracing::trace;

use crate::{
    core::{
        analyze::AnalyzerState,
        ir::{BinaryOp, Expr, Statement},
    },
    interfaces::AnalyzedFunction,
    utils::precompile::decode_precompile,
    Error,
};

pub(crate) fn extcall_heuristic<'a>(
    function: &'a mut AnalyzedFunction,
    state: &'a State,
    analyzer_state: &'a mut AnalyzerState,
) -> BoxFuture<'a, Result<(), Error>> {
    Box::pin(async move {
        let instruction = &state.last_instruction;

        match instruction.opcode {
            // CALL / CALLCODE
            0xf1 | 0xf2 => {
                let memory =
                    function.get_memory_range(instruction.inputs[3], instruction.inputs[4]);
                let extcalldata =
                    memory.iter().map(|x| x.value.to_lower_hex()).collect::<Vec<String>>().join("");
                let address = Expr::from_opcode(&instruction.input_operations[1]);
                let value = Expr::from_opcode(&instruction.input_operations[2]);

                // Empty concrete memory is not evidence of an empty call: symbolic calldata
                // lengths evaluate to zero when executing with only the function selector.
                let size = Expr::from_opcode(&instruction.input_operations[4]);
                if matches!(size, Expr::Literal(size) if size.is_zero()) {
                    trace!(
                        "instruction {} ({}) indicates a value transfer",
                        instruction.instruction,
                        opcode_name(instruction.opcode)
                    );
                    function.push_statement(Statement::ExternalCall {
                        address,
                        function: "transfer".to_string(),
                        args: vec![value],
                        gas: None,
                        value: None,
                        comment: None,
                    });
                    return Ok(());
                }

                let decoded = decode(
                    DecodeArgsBuilder::new()
                        .target(extcalldata.clone())
                        .raw(true)
                        .skip_resolving(analyzer_state.skip_resolving)
                        .build()
                        .expect("Failed to build DecodeArgs"),
                )
                .await
                .ok();

                if let Some(precompile) = decode_precompile(
                    instruction.inputs[1],
                    &memory,
                    &instruction.input_operations[5],
                ) {
                    function.push_statement(precompile);
                    return Ok(());
                }

                let (name, args) = if let Some(decoded) = decoded {
                    let start_slot = instruction.inputs[3] + U256::from(4);
                    (
                        decoded.decoded.name,
                        decoded
                            .decoded
                            .inputs
                            .iter()
                            .enumerate()
                            .map(|(i, _)| {
                                Expr::index(
                                    "memory",
                                    Expr::Literal(start_slot + U256::from(i * 32)),
                                )
                            })
                            .collect(),
                    )
                } else {
                    let start = Expr::from_opcode(&instruction.input_operations[3]);
                    let name = extcalldata
                        .get(2..10)
                        .map(|selector| format!("Unresolved_{selector}"))
                        .unwrap_or_else(|| opcode_name(instruction.opcode).to_lowercase());
                    (
                        name,
                        vec![Expr::slice(
                            "memory",
                            start.clone(),
                            Expr::binary(BinaryOp::Add, start, size),
                        )],
                    )
                };

                function.push_statement(Statement::ExternalCall {
                    address,
                    function: name,
                    args,
                    gas: (instruction.input_operations[0] != w_gas!())
                        .then(|| Expr::from_opcode(&instruction.input_operations[0])),
                    value: (instruction.input_operations[2] != w_push0!()).then_some(value),
                    comment: Some(opcode_name(instruction.opcode).to_lowercase()),
                });
            }

            // STATICCALL / DELEGATECALL
            0xfa | 0xf4 => {
                let memory =
                    function.get_memory_range(instruction.inputs[2], instruction.inputs[3]);
                let extcalldata = memory
                    .iter()
                    .map(|x| x.value.to_lower_hex().trim_start_matches("0x").to_owned())
                    .collect::<Vec<String>>()
                    .join("");
                let address = Expr::from_opcode(&instruction.input_operations[1]);

                let decoded = decode(
                    DecodeArgsBuilder::new()
                        .target(extcalldata.clone())
                        .raw(true)
                        .skip_resolving(analyzer_state.skip_resolving)
                        .build()
                        .expect("Failed to build DecodeArgs"),
                )
                .await
                .ok();

                if let Some(precompile) = decode_precompile(
                    instruction.inputs[1],
                    &memory,
                    &instruction.input_operations[4],
                ) {
                    function.push_statement(precompile);
                    return Ok(());
                }

                let (name, args) = if let Some(decoded) = decoded {
                    let start_slot = instruction.inputs[2] + U256::from(4);
                    (
                        decoded.decoded.name,
                        decoded
                            .decoded
                            .inputs
                            .iter()
                            .enumerate()
                            .map(|(i, _)| {
                                Expr::index(
                                    "memory",
                                    Expr::Literal(start_slot + U256::from(i * 32)),
                                )
                            })
                            .collect(),
                    )
                } else {
                    let start = Expr::from_opcode(&instruction.input_operations[2]);
                    let size = Expr::from_opcode(&instruction.input_operations[3]);
                    (
                        format!("Unresolved_{}", extcalldata.get(2..10).unwrap_or("")),
                        vec![Expr::slice(
                            "memory",
                            start.clone(),
                            Expr::binary(BinaryOp::Add, start, size),
                        )],
                    )
                };

                function.push_statement(Statement::ExternalCall {
                    address,
                    function: name,
                    args,
                    gas: (instruction.input_operations[0] != w_gas!())
                        .then(|| Expr::from_opcode(&instruction.input_operations[0])),
                    value: None,
                    comment: Some(opcode_name(instruction.opcode).to_lowercase()),
                });
            }

            _ => {}
        };

        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::analyze::AnalyzerType;
    use heimdall_vm::core::{
        memory::Memory,
        opcodes::{self, WrappedOpcode},
        stack::Stack,
        storage::Storage,
        vm::Instruction,
    };

    fn literal(value: u64) -> WrappedOpcode {
        WrappedOpcode::new(opcodes::PUSH32, vec![U256::from(value).into()])
    }

    async fn analyze_call(gas: u64, size: u64, symbolic_size: WrappedOpcode) -> Statement {
        let inputs = [gas, 0x1234, 7, 128, size, 0, 0];
        let mut input_operations: Vec<_> = inputs.iter().map(|value| literal(*value)).collect();
        input_operations[4] = symbolic_size;
        let state = State {
            last_instruction: Instruction {
                instruction: 1,
                opcode: opcodes::CALL,
                inputs: inputs.iter().map(|value| U256::from(*value)).collect(),
                outputs: Vec::new(),
                input_operations,
                output_operations: Vec::new(),
            },
            gas_used: 0,
            gas_remaining: 0,
            stack: Stack::new(),
            memory: Memory::new(),
            storage: Storage::new(),
            events: Vec::new(),
        };
        let mut function = AnalyzedFunction::new("12345678", false);
        let mut analyzer_state =
            AnalyzerState { analyzer_type: AnalyzerType::Solidity, skip_resolving: true };
        extcall_heuristic(&mut function, &state, &mut analyzer_state)
            .await
            .expect("call analysis should succeed");
        assert_eq!(function.statements.len(), 1);
        function.statements.pop().expect("external call statement")
    }

    #[tokio::test]
    async fn dynamic_call_data_is_preserved_when_concrete_size_is_zero() {
        for gas in [2300, 100_000] {
            let symbolic_size =
                WrappedOpcode::new(opcodes::CALLDATALOAD, vec![U256::from(36).into()]);
            let statement = analyze_call(gas, 0, symbolic_size).await;
            let Statement::ExternalCall { function, args, value, .. } = statement else {
                panic!("expected external call");
            };
            assert_eq!(function, "call");
            assert_eq!(value, Some(Expr::Literal(U256::from(7))));
            assert_eq!(
                args,
                vec![Expr::slice(
                    "memory",
                    Expr::Literal(U256::from(128)),
                    Expr::binary(
                        BinaryOp::Add,
                        Expr::Literal(U256::from(128)),
                        Expr::identifier("arg1"),
                    ),
                )]
            );
        }
    }

    #[tokio::test]
    async fn stipend_does_not_discard_nonempty_call_data() {
        let statement = analyze_call(2300, 32, literal(32)).await;
        assert!(
            matches!(statement, Statement::ExternalCall { function, .. } if function == "call")
        );
    }

    #[tokio::test]
    async fn literal_empty_call_data_retains_value_transfer() {
        let statement = analyze_call(2300, 0, literal(0)).await;
        assert!(matches!(
            statement,
            Statement::ExternalCall { function, args, .. }
                if function == "transfer" && args == vec![Expr::Literal(U256::from(7))]
        ));
    }
}
