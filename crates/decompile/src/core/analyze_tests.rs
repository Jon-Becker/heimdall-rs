use alloy::primitives::U256;
use heimdall_vm::{
    core::{
        memory::Memory,
        opcodes::{self, WrappedOpcode},
        stack::Stack,
        storage::Storage,
        vm::{Instruction, State},
    },
    ext::exec::VMTrace,
};

use crate::{
    core::{
        analyze::{Analyzer, AnalyzerType},
        ir::{Expr, RenderTarget, Statement},
        types::SolidityType,
    },
    interfaces::AnalyzedFunction,
};

fn literal(value: u64) -> WrappedOpcode {
    WrappedOpcode::new(opcodes::PUSH32, vec![U256::from(value).into()])
}

fn condition() -> WrappedOpcode {
    WrappedOpcode::new(opcodes::CALLDATALOAD, vec![U256::from(4).into()])
}

fn state(pc: u128, opcode: u8, inputs: &[u64]) -> State {
    State {
        last_instruction: Instruction {
            instruction: pc,
            opcode,
            inputs: inputs.iter().map(|value| U256::from(*value)).collect(),
            outputs: Vec::new(),
            input_operations: inputs.iter().map(|value| literal(*value)).collect(),
            output_operations: Vec::new(),
        },
        gas_used: 0,
        gas_remaining: 0,
        stack: Stack::new(),
        memory: Memory::new(),
        storage: Storage::new(),
        events: Vec::new(),
    }
}

fn trace(pc: u128, operations: Vec<State>) -> VMTrace {
    VMTrace { instruction: pc, gas_used: 0, operations, children: Vec::new() }
}

fn branch(pc: u128, destination: u64, condition: WrappedOpcode, children: Vec<VMTrace>) -> VMTrace {
    let mut jump = state(pc, opcodes::JUMPI, &[destination, 1]);
    jump.last_instruction.input_operations[1] = condition;
    VMTrace { children, ..trace(pc, vec![jump]) }
}

fn returning(pc: u128) -> VMTrace {
    trace(
        pc,
        vec![state(pc, opcodes::MSTORE, &[0, 0xffff]), state(pc + 1, opcodes::RETURN, &[0, 32])],
    )
}

fn reverting(pc: u128, value: u64) -> VMTrace {
    trace(
        pc,
        vec![state(pc, opcodes::MSTORE, &[0, value]), state(pc + 1, opcodes::REVERT, &[0, 32])],
    )
}

fn call(callee: &str, values: &[u64]) -> Statement {
    Statement::Expression(Expr::Call {
        callee: callee.to_string(),
        args: values.iter().map(|value| Expr::Literal(U256::from(*value))).collect(),
    })
}

fn return_body() -> Vec<Statement> {
    vec![call("mstore", &[0, 0xffff]), call("return", &[0, 32])]
}

fn revert_body(value: u64) -> Vec<Statement> {
    vec![call("mstore", &[0, value]), call("revert", &[0, 32])]
}

async fn analyze(trace: VMTrace) -> Vec<Statement> {
    Analyzer::new(AnalyzerType::Yul, true, AnalyzedFunction::new("b3de648b", false))
        .analyze(trace)
        .await
        .expect("synthetic trace should analyze")
        .statements
}

#[tokio::test]
async fn yul_preserves_revert_branch_and_payload_for_either_child_order() {
    for revert_taken in [false, true] {
        for reverse_children in [false, true] {
            let (taken, fallthrough) = if revert_taken {
                (reverting(21, 0xe0), returning(11))
            } else {
                (returning(21), reverting(11, 0xe0))
            };
            let mut children = vec![taken, fallthrough];
            if reverse_children {
                children.reverse();
            }
            let statements = analyze(branch(10, 20, condition(), children)).await;
            let (then_body, else_body) = if revert_taken {
                (revert_body(0xe0), return_body())
            } else {
                (return_body(), revert_body(0xe0))
            };
            assert_eq!(
                statements,
                vec![Statement::IfElse {
                    condition: Expr::from_yul_opcode(&condition()),
                    then_body,
                    else_body,
                }],
                "revert_taken={revert_taken}, reverse_children={reverse_children}",
            );
        }
    }
}

