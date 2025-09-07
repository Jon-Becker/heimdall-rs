use alloy::primitives::U256;
use std::collections::HashMap;

use crate::{
    ir::{
        tokenizer::{StackOpType, Token},
        types::{BinOp, Block, CallType, Expr, Function, Label, LoadType, Stmt, Terminator, UnOp},
    },
    Error,
};

use heimdall_vm::core::opcodes::{
    ADD, AND, CALL, CALLDATALOAD, CALLVALUE, CREATE, CREATE2, DELEGATECALL, DIV, EQ, EXP, GT,
    ISZERO, JUMP, JUMPI, LOG0, LOG1, LOG2, LOG3, LOG4, LT, MLOAD, MOD, MUL, NOT, OR, RETURN,
    REVERT, SAR, SDIV, SGT, SHA3, SHL, SHR, SLOAD, SLT, SMOD, SSTORE, STATICCALL, STOP, SUB,
    TLOAD, TSTORE, XOR,
};

pub struct Parser {
    tokens: Vec<Token>,
    position: usize,
    stack: Vec<Expr>,
    variables: HashMap<String, Expr>,
    var_counter: usize,
}

impl Parser {
    pub fn parse(tokens: Vec<Token>) -> Result<Function, Error> {
        let mut parser = Parser {
            tokens,
            position: 0,
            stack: Vec::new(),
            variables: HashMap::new(),
            var_counter: 0,
        };

        let blocks = parser.parse_blocks()?;

        Ok(Function {
            selector: None,
            params: vec![],
            returns: vec![],
            blocks,
            modifiers: vec![],
            visibility: crate::ir::types::Visibility::Public,
        })
    }

    fn parse_blocks(&mut self) -> Result<Vec<Block>, Error> {
        let mut blocks = Vec::new();
        let mut current_block = Block {
            label: None,
            stmts: Vec::new(),
            terminator: None,
        };

        while self.position < self.tokens.len() {
            let token = &self.tokens[self.position].clone();

            match token {
                Token::Label(pc) => {
                    // Save current block if it has content
                    if !current_block.stmts.is_empty() || current_block.terminator.is_some() {
                        blocks.push(current_block);
                    }
                    // Start new block with label
                    current_block = Block {
                        label: Some(Label(*pc)),
                        stmts: Vec::new(),
                        terminator: None,
                    };
                    self.position += 1;
                }
                Token::Opcode { opcode, .. } => {
                    let stmt = self.parse_opcode(*opcode)?;
                    if let Some(stmt) = stmt {
                        match stmt {
                            ParsedItem::Stmt(s) => current_block.stmts.push(s),
                            ParsedItem::Term(t) => {
                                current_block.terminator = Some(t);
                                blocks.push(current_block);
                                current_block = Block {
                                    label: None,
                                    stmts: Vec::new(),
                                    terminator: None,
                                };
                            }
                            ParsedItem::Expr(_) => {} // Expression pushed to stack
                        }
                    }
                    self.position += 1;
                }
                Token::Immediate(value) => {
                    self.stack.push(Expr::Const(*value));
                    self.position += 1;
                }
                Token::StackOp { op_type, index, .. } => {
                    self.handle_stack_op(op_type.clone(), *index)?;
                    self.position += 1;
                }
                Token::Input { .. } => {
                    // Inputs are handled within opcode parsing
                    self.position += 1;
                }
            }
        }

        // Add final block if it has content
        if !current_block.stmts.is_empty() || current_block.terminator.is_some() {
            blocks.push(current_block);
        }

        Ok(blocks)
    }

