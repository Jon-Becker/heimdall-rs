use alloy::primitives::U256;
use heimdall_vm::{
    core::{
        opcodes::{
            ADD, ADDRESS, AND, BALANCE, BASEFEE, BLOCKHASH, BYTE, CALL, CALLCODE, CALLDATALOAD,
            CALLDATASIZE, CALLER, CALLVALUE, CHAINID, CODESIZE, COINBASE, CREATE, CREATE2,
            DELEGATECALL, DIV, DUP1, DUP10, DUP11, DUP12, DUP13, DUP14, DUP15, DUP16, DUP2, DUP3,
            DUP4, DUP5, DUP6, DUP7, DUP8, DUP9, EQ, EXP, EXTCODEHASH, EXTCODESIZE, GAS, GASLIMIT,
            GASPRICE, GT, ISZERO, JUMP, JUMPDEST, JUMPI, LOG0, LOG1, LOG2, LOG3, LOG4, LT, MLOAD,
            MOD, MSIZE, MSTORE, MSTORE8, MUL, NOT, NUMBER, OR, ORIGIN, POP, PREVRANDAO, PUSH0,
            PUSH1, PUSH10, PUSH11, PUSH12, PUSH13, PUSH14, PUSH15, PUSH16, PUSH17, PUSH18, PUSH19,
            PUSH2, PUSH20, PUSH21, PUSH22, PUSH23, PUSH24, PUSH25, PUSH26, PUSH27, PUSH28, PUSH29,
            PUSH3, PUSH30, PUSH31, PUSH32, PUSH4, PUSH5, PUSH6, PUSH7, PUSH8, PUSH9, RETURN,
            RETURNDATASIZE, REVERT, SAR, SDIV, SELFBALANCE, SGT, SHA3, SHL, SHR, SLOAD, SLT, SMOD,
            SSTORE, STATICCALL, STOP, SUB, SWAP1, SWAP10, SWAP11, SWAP12, SWAP13, SWAP14, SWAP15,
            SWAP16, SWAP2, SWAP3, SWAP4, SWAP5, SWAP6, SWAP7, SWAP8, SWAP9, TIMESTAMP, TLOAD,
            TSTORE, XOR,
        },
        vm::Instruction,
    },
    ext::exec::VMTrace,
};