#[tokio::test]
async fn yul_keeps_repeated_nested_conditions_in_their_own_scopes() {
    let mut nested = branch(30, 40, condition(), vec![reverting(31, 0xe1), returning(41)]);
    nested.instruction = 21;
    let statements = analyze(branch(10, 20, condition(), vec![reverting(11, 0xe2), nested])).await;
    assert_eq!(
        statements,
        vec![Statement::IfElse {
            condition: Expr::from_yul_opcode(&condition()),
            then_body: vec![Statement::IfElse {
                condition: Expr::from_yul_opcode(&condition()),
                then_body: return_body(),
                else_body: revert_body(0xe1),
            }],
            else_body: revert_body(0xe2),
        }],
    );
}

#[tokio::test]
async fn yul_retains_both_revert_paths() {
    let statements =
        analyze(branch(10, 20, condition(), vec![reverting(11, 0xe1), reverting(21, 0xe2)])).await;
    assert_eq!(
        statements,
        vec![Statement::IfElse {
            condition: Expr::from_yul_opcode(&condition()),
            then_body: revert_body(0xe2),
            else_body: revert_body(0xe1),
        }],
    );
}

#[tokio::test]
async fn yul_emits_unconditional_empty_custom_error_and_panic_reverts() {
    for data in [
        vec![],
        vec![0x08, 0xc3, 0x79, 0xa0],
        vec![0xde, 0xad, 0xbe, 0xef],
        vec![0x4e, 0x48, 0x7b, 0x71],
    ] {
        let mut revert = state(1, opcodes::REVERT, &[0, data.len() as u64]);
        revert.memory.store(0, data.len(), &data);
        assert_eq!(
            analyze(trace(1, vec![revert])).await,
            vec![call("revert", &[0, data.len() as u64])],
            "revert payload {data:x?} must not be dropped",
        );
    }
}

#[tokio::test]
async fn yul_guards_a_sole_symbolic_child_with_its_actual_polarity() {
    for taken in [false, true] {
        for size in [0, 32] {
            let pc = if taken { 21 } else { 11 };
            let child = trace(pc, vec![state(pc, opcodes::REVERT, &[0, size])]);
            let body = vec![call("revert", &[0, size])];
            let (then_body, else_body) =
                if taken { (body, Vec::new()) } else { (Vec::new(), body) };
            assert_eq!(
                analyze(branch(10, 20, condition(), vec![child])).await,
                vec![Statement::IfElse {
                    condition: Expr::from_yul_opcode(&condition()),
                    then_body,
                    else_body,
                }],
                "taken={taken}, revert size={size}",
            );
        }
    }
}

#[tokio::test]
async fn yul_flattens_only_the_feasible_child_of_a_constant_branch() {
    let caller = WrappedOpcode::new(opcodes::CALLER, Vec::new());
    let identity = WrappedOpcode::new(opcodes::EQ, vec![caller.clone().into(), caller.into()]);
    for (condition, taken) in [(identity, true), (literal(0), false)] {
        let stop_pc = if taken { 21 } else { 11 };
        let revert_pc = if taken { 11 } else { 21 };
        let children = vec![
            reverting(revert_pc, 0xe0),
            trace(stop_pc, vec![state(stop_pc, opcodes::STOP, &[])]),
        ];
        assert_eq!(analyze(branch(10, 20, condition, children)).await, vec![call("stop", &[])]);
    }
}

#[tokio::test]
async fn yul_keeps_the_continuation_of_an_incomplete_constant_branch() {
    for taken in [false, true] {
        let feasible_pc = if taken { 21 } else { 11 };
        let sibling_pc = if taken { 11 } else { 21 };
        let mut incomplete = branch(30, 40, condition(), Vec::new());
        incomplete.instruction = feasible_pc;
        incomplete.operations.insert(0, state(feasible_pc, opcodes::MSTORE, &[0, 0xffff]));
        let sibling = trace(sibling_pc, vec![state(sibling_pc, opcodes::RETURN, &[0, 32])]);
        assert_eq!(
            analyze(branch(10, 20, literal(u64::from(taken)), vec![sibling, incomplete])).await,
            return_body(),
            "taken={taken}",
        );
    }
}

#[tokio::test]
async fn yul_preserves_stop_as_a_terminal_alternative_to_return() {
    let statements = analyze(branch(
        10,
        20,
        condition(),
        vec![returning(11), trace(21, vec![state(21, opcodes::STOP, &[])])],
    ))
    .await;
    assert_eq!(
        statements,
        vec![Statement::IfElse {
            condition: Expr::from_yul_opcode(&condition()),
            then_body: vec![call("stop", &[])],
            else_body: return_body(),
        }],
    );
}