    fn parse_opcode(&mut self, opcode: u8) -> Result<Option<ParsedItem>, Error> {
        match opcode {
            // Arithmetic operations
            ADD => self.parse_binop(BinOp::Add),
            SUB => self.parse_binop(BinOp::Sub),
            MUL => self.parse_binop(BinOp::Mul),
            DIV => self.parse_binop(BinOp::Div),
            SDIV => self.parse_binop(BinOp::Div), // TODO: Handle signed
            MOD => self.parse_binop(BinOp::Mod),
            SMOD => self.parse_binop(BinOp::Mod), // TODO: Handle signed
            EXP => self.parse_binop(BinOp::Exp),

            // Bitwise operations
            AND => self.parse_binop(BinOp::And),
            OR => self.parse_binop(BinOp::Or),
            XOR => self.parse_binop(BinOp::Xor),
            NOT => self.parse_unop(UnOp::Not),
            SHL => self.parse_binop(BinOp::Shl),
            SHR => self.parse_binop(BinOp::Shr),
            SAR => self.parse_binop(BinOp::Sar),

            // Comparison operations
            LT => self.parse_binop(BinOp::Lt),
            GT => self.parse_binop(BinOp::Gt),
            SLT => self.parse_binop(BinOp::Slt),
            SGT => self.parse_binop(BinOp::Sgt),
            EQ => self.parse_binop(BinOp::Eq),
            ISZERO => self.parse_unop(UnOp::IsZero),

            // Memory operations
            MLOAD => {
                let addr = self.pop_stack()?;
                self.stack.push(Expr::Load(LoadType::Memory, Box::new(addr)));
                Ok(None)
            }
            SLOAD => {
                let slot = self.pop_stack()?;
                self.stack.push(Expr::Load(LoadType::Storage, Box::new(slot)));
                Ok(None)
            }
            TLOAD => {
                let slot = self.pop_stack()?;
                self.stack.push(Expr::Load(LoadType::Transient, Box::new(slot)));
                Ok(None)
            }
            CALLDATALOAD => {
                let offset = self.pop_stack()?;
                self.stack.push(Expr::Load(LoadType::Calldata, Box::new(offset)));
                Ok(None)
            }

            // Store operations
            SSTORE => {
                let slot = self.pop_stack()?;
                let value = self.pop_stack()?;
                Ok(Some(ParsedItem::Stmt(Stmt::Store(
                    crate::ir::types::StoreType::Storage,
                    slot,
                    value,
                ))))
            }
            TSTORE => {
                let slot = self.pop_stack()?;
                let value = self.pop_stack()?;
                Ok(Some(ParsedItem::Stmt(Stmt::Store(
                    crate::ir::types::StoreType::Transient,
                    slot,
                    value,
                ))))
            }

            // Control flow
            JUMP => {
                let dest = self.pop_stack()?;
                if let Expr::Const(addr) = dest {
                    let label = Label(addr.try_into().unwrap_or(0));
                    Ok(Some(ParsedItem::Term(Terminator::Jump(label))))
                } else {
                    // Dynamic jump - create a placeholder
                    Ok(Some(ParsedItem::Term(Terminator::Jump(Label(0)))))
                }
            }
            JUMPI => {
                let dest = self.pop_stack()?;
                let cond = self.pop_stack()?;
                if let Expr::Const(addr) = dest {
                    let label = Label(addr.try_into().unwrap_or(0));
                    Ok(Some(ParsedItem::Term(Terminator::ConditionalJump(cond, label, None))))
                } else {
                    // Dynamic jump
                    Ok(Some(ParsedItem::Term(Terminator::ConditionalJump(cond, Label(0), None))))
                }
            }

            // Terminating operations
            RETURN => {
                let _offset = self.pop_stack()?;
                let _size = self.pop_stack()?;
                // TODO: Handle return data properly
                Ok(Some(ParsedItem::Term(Terminator::Return(vec![]))))
            }
            REVERT => {
                let _offset = self.pop_stack()?;
                let _size = self.pop_stack()?;
                // TODO: Handle revert data properly
                Ok(Some(ParsedItem::Term(Terminator::Revert(vec![]))))
            }
            STOP => Ok(Some(ParsedItem::Term(Terminator::Stop))),

            // Call operations
            CALL => {
                let gas = self.pop_stack()?;
                let address = self.pop_stack()?;
                let value = self.pop_stack()?;
                let _args_offset = self.pop_stack()?;
                let _args_size = self.pop_stack()?;
                let _ret_offset = self.pop_stack()?;
                let _ret_size = self.pop_stack()?;

                // Push success flag
                let var_name = self.new_var();
                self.stack.push(Expr::Var(var_name.clone()));

                Ok(Some(ParsedItem::Stmt(Stmt::Call(CallType::Call(
                    Box::new(address),
                    Box::new(value),
                    vec![gas],
                )))))
            }
            DELEGATECALL => {
                let gas = self.pop_stack()?;
                let address = self.pop_stack()?;
                let _args_offset = self.pop_stack()?;
                let _args_size = self.pop_stack()?;
                let _ret_offset = self.pop_stack()?;
                let _ret_size = self.pop_stack()?;

                // Push success flag
                let var_name = self.new_var();
                self.stack.push(Expr::Var(var_name.clone()));

                Ok(Some(ParsedItem::Stmt(Stmt::Call(CallType::DelegateCall(
                    Box::new(address),
                    vec![gas],
                )))))
            }
            STATICCALL => {
                let gas = self.pop_stack()?;
                let address = self.pop_stack()?;
                let _args_offset = self.pop_stack()?;
                let _args_size = self.pop_stack()?;
                let _ret_offset = self.pop_stack()?;
                let _ret_size = self.pop_stack()?;

                // Push success flag
                let var_name = self.new_var();
                self.stack.push(Expr::Var(var_name.clone()));

                Ok(Some(ParsedItem::Stmt(Stmt::Call(CallType::StaticCall(
                    Box::new(address),
                    vec![gas],
                )))))
            }
            CREATE => {
                let value = self.pop_stack()?;
                let _offset = self.pop_stack()?;
                let _size = self.pop_stack()?;
                
                // Push created address
                let var_name = self.new_var();
                self.stack.push(Expr::Var(var_name.clone()));

                Ok(Some(ParsedItem::Stmt(Stmt::Call(CallType::Create(
                    Box::new(value),
                    Box::new(Expr::Const(U256::ZERO)), // TODO: Handle code properly
                )))))
            }
            CREATE2 => {
                let value = self.pop_stack()?;
                let _offset = self.pop_stack()?;
                let _size = self.pop_stack()?;
                let salt = self.pop_stack()?;
                
                // Push created address
                let var_name = self.new_var();
                self.stack.push(Expr::Var(var_name.clone()));

                Ok(Some(ParsedItem::Stmt(Stmt::Call(CallType::Create2(
                    Box::new(value),
                    Box::new(Expr::Const(U256::ZERO)), // TODO: Handle code properly
                    Box::new(salt),
                )))))
            }

            // Log operations
            LOG0 => {
                let _offset = self.pop_stack()?;
                let _size = self.pop_stack()?;
                Ok(Some(ParsedItem::Stmt(Stmt::Log(0, vec![]))))
            }
            LOG1 => {
                let _offset = self.pop_stack()?;
                let _size = self.pop_stack()?;
                let topic = self.pop_stack()?;
                Ok(Some(ParsedItem::Stmt(Stmt::Log(1, vec![topic]))))
            }
            LOG2 => {
                let _offset = self.pop_stack()?;
                let _size = self.pop_stack()?;
                let t1 = self.pop_stack()?;
                let t2 = self.pop_stack()?;
                Ok(Some(ParsedItem::Stmt(Stmt::Log(2, vec![t1, t2]))))
            }
            LOG3 => {
                let _offset = self.pop_stack()?;
                let _size = self.pop_stack()?;
                let t1 = self.pop_stack()?;
                let t2 = self.pop_stack()?;
                let t3 = self.pop_stack()?;
                Ok(Some(ParsedItem::Stmt(Stmt::Log(3, vec![t1, t2, t3]))))
            }
            LOG4 => {
                let _offset = self.pop_stack()?;
                let _size = self.pop_stack()?;
                let t1 = self.pop_stack()?;
                let t2 = self.pop_stack()?;
                let t3 = self.pop_stack()?;
                let t4 = self.pop_stack()?;
                Ok(Some(ParsedItem::Stmt(Stmt::Log(4, vec![t1, t2, t3, t4]))))
            }

            // SHA3
            SHA3 => {
                let _offset = self.pop_stack()?;
                let _size = self.pop_stack()?;
                // TODO: Implement proper SHA3 handling
                let var_name = self.new_var();
                self.stack.push(Expr::Var(var_name));
                Ok(None)
            }

            // Environment operations
            CALLVALUE => {
                self.stack.push(Expr::Var("msg.value".to_string()));
                Ok(None)
            }

            _ => {
                // For unhandled opcodes, create a variable
                let var_name = self.new_var();
                self.stack.push(Expr::Var(var_name));
                Ok(None)
            }
        }
    }

