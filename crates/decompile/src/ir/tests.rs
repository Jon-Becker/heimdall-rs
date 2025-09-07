#[cfg(test)]
mod tests {
    use alloy::primitives::U256;
    use heimdall_vm::{
        core::vm::{Instruction, State},
        ext::exec::VMTrace,
    };

    use crate::ir::{
        passes,
        parser::Parser,
        tokenizer::Tokenizer,
        types::{BinOp, Expr, UnOp},
        SolidityEmitter,
    };

    fn create_test_trace(instructions: Vec<Instruction>) -> VMTrace {
        VMTrace {
            instruction: 0,
            gas_used: 0,
            operations: instructions
                .into_iter()
                .map(|instr| State {
                    last_instruction: instr,
                    gas_used: 0,
                    gas_remaining: 0,
                    stack: Default::default(),
                    memory: Default::default(),
                    storage: Default::default(),
                    events: vec![],
                })
                .collect(),
            children: vec![],
        }
    }

    #[test]
    fn test_simple_arithmetic() {
        use heimdall_vm::core::opcodes::{ADD, PUSH1};

        let trace = create_test_trace(vec![
            Instruction {
                instruction: 0,
                opcode: PUSH1,
                inputs: vec![],
                outputs: vec![U256::from(10)],
                input_operations: vec![],
                output_operations: vec![],
            },
            Instruction {
                instruction: 2,
                opcode: PUSH1,
                inputs: vec![],
                outputs: vec![U256::from(20)],
                input_operations: vec![],
                output_operations: vec![],
            },
            Instruction {
                instruction: 4,
                opcode: ADD,
                inputs: vec![U256::from(10), U256::from(20)],
                outputs: vec![U256::from(30)],
                input_operations: vec![],
                output_operations: vec![],
            },
        ]);

        let tokens = Tokenizer::tokenize(&trace).unwrap();
        let ir = Parser::parse(tokens).unwrap();
        let optimized = passes::run_all_passes(ir).unwrap();
        let emitter = SolidityEmitter::new();
        let output = emitter.emit(&optimized);

        assert!(output.contains("function"));
    }

    #[test]
    fn test_algebraic_simplification() {
        // Test x + 0 = x
        let expr = Expr::BinOp(
            BinOp::Add,
            Box::new(Expr::Var("x".to_string())),
            Box::new(Expr::Const(U256::ZERO)),
        );
        let result = crate::ir::passes::algebraic::simplify_expr(expr).unwrap();
        assert_eq!(result, Expr::Var("x".to_string()));

        // Test x * 1 = x
        let expr = Expr::BinOp(
            BinOp::Mul,
            Box::new(Expr::Var("x".to_string())),
            Box::new(Expr::Const(U256::from(1))),
        );
        let result = crate::ir::passes::algebraic::simplify_expr(expr).unwrap();
        assert_eq!(result, Expr::Var("x".to_string()));

        // Test x - x = 0
        let expr = Expr::BinOp(
            BinOp::Sub,
            Box::new(Expr::Var("x".to_string())),
            Box::new(Expr::Var("x".to_string())),
        );
        let result = crate::ir::passes::algebraic::simplify_expr(expr).unwrap();
        assert_eq!(result, Expr::Const(U256::ZERO));
    }

    #[test]
    fn test_bitwise_simplification() {
        // Test x & 0 = 0
        let expr = Expr::BinOp(
            BinOp::And,
            Box::new(Expr::Var("x".to_string())),
            Box::new(Expr::Const(U256::ZERO)),
        );
        let result = crate::ir::passes::bitwise::simplify_expr(expr).unwrap();
        assert_eq!(result, Expr::Const(U256::ZERO));

        // Test x | 0 = x
        let expr = Expr::BinOp(
            BinOp::Or,
            Box::new(Expr::Var("x".to_string())),
            Box::new(Expr::Const(U256::ZERO)),
        );
        let result = crate::ir::passes::bitwise::simplify_expr(expr).unwrap();
        assert_eq!(result, Expr::Var("x".to_string()));

        // Test x ^ x = 0
        let expr = Expr::BinOp(
            BinOp::Xor,
            Box::new(Expr::Var("x".to_string())),
            Box::new(Expr::Var("x".to_string())),
        );
        let result = crate::ir::passes::bitwise::simplify_expr(expr).unwrap();
        assert_eq!(result, Expr::Const(U256::ZERO));
    }

