use alloy::primitives::U256;
use futures::future::BoxFuture;
use heimdall_common::utils::strings::encode_hex;
use heimdall_decoder::{decode, DecodeArgsBuilder};
use heimdall_vm::{
    core::{
        opcodes::opcode_name,
        vm::{Instruction, State},
    },
    w_gas, w_push0,
};

use crate::{
    core::{
        analyze::AnalyzerState,
        ir::{BinaryOp, Expr, Statement},
    },
    interfaces::AnalyzedFunction,
    Error,
};

/// CALL and CALLCODE have a value operand; DELEGATECALL and STATICCALL do not.
struct CallOperands<'a> {
    instruction: &'a Instruction,
    input_offset: usize,
    value: Option<usize>,
}

impl<'a> CallOperands<'a> {
    fn new(instruction: &'a Instruction) -> Option<Self> {
        let (input_offset, value) = match instruction.opcode {
            0xf1 | 0xf2 => (3, Some(2)),
            0xfa | 0xf4 => (2, None),
            _ => return None,
        };
        Some(Self { instruction, input_offset, value })
    }
}

pub(crate) fn extcall_heuristic<'a>(
    function: &'a mut AnalyzedFunction,
    state: &'a State,
    analyzer_state: &'a mut AnalyzerState,
) -> BoxFuture<'a, Result<(), Error>> {
    Box::pin(async move {
        let Some(call) = CallOperands::new(&state.last_instruction) else { return Ok(()) };
        let instruction = call.instruction;
        let offset = call.input_offset;
        let input_range =
            analyzer_state.memory_provenance.argument_range(state, offset, offset + 1);
        // Placeholder values for symbolic inputs are not evidence for ABI signature matching.
        // A known selector can still label an unresolved call when its arguments are unknown.
        let calldata = input_range.and_then(|(start, size)| {
            analyzer_state
                .memory_provenance
                .contains(start, size)
                .then(|| state.memory.try_read(start, size))
                .flatten()
        });
        let selector = input_range.and_then(|(start, size)| {
            (size >= 4 && analyzer_state.memory_provenance.contains(start, 4))
                .then(|| state.memory.try_read(start, 4))
                .flatten()
                .map(|bytes| encode_hex(&bytes))
        });
        let address = Expr::from_opcode(&instruction.input_operations[1]);
        let value = call.value.map(|index| Expr::from_opcode(&instruction.input_operations[index]));

        let known_precompile = instruction
            .input_operations
            .get(1)
            .is_some_and(crate::core::memory::MemoryProvenance::is_constant) &&
            instruction.inputs[1] >= U256::from(1) &&
            instruction.inputs[1] <= U256::from(9);
        let decoded = if let Some(calldata) = calldata.filter(|_| !known_precompile) {
            decode(
                DecodeArgsBuilder::new()
                    .target(encode_hex(&calldata))
                    .raw(true)
                    .skip_resolving(analyzer_state.skip_resolving)
                    .build()
                    .expect("Failed to build DecodeArgs"),
            )
            .await
            .ok()
        } else {
            None
        };

        let (name, args) = if let Some(decoded) = decoded {
            let start_slot = instruction.inputs[offset] + U256::from(4);
            (
                decoded.decoded.name,
                decoded
                    .decoded
                    .inputs
                    .iter()
                    .enumerate()
                    .map(|(i, _)| {
                        Expr::index("memory", Expr::Literal(start_slot + U256::from(i * 32)))
                    })
                    .collect(),
            )
        } else {
            let start = Expr::from_opcode(&instruction.input_operations[offset]);
            let size = Expr::from_opcode(&instruction.input_operations[offset + 1]);
            (
                format!("Unresolved_{}", selector.unwrap_or_default()),
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
            value: value.filter(|_| {
                call.value.is_some_and(|index| instruction.input_operations[index] != w_push0!())
            }),
            comment: Some(opcode_name(instruction.opcode).to_lowercase()),
        });
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::analyze::AnalyzerType;
    use heimdall_vm::{
        core::{memory::Memory, stack::Stack, storage::Storage, vm::VM},
        w_caller, w_callvalue, w_codesize, w_origin, w_push1, w_push32,
    };

    fn call_state(opcode: u8, address: u64) -> State {
        let with_value = matches!(opcode, 0xf1 | 0xf2);
        let mut inputs = vec![U256::from(5000), U256::from(address)];
        let mut operations =
            vec![w_gas!(), if address == 2 { w_push1!(U256::from(address)) } else { w_caller!() }];
        if with_value {
            inputs.push(U256::from(7));
            operations.push(w_callvalue!());
        }
        inputs.extend([U256::from(32), U256::from(32), U256::from(96), U256::from(32)]);
        operations.extend([
            w_push1!(U256::from(32)),
            w_push1!(U256::from(32)),
            w_codesize!(),
            w_origin!(),
        ]);
        let mut memory = Memory::new();
        let mut calldata = [0; 32];
        calldata[..4].copy_from_slice(&[0x12, 0x34, 0x56, 0x78]);
        memory.store(32, 32, &calldata);
        State {
            last_instruction: Instruction {
                instruction: 1,
                opcode,
                inputs,
                outputs: vec![],
                input_operations: operations,
                output_operations: vec![],
            },
            gas_used: 0,
            gas_remaining: 0,
            stack: Stack::new(),
            memory,
            storage: Storage::new(),
            events: vec![],
        }
    }

    fn analyzer_state() -> AnalyzerState {
        AnalyzerState {
            jumped_conditional: None,
            conditional_stack: vec![],
            analyzer_type: AnalyzerType::Solidity,
            skip_resolving: true,
            memory_provenance: Default::default(),
        }
    }

    fn known_calldata(state: &State, analyzer: &mut AnalyzerState) {
        let mut write = state.clone();
        write.last_instruction.opcode = heimdall_vm::core::opcodes::MSTORE;
        write.last_instruction.inputs =
            vec![U256::from(32), U256::from_be_slice(&state.memory.read(32, 32))];
        write.last_instruction.input_operations =
            vec![w_push1!(U256::from(32)), w_push32!(write.last_instruction.inputs[1])];
        analyzer.memory_provenance.observe_write(&write);
    }

    #[tokio::test]
    async fn normalizes_all_call_opcodes_without_losing_value_or_kind() {
        for (opcode, kind, has_value) in [
            (0xf1, "call", true),
            (0xf2, "callcode", true),
            (0xfa, "staticcall", false),
            (0xf4, "delegatecall", false),
        ] {
            let mut function = AnalyzedFunction::new("12345678", false);
            let state = call_state(opcode, 100);
            let mut analyzer = analyzer_state();
            known_calldata(&state, &mut analyzer);
            extcall_heuristic(&mut function, &state, &mut analyzer).await.unwrap();
            match &function.statements[..] {
                [Statement::ExternalCall { address, function, value, comment, .. }] => {
                    assert_eq!(*address, Expr::from_opcode(&w_caller!()));
                    assert!(function.starts_with("Unresolved_12345678"), "{function}");
                    assert_eq!(value.is_some(), has_value);
                    assert_eq!(comment.as_deref(), Some(kind));
                }
                other => panic!("unexpected call: {other:?}"),
            }
        }
    }

    #[tokio::test]
    async fn precompile_calls_are_kept_as_low_level_calls() {
        for opcode in [0xf1, 0xfa] {
            let mut function = AnalyzedFunction::new("12345678", false);
            let mut state = call_state(opcode, 2);
            let output_size = if opcode == 0xf1 { 6 } else { 5 };
            state.last_instruction.input_operations[output_size] = w_push1!(U256::from(32));
            if opcode == 0xf1 {
                state.last_instruction.inputs[2] = U256::ZERO;
                state.last_instruction.input_operations[2] = w_push0!();
            }
            let mut analyzer = analyzer_state();
            known_calldata(&state, &mut analyzer);
            extcall_heuristic(&mut function, &state, &mut analyzer).await.unwrap();
            assert!(matches!(&function.statements[..],
                [Statement::ExternalCall { function, args, .. }]
                    if function.starts_with("Unresolved_") && matches!(&args[..], [Expr::Slice { .. }])));
        }
    }

    #[tokio::test]
    async fn precompile_variants_do_not_invent_memory_assignments() {
        for (opcode, output_size, value) in
            [(0xf2, 32, 0), (0xf4, 32, 0), (0xf1, 32, 7), (0xfa, 0, 0)]
        {
            let mut function = AnalyzedFunction::new("12345678", false);
            let mut state = call_state(opcode, 2);
            let index = if matches!(opcode, 0xf1 | 0xf2) { 6 } else { 5 };
            state.last_instruction.inputs[index] = U256::from(output_size);
            state.last_instruction.input_operations[index] = w_push1!(U256::from(output_size));
            if matches!(opcode, 0xf1 | 0xf2) {
                state.last_instruction.inputs[2] = U256::from(value);
                state.last_instruction.input_operations[2] = w_push1!(U256::from(value));
            }
            let mut analyzer = analyzer_state();
            known_calldata(&state, &mut analyzer);
            extcall_heuristic(&mut function, &state, &mut analyzer).await.unwrap();
            assert!(matches!(&function.statements[..], [Statement::ExternalCall { .. }]));
        }
    }

    #[tokio::test]
    async fn precompile_does_not_reuse_a_stale_memory_expression() {
        let code = "600160005260005160205260026000526020606060206020600261fffffa";
        let function = run_call(code, &[]).await;
        assert!(matches!(&function.statements[..], [Statement::ExternalCall { .. }]));
    }

    #[tokio::test]
    async fn empty_value_call_retains_call_semantics() {
        let mut function = AnalyzedFunction::new("12345678", false);
        let mut state = call_state(0xf1, 100);
        state.last_instruction.inputs[0] = U256::from(2300);
        state.last_instruction.input_operations[0] = w_push32!(U256::from(2300));
        state.last_instruction.inputs[4] = U256::ZERO;
        state.last_instruction.input_operations[4] = w_push0!();
        extcall_heuristic(&mut function, &state, &mut analyzer_state()).await.unwrap();
        assert!(
            matches!(&function.statements[..], [Statement::ExternalCall { function, value: Some(_), .. }] if function != "transfer")
        );
    }

    #[tokio::test]
    async fn symbolic_stipend_placeholder_is_not_rendered_as_transfer() {
        let mut function = AnalyzedFunction::new("12345678", false);
        let mut state = call_state(0xf1, 100);
        state.last_instruction.inputs[0] = U256::from(2300);
        state.last_instruction.inputs[4] = U256::ZERO;
        state.last_instruction.input_operations[4] = w_push0!();
        extcall_heuristic(&mut function, &state, &mut analyzer_state()).await.unwrap();
        assert!(
            matches!(&function.statements[..], [Statement::ExternalCall { function, .. }] if function != "transfer")
        );
    }

    async fn run_call(bytecode: &str, calldata: &[u8]) -> AnalyzedFunction {
        use alloy::primitives::Address;
        use heimdall_common::utils::strings::decode_hex;
        let code = decode_hex(bytecode).unwrap();
        let mut vm =
            VM::new(&code, calldata, Address::ZERO, Address::ZERO, Address::ZERO, 0, u128::MAX);
        let mut function = AnalyzedFunction::new("12345678", false);
        let mut analyzer = analyzer_state();
        while vm.instruction <= vm.bytecode.len() as u128 {
            let state = vm.step().unwrap();
            analyzer.memory_provenance.observe_write(&state);
            if CallOperands::new(&state.last_instruction).is_some() {
                extcall_heuristic(&mut function, &state, &mut analyzer).await.unwrap();
                break;
            }
            analyzer.memory_provenance.observe_call_output(&state);
        }
        function
    }

    fn call_name(function: &AnalyzedFunction) -> &str {
        match &function.statements[..] {
            [Statement::ExternalCall { function, .. }] => function,
            statements => panic!("unexpected call: {statements:?}"),
        }
    }

    #[tokio::test]
    async fn call_data_uses_exact_unaligned_and_overwritten_bytes() {
        let suffix = "5f5f60046001601061fffffa";
        for (prefix, expected) in [
            ("631234567860e01b600152", "Unresolved_12345678"),
            ("631234567860e01b60015260ab600453", "Unresolved_123456ab"),
            ("631234567860e01b60015263aabbccdd60e01b600252", "Unresolved_12aabbcc"),
        ] {
            let function = run_call(&format!("{prefix}{suffix}"), &[]).await;
            assert_eq!(call_name(&function), expected);
        }
    }

    #[tokio::test]
    async fn copied_known_bytes_are_tracked_and_unknown_bytes_are_not_guessed() {
        let known_copy = "631234567860e01b6001526004600160305e5f5f60046030601061fffffa";
        assert_eq!(call_name(&run_call(known_copy, &[]).await), "Unresolved_12345678");

        let unknown_write = "6000356001525f5f60046001601061fffffa";
        assert_eq!(
            call_name(&run_call(unknown_write, &[0x12, 0x34, 0x56, 0x78]).await),
            "Unresolved_"
        );

        let unknown_copy = "631234567860e01b60015260045f6001375f5f60046001601061fffffa";
        assert_eq!(
            call_name(&run_call(unknown_copy, &[0xde, 0xad, 0xbe, 0xef]).await),
            "Unresolved_"
        );
    }

    #[tokio::test]
    async fn short_input_does_not_invent_a_four_byte_selector() {
        let code = "631234567860e01b6001525f5f60036001601061fffffa";
        assert_eq!(call_name(&run_call(code, &[]).await), "Unresolved_");
    }

    #[tokio::test]
    async fn gaps_and_unknown_arguments_do_not_become_decoded_values() {
        let gaps = "601260015360566003535f5f60046001601061fffffa";
        assert_eq!(call_name(&run_call(gaps, &[]).await), "Unresolved_");

        let unknown_argument = "631234567860e01b6001525f5f60246001601061fffffa";
        let function = run_call(unknown_argument, &[]).await;
        assert!(matches!(&function.statements[..],
            [Statement::ExternalCall { function, args, .. }]
                if function == "Unresolved_12345678" && matches!(&args[..], [Expr::Slice { .. }])));
    }

    #[tokio::test]
    async fn bounded_offsets_do_not_alias_other_memory() {
        for (store_offset, call_offset, expected) in [
            ("610901", "610901", "Unresolved_12345678"),
            ("62010000", "62010000", "Unresolved_12345678"),
            ("62010001", "62010001", "Unresolved_"),
        ] {
            let code = format!("631234567860e01b{store_offset}525f5f6004{call_offset}601061fffffa");
            assert_eq!(call_name(&run_call(&code, &[]).await), expected);
        }
    }

    #[tokio::test]
    async fn sibling_branches_do_not_share_known_memory_bytes() {
        use crate::core::analyze::Analyzer;
        use alloy::primitives::Address;
        use heimdall_common::utils::strings::decode_hex;
        use std::time::{Duration, Instant};

        let mut code = vec![0x33, 0x60, 0, 0x57];
        code.extend(decode_hex("60ab6004535f5f60046001601061fffffa00").unwrap());
        code[2] = u8::try_from(code.len()).unwrap();
        code.push(0x5b);
        code.extend(decode_hex("631234567860e01b6001525f5f60046001601061fffffa00").unwrap());
        let mut vm = VM::new(&code, &[], Address::ZERO, Address::ZERO, Address::ZERO, 0, u128::MAX);
        let (trace, details, _) =
            vm.symbolic_exec_detailed(Instant::now() + Duration::from_secs(1)).unwrap();
        let mut analyzer =
            Analyzer::new(AnalyzerType::Solidity, true, AnalyzedFunction::new("fallback", true));
        let function = analyzer.analyze(trace, details).await.unwrap();
        let mut names = function
            .statements
            .iter()
            .filter_map(|statement| match statement {
                Statement::ExternalCall { function, .. } => Some(function.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();
        names.sort();
        assert_eq!(names, vec!["Unresolved_", "Unresolved_12345678"]);
    }
}