    fn parse_binop(&mut self, op: BinOp) -> Result<Option<ParsedItem>, Error> {
        let right = self.pop_stack()?;
        let left = self.pop_stack()?;
        self.stack.push(Expr::BinOp(op, Box::new(left), Box::new(right)));
        Ok(None)
    }

    fn parse_unop(&mut self, op: UnOp) -> Result<Option<ParsedItem>, Error> {
        let operand = self.pop_stack()?;
        self.stack.push(Expr::UnOp(op, Box::new(operand)));
        Ok(None)
    }

    fn handle_stack_op(&mut self, op_type: StackOpType, index: usize) -> Result<(), Error> {
        match op_type {
            StackOpType::Dup => {
                if self.stack.len() >= index {
                    let item = self.stack[self.stack.len() - index].clone();
                    self.stack.push(item);
                }
            }
            StackOpType::Swap => {
                let stack_len = self.stack.len();
                if stack_len > index {
                    self.stack.swap(stack_len - 1, stack_len - index - 1);
                }
            }
        }
        Ok(())
    }

    fn pop_stack(&mut self) -> Result<Expr, Error> {
        self.stack.pop().ok_or_else(|| Error::Eyre(eyre::eyre!("Stack underflow")))
    }

    fn new_var(&mut self) -> String {
        let name = format!("var_{}", self.var_counter);
        self.var_counter += 1;
        name
    }
}

enum ParsedItem {
    Stmt(Stmt),
    Term(Terminator),
    Expr(Expr),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_arithmetic() {
        let tokens = vec![
            Token::Immediate(U256::from(10)),
            Token::Immediate(U256::from(20)),
            Token::Opcode {
                opcode: ADD,
                name: "ADD".to_string(),
                instruction: 0,
            },
        ];

        let result = Parser::parse(tokens).unwrap();
        assert_eq!(result.blocks.len(), 1);
    }
}