    #[test]
    fn test_constant_folding() {
        // Test 10 + 20 = 30
        let expr = Expr::BinOp(
            BinOp::Add,
            Box::new(Expr::Const(U256::from(10))),
            Box::new(Expr::Const(U256::from(20))),
        );
        let result = crate::ir::passes::constant_fold::fold_expr(expr).unwrap();
        assert_eq!(result, Expr::Const(U256::from(30)));

        // Test nested: (2 + 3) * 4 = 20
        let expr = Expr::BinOp(
            BinOp::Mul,
            Box::new(Expr::BinOp(
                BinOp::Add,
                Box::new(Expr::Const(U256::from(2))),
                Box::new(Expr::Const(U256::from(3))),
            )),
            Box::new(Expr::Const(U256::from(4))),
        );
        let result = crate::ir::passes::constant_fold::fold_expr(expr).unwrap();
        assert_eq!(result, Expr::Const(U256::from(20)));
    }

    #[test]
    fn test_strength_reduction() {
        // Test x * 8 = x << 3
        let expr = Expr::BinOp(
            BinOp::Mul,
            Box::new(Expr::Var("x".to_string())),
            Box::new(Expr::Const(U256::from(8))),
        );
        let result = crate::ir::passes::strength::reduce_expr(expr).unwrap();
        assert!(matches!(result, Expr::BinOp(BinOp::Shl, _, _)));

        // Test x / 16 = x >> 4
        let expr = Expr::BinOp(
            BinOp::Div,
            Box::new(Expr::Var("x".to_string())),
            Box::new(Expr::Const(U256::from(16))),
        );
        let result = crate::ir::passes::strength::reduce_expr(expr).unwrap();
        assert!(matches!(result, Expr::BinOp(BinOp::Shr, _, _)));

        // Test x % 256 = x & 255
        let expr = Expr::BinOp(
            BinOp::Mod,
            Box::new(Expr::Var("x".to_string())),
            Box::new(Expr::Const(U256::from(256))),
        );
        let result = crate::ir::passes::strength::reduce_expr(expr).unwrap();
        assert!(matches!(result, Expr::BinOp(BinOp::And, _, _)));
    }

    #[test]
    fn test_precedence() {
        // Test operator precedence values
        assert!(BinOp::Mul.precedence() > BinOp::Add.precedence());
        assert!(BinOp::Add.precedence() > BinOp::Eq.precedence());
        assert!(BinOp::Eq.precedence() > BinOp::And.precedence());
        assert!(BinOp::And.precedence() > BinOp::Or.precedence());
    }

    #[test]
    fn test_double_negation() {
        // Test !!x simplification
        let expr = Expr::UnOp(
            UnOp::IsZero,
            Box::new(Expr::UnOp(
                UnOp::IsZero,
                Box::new(Expr::Var("x".to_string())),
            )),
        );
        // For now, double negation is not simplified to preserve semantics
        let result = crate::ir::passes::algebraic::simplify_expr(expr.clone()).unwrap();
        assert_eq!(result, expr);
    }

    #[test]
    fn test_mask_to_cast() {
        // Test address mask conversion
        let address_mask = U256::from_be_bytes([
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
            0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
        ]);
        
        let expr = Expr::BinOp(
            BinOp::And,
            Box::new(Expr::Var("x".to_string())),
            Box::new(Expr::Const(address_mask)),
        );
        let result = crate::ir::passes::bitwise::simplify_expr(expr).unwrap();
        assert!(matches!(result, Expr::Cast(crate::ir::types::SolidityType::Address, _)));
    }

    #[test]
    fn test_wrapping_arithmetic() {
        // Test that overflow wraps correctly
        let expr = Expr::BinOp(
            BinOp::Add,
            Box::new(Expr::Const(U256::MAX)),
            Box::new(Expr::Const(U256::from(1))),
        );
        let result = crate::ir::passes::constant_fold::fold_expr(expr).unwrap();
        assert_eq!(result, Expr::Const(U256::ZERO));

        // Test underflow wraps correctly
        let expr = Expr::BinOp(
            BinOp::Sub,
            Box::new(Expr::Const(U256::ZERO)),
            Box::new(Expr::Const(U256::from(1))),
        );
        let result = crate::ir::passes::constant_fold::fold_expr(expr).unwrap();
        assert_eq!(result, Expr::Const(U256::MAX));
    }
}