use crate::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Token {
    Opcode {
        opcode: u8,
        name: String,
        instruction: u128,
    },
    Immediate(U256),
    StackOp {
        op_type: StackOpType,
        index: usize,
        instruction: u128,
    },
    Label(u128),
    Input {
        index: usize,
        value: U256,
        instruction: u128,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StackOpType {
    Dup,
    Swap,
}

pub struct Tokenizer;

impl Tokenizer {
    pub fn tokenize(trace: &VMTrace) -> Result<Vec<Token>, Error> {
        let mut tokens = Vec::new();

        for state in &trace.operations {
            let instruction = &state.last_instruction;
            tokens.extend(Self::tokenize_instruction(instruction)?);
        }

        Ok(tokens)
    }

    fn tokenize_instruction(instruction: &Instruction) -> Result<Vec<Token>, Error> {
        let mut tokens = Vec::new();
        let opcode = instruction.opcode;
        let pc = instruction.instruction;

        // Handle different opcode categories
        match opcode {
            // Push operations
            PUSH0 => {
                tokens.push(Token::Opcode {
                    opcode,
                    name: "PUSH0".to_string(),
                    instruction: pc,
                });
                tokens.push(Token::Immediate(U256::ZERO));
            }
            PUSH1..=PUSH32 => {
                let push_size = (opcode - PUSH1 + 1) as usize;
                tokens.push(Token::Opcode {
                    opcode,
                    name: format!("PUSH{}", push_size),
                    instruction: pc,
                });
                if let Some(value) = instruction.outputs.first() {
                    tokens.push(Token::Immediate(*value));
                }
            }

            // Dup operations
            DUP1..=DUP16 => {
                let index = (opcode - DUP1 + 1) as usize;
                tokens.push(Token::StackOp {
                    op_type: StackOpType::Dup,
                    index,
                    instruction: pc,
                });
            }

            // Swap operations
            SWAP1..=SWAP16 => {
                let index = (opcode - SWAP1 + 1) as usize;
                tokens.push(Token::StackOp {
                    op_type: StackOpType::Swap,
                    index,
                    instruction: pc,
                });
            }

            // Jump operations
            JUMPDEST => {
                tokens.push(Token::Label(pc));
            }
            JUMP | JUMPI => {
                tokens.push(Token::Opcode {
                    opcode,
                    name: opcode_name(opcode).to_string(),
                    instruction: pc,
                });
            }

            // Log operations
            LOG0..=LOG4 => {
                let topic_count = opcode - LOG0;
                tokens.push(Token::Opcode {
                    opcode,
                    name: format!("LOG{}", topic_count),
                    instruction: pc,
                });
            }

            // All other opcodes
            _ => {
                tokens.push(Token::Opcode {
                    opcode,
                    name: opcode_name(opcode).to_string(),
                    instruction: pc,
                });
            }
        }

        // Add input tokens for operations that consume stack values
        for (idx, input) in instruction.inputs.iter().enumerate() {
            tokens.push(Token::Input {
                index: idx,
                value: *input,
                instruction: pc,
            });
        }

        Ok(tokens)
    }
}

fn opcode_name(opcode: u8) -> &'static str {
    match opcode {
        ADD => "ADD",
        MUL => "MUL",
        SUB => "SUB",
        DIV => "DIV",
        SDIV => "SDIV",
        MOD => "MOD",
        SMOD => "SMOD",
        EXP => "EXP",
        NOT => "NOT",
        LT => "LT",
        GT => "GT",
        SLT => "SLT",
        SGT => "SGT",
        EQ => "EQ",
        ISZERO => "ISZERO",
        AND => "AND",
        OR => "OR",
        XOR => "XOR",
        BYTE => "BYTE",
        SHL => "SHL",
        SHR => "SHR",
        SAR => "SAR",
        SHA3 => "SHA3",
        ADDRESS => "ADDRESS",
        BALANCE => "BALANCE",
        ORIGIN => "ORIGIN",
        CALLER => "CALLER",
        CALLVALUE => "CALLVALUE",
        CALLDATALOAD => "CALLDATALOAD",
        CALLDATASIZE => "CALLDATASIZE",
        CODESIZE => "CODESIZE",
        GASPRICE => "GASPRICE",
        EXTCODEHASH => "EXTCODEHASH",
        EXTCODESIZE => "EXTCODESIZE",
        RETURNDATASIZE => "RETURNDATASIZE",
        BLOCKHASH => "BLOCKHASH",
        COINBASE => "COINBASE",
        TIMESTAMP => "TIMESTAMP",
        NUMBER => "NUMBER",
        PREVRANDAO => "PREVRANDAO",
        GASLIMIT => "GASLIMIT",
        CHAINID => "CHAINID",
        SELFBALANCE => "SELFBALANCE",
        BASEFEE => "BASEFEE",
        POP => "POP",
        MLOAD => "MLOAD",
        MSTORE => "MSTORE",
        MSTORE8 => "MSTORE8",
        SLOAD => "SLOAD",
        SSTORE => "SSTORE",
        TLOAD => "TLOAD",
        TSTORE => "TSTORE",
        JUMP => "JUMP",
        JUMPI => "JUMPI",
        JUMPDEST => "JUMPDEST",
        MSIZE => "MSIZE",
        GAS => "GAS",
        CREATE => "CREATE",
        CALL => "CALL",
        CALLCODE => "CALLCODE",
        RETURN => "RETURN",
        DELEGATECALL => "DELEGATECALL",
        CREATE2 => "CREATE2",
        STATICCALL => "STATICCALL",
        REVERT => "REVERT",
        STOP => "STOP",
        _ => "UNKNOWN",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokenize_push() {
        let mut trace = VMTrace {
            instruction: 0,
            gas_used: 0,
            operations: vec![],
            children: vec![],
        };

        let instr = Instruction {
            instruction: 0,
            opcode: PUSH1,
            inputs: vec![],
            outputs: vec![U256::from(0x42)],
            input_operations: vec![],
            output_operations: vec![],
        };

        let state = heimdall_vm::core::vm::State {
            last_instruction: instr,
            gas_used: 0,
            gas_remaining: 0,
            stack: Default::default(),
            memory: Default::default(),
            storage: Default::default(),
            events: vec![],
        };
        trace.operations.push(state);

        let tokens = Tokenizer::tokenize(&trace).unwrap();
        assert_eq!(tokens.len(), 2);
        assert!(matches!(tokens[0], Token::Opcode { opcode: PUSH1, .. }));
        assert_eq!(tokens[1], Token::Immediate(U256::from(0x42)));
    }
}