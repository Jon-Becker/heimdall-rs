use crate::{
    core::{
        ir::{BinaryOp, Expr, Statement, UnaryOp},
        postprocess::PostprocessorState,
        types::SolidityType,
    },
    interfaces::AnalyzedFunction,
    Error,
};

fn infer_type(expr: &Expr, state: &PostprocessorState) -> SolidityType {
    match expr {
        Expr::Identifier(name) => match name.as_str() {
            "msg.sender" | "tx.origin" | "address(this)" => SolidityType::Address,
            "msg.value" | "block.timestamp" | "block.number" | "block.chainid" => {
                SolidityType::Uint(256)
            }
            _ => state
                .memory_type_map
                .get(name)
                .or_else(|| state.storage_type_map.get(name))
                .cloned()
                .unwrap_or(SolidityType::Unknown),
        },
        Expr::Literal(_) => SolidityType::Uint(256),
        Expr::Bool(_) => SolidityType::Bool,
        Expr::StringLiteral(_) => SolidityType::String,
        Expr::Cast { ty, .. } => ty.clone(),
        Expr::Unary { op: UnaryOp::LogicalNot, .. } => SolidityType::Bool,
        Expr::Unary { .. } => SolidityType::Uint(256),
        Expr::Binary { op, .. } => match op {
            BinaryOp::LogicalAnd |
            BinaryOp::Lt |
            BinaryOp::Le |
            BinaryOp::Gt |
            BinaryOp::Ge |
            BinaryOp::Eq |
            BinaryOp::Ne => SolidityType::Bool,
            _ => SolidityType::Uint(256),
        },
        Expr::Call { callee, .. } if callee == "keccak256" => SolidityType::FixedBytes(32),
        Expr::Keccak { .. } => SolidityType::FixedBytes(32),
        Expr::Index { base, .. } => infer_type(base, state).indexed(),
        Expr::Member { base, member } if member == "balance" => {
            let _ = base;
            SolidityType::Uint(256)
        }
        _ => SolidityType::Unknown,
    }
}

/// Removes casts proven redundant by ABI, builtin, local-variable, or storage type information.
pub(crate) fn type_cleanup_postprocessor(
    statement: &mut Statement,
    state: &mut PostprocessorState,
) -> Result<(), Error> {
    statement.visit_exprs_mut(&mut |expr| match expr {
        Expr::Literal(value) if *value == alloy::primitives::U256::MAX => {
            *expr = Expr::identifier("type(uint256).max");
        }
        Expr::Cast { ty, value } => {
            let target = ty.without_location();
            if target != SolidityType::Unknown &&
                infer_type(value, state).without_location() == target
            {
                *expr = *value.clone();
            }
        }
        _ => {}
    });
    Ok(())
}

/// Rewrites literal return values using the function's inferred return type.
pub(crate) fn normalize_typed_returns(
    function: &mut AnalyzedFunction,
    _: &mut PostprocessorState,
) -> Result<(), Error> {
    if function.returns.as_ref() != Some(&SolidityType::Bool) {
        return Ok(())
    }
    for statement in &mut function.statements {
        if let Statement::Return(Expr::Literal(value)) = statement {
            if value.is_zero() || *value == alloy::primitives::U256::from(1) {
                *statement = Statement::Return(Expr::Bool(!value.is_zero()));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::ir::RenderTarget;

    #[test]
    fn renders_uint256_max_symbolically() {
        let mut statement = Statement::Return(Expr::Literal(alloy::primitives::U256::MAX));
        type_cleanup_postprocessor(&mut statement, &mut PostprocessorState::default()).unwrap();
        assert_eq!(statement.render(RenderTarget::Solidity), "return type(uint256).max;");
    }

    #[test]
    fn renders_boolean_return() {
        let mut function = AnalyzedFunction::new("00000000", false);
        function.returns = Some(SolidityType::Bool);
        function.statements =
            vec![Statement::Return(Expr::Literal(alloy::primitives::U256::from(1)))];
        normalize_typed_returns(&mut function, &mut PostprocessorState::default()).unwrap();
        assert_eq!(function.statements[0].render(RenderTarget::Solidity), "return true;");
    }

    #[test]
    fn removes_redundant_address_casts() {
        let mut statement = Statement::Return(Expr::Cast {
            ty: SolidityType::Address,
            value: Box::new(Expr::Cast {
                ty: SolidityType::Address,
                value: Box::new(Expr::identifier("arg0")),
            }),
        });
        let mut state = PostprocessorState::default();
        state.memory_type_map.insert("arg0".to_string(), SolidityType::Address);
        type_cleanup_postprocessor(&mut statement, &mut state).unwrap();
        assert_eq!(statement.render(RenderTarget::Solidity), "return arg0;");
    }

    #[test]
    fn removes_redundant_address_conversion() {
        let mut statement = Statement::Return(Expr::Cast {
            ty: SolidityType::Address,
            value: Box::new(Expr::identifier("msg.sender")),
        });
        type_cleanup_postprocessor(&mut statement, &mut PostprocessorState::default()).unwrap();
        assert_eq!(statement.render(RenderTarget::Solidity), "return msg.sender;");
    }

    #[test]
    fn keeps_cast_for_single_index_into_nested_mapping() {
        let mut statement = Statement::Return(Expr::Cast {
            ty: SolidityType::Bool,
            value: Box::new(Expr::Index {
                base: Box::new(Expr::identifier("storage_map")),
                index: Box::new(Expr::identifier("arg0")),
            }),
        });
        let mut state = PostprocessorState::default();
        state.storage_type_map.insert(
            "storage_map".to_string(),
            SolidityType::Mapping {
                key: Box::new(SolidityType::Address),
                value: Box::new(SolidityType::Mapping {
                    key: Box::new(SolidityType::Uint(256)),
                    value: Box::new(SolidityType::Bool),
                }),
            },
        );
        type_cleanup_postprocessor(&mut statement, &mut state).unwrap();
        assert_eq!(statement.render(RenderTarget::Solidity), "return bool(storage_map[arg0]);");
    }

    #[test]
    fn infers_value_after_indexing_every_mapping_layer() {
        let indexed = Expr::Index {
            base: Box::new(Expr::Index {
                base: Box::new(Expr::identifier("storage_map")),
                index: Box::new(Expr::identifier("arg0")),
            }),
            index: Box::new(Expr::identifier("arg1")),
        };
        let mut state = PostprocessorState::default();
        state.storage_type_map.insert(
            "storage_map".to_string(),
            SolidityType::Mapping {
                key: Box::new(SolidityType::Address),
                value: Box::new(SolidityType::Mapping {
                    key: Box::new(SolidityType::Uint(256)),
                    value: Box::new(SolidityType::Bool),
                }),
            },
        );
        assert_eq!(infer_type(&indexed, &state), SolidityType::Bool);
    }

    #[test]
    fn keeps_narrowing_cast() {
        let mut statement = Statement::Return(Expr::Cast {
            ty: SolidityType::Uint(8),
            value: Box::new(Expr::identifier("arg0")),
        });
        let mut state = PostprocessorState::default();
        state.memory_type_map.insert("arg0".to_string(), SolidityType::Uint(256));
        type_cleanup_postprocessor(&mut statement, &mut state).unwrap();
        assert_eq!(statement.render(RenderTarget::Solidity), "return uint8(arg0);");
    }
}