#[tokio::test]
async fn yul_renders_nonboolean_memory_conditions_once_with_valid_branch_syntax() {
    let condition = WrappedOpcode::new(opcodes::MLOAD, vec![U256::ZERO.into()]);
    let statements =
        analyze(branch(10, 20, condition, vec![reverting(11, 0xe0), returning(21)])).await;
    let source = statements
        .iter()
        .flat_map(|statement| statement.render_lines(RenderTarget::Yul))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(source.starts_with("switch iszero(mload(0))\ncase 0 {"), "{source}");
    assert_eq!(source.matches("mload(0)").count(), 1, "{source}");
    assert!(source.contains("default {"), "{source}");
    assert!(!source.contains("else"), "{source}");
    assert!(!source.contains(';'), "{source}");
}

#[tokio::test]
async fn yul_does_not_infer_a_return_type_from_a_sibling_revert_payload() {
    for reverse_children in [false, true] {
        let mut shared_store = state(1, opcodes::MSTORE, &[0, 0]);
        shared_store.last_instruction.input_operations[1] =
            WrappedOpcode::new(opcodes::CALLER, Vec::new());
        let mut children =
            vec![trace(11, vec![state(11, opcodes::RETURN, &[0, 32])]), reverting(21, 0xe0)];
        if reverse_children {
            children.reverse();
        }
        let mut root = branch(10, 20, condition(), children);
        root.operations.insert(0, shared_store);
        let function =
            Analyzer::new(AnalyzerType::Yul, true, AnalyzedFunction::new("b3de648b", false))
                .analyze(root)
                .await
                .expect("synthetic trace should analyze");
        assert_eq!(function.returns, Some(SolidityType::Address));
    }
}

#[tokio::test]
async fn yul_preserves_complete_branches_when_solidity_lowering_loses_information() {
    let value = WrappedOpcode::new(opcodes::CALLVALUE, Vec::new());
    let unary = |opcode, operand: WrappedOpcode| WrappedOpcode::new(opcode, vec![operand.into()]);
    let binary = |opcode, lhs: WrappedOpcode, rhs: WrappedOpcode| {
        WrappedOpcode::new(opcode, vec![lhs.into(), rhs.into()])
    };
    let identities = [
        (binary(opcodes::BYTE, literal(0), value.clone()), value.clone()),
        (
            binary(opcodes::SDIV, value.clone(), literal(2)),
            binary(opcodes::DIV, value.clone(), literal(2)),
        ),
        (
            binary(opcodes::SMOD, value.clone(), literal(2)),
            binary(opcodes::MOD, value.clone(), literal(2)),
        ),
        (
            binary(opcodes::SLT, value.clone(), literal(0)),
            binary(opcodes::LT, value.clone(), literal(0)),
        ),
        (
            binary(opcodes::SAR, literal(1), value.clone()),
            binary(opcodes::SHR, literal(1), value.clone()),
        ),
        (unary(opcodes::ISZERO, unary(opcodes::ISZERO, value.clone())), value),
    ];

    for (lhs, rhs) in identities {
        let equality = binary(opcodes::EQ, lhs, rhs);
        for inverted in [false, true] {
            let condition =
                if inverted { unary(opcodes::ISZERO, equality.clone()) } else { equality.clone() };
            for terminal in [opcodes::REVERT, opcodes::STOP] {
                for reverse_children in [false, true] {
                    let terminal_pc = if inverted { 11 } else { 21 };
                    let return_pc = if inverted { 21 } else { 11 };
                    let (inputs, terminal_body) = if terminal == opcodes::REVERT {
                        (vec![0, 0], vec![call("revert", &[0, 0])])
                    } else {
                        (vec![], vec![call("stop", &[])])
                    };
                    let mut children = vec![
                        trace(terminal_pc, vec![state(terminal_pc, terminal, &inputs)]),
                        returning(return_pc),
                    ];
                    if reverse_children {
                        children.reverse();
                    }
                    let (then_body, else_body) = if inverted {
                        (return_body(), terminal_body)
                    } else {
                        (terminal_body, return_body())
                    };
                    assert_eq!(
                        analyze(branch(10, 20, condition.clone(), children)).await,
                        vec![Statement::IfElse {
                            condition: Expr::from_yul_opcode(&condition),
                            then_body,
                            else_body,
                        }],
                        "condition={condition:?}, terminal={terminal:#x}, reverse_children={reverse_children}",
                    );
                }
            }
        }
    }
}
