use crate::{
    core::{
        ir::{BinaryOp, Expr, Statement, UnaryOp},
        postprocess::PostprocessorState,
    },
    interfaces::AnalyzedFunction,
    Error,
};

#[derive(Clone, Debug, Eq, PartialEq)]
enum InferredType {
    Address,
    Bool,
    Uint(u16),
    Int(u16),
    Bytes(u16),
    DynamicBytes,
    String,
    Unknown,
}

impl InferredType {
    fn parse(ty: &str) -> Self {
        let ty = ty.trim().trim_end_matches(" memory");
        match ty {
            "address" => Self::Address,
            "bool" => Self::Bool,
            "bytes" => Self::DynamicBytes,
            "string" => Self::String,
            _ if ty.starts_with("uint") => Self::Uint(ty[4..].parse().unwrap_or(256)),
            _ if ty.starts_with("int") => Self::Int(ty[3..].parse().unwrap_or(256)),
            _ if ty.starts_with("bytes") => Self::Bytes(ty[5..].parse::<u16>().unwrap_or(32) * 8),
            _ => Self::Unknown,
        }
    }
}

fn mapping_value_type(ty: &str) -> Option<InferredType> {
    if !ty.starts_with("mapping(") {
        return None;
    }
    let value = ty.rsplit_once("=>")?.1.trim().trim_end_matches(')').trim();
    Some(InferredType::parse(value))
}

fn infer_type(expr: &Expr, state: &PostprocessorState) -> InferredType {
    match expr {
        Expr::Identifier(name) => match name.as_str() {
            "msg.sender" | "tx.origin" | "address(this)" => InferredType::Address,
            "msg.value" | "block.timestamp" | "block.number" | "block.chainid" => {
                InferredType::Uint(256)
            }
            _ => state
                .memory_type_map
                .get(name)
                .or_else(|| state.storage_type_map.get(name))
                .map(|ty| InferredType::parse(ty))
                .unwrap_or(InferredType::Unknown),
        },
        Expr::Literal(_) => InferredType::Uint(256),
        Expr::Bool(_) => InferredType::Bool,
        Expr::StringLiteral(_) => InferredType::String,
        Expr::Cast { ty, .. } => InferredType::parse(ty),
        Expr::Unary { op: UnaryOp::LogicalNot, .. } => InferredType::Bool,
        Expr::Unary { .. } => InferredType::Uint(256),
        Expr::Binary { op, .. } => match op {
            BinaryOp::LogicalAnd |
            BinaryOp::Lt |
            BinaryOp::Le |
            BinaryOp::Gt |
            BinaryOp::Ge |
            BinaryOp::Eq |
            BinaryOp::Ne => InferredType::Bool,
            _ => InferredType::Uint(256),
        },
        Expr::Call { callee, .. } if callee == "address" => InferredType::Address,
        Expr::Call { callee, .. } if callee == "keccak256" => InferredType::Bytes(256),
        Expr::Keccak { .. } => InferredType::Bytes(256),
        Expr::Index { base, .. } => match &**base {
            Expr::Identifier(name) => state
                .storage_type_map
                .get(name)
                .and_then(|ty| mapping_value_type(ty))
                .unwrap_or(InferredType::Unknown),
            _ => InferredType::Unknown,
        },
        Expr::Member { base, member } if member == "balance" => {
            let _ = base;
            InferredType::Uint(256)
        }
        _ => InferredType::Unknown,
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
            let target = InferredType::parse(ty);
            if target != InferredType::Unknown && infer_type(value, state) == target {
                *expr = *value.clone();
            }
        }
        Expr::Call { callee, args }
            if callee == "address" &&
                args.len() == 1 &&
                infer_type(&args[0], state) == InferredType::Address =>
        {
            *expr = args.remove(0);
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
    if function.returns.as_deref() != Some("bool") {
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
        function.returns = Some("bool".to_string());
        function.statements =
            vec![Statement::Return(Expr::Literal(alloy::primitives::U256::from(1)))];
        normalize_typed_returns(&mut function, &mut PostprocessorState::default()).unwrap();
        assert_eq!(function.statements[0].render(RenderTarget::Solidity), "return true;");
    }

    #[test]
    fn removes_redundant_address_casts() {
        let mut statement = Statement::Return(Expr::Cast {
            ty: "address".to_string(),
            value: Box::new(Expr::Cast {
                ty: "address".to_string(),
                value: Box::new(Expr::identifier("arg0")),
            }),
        });
        let mut state = PostprocessorState::default();
        state.memory_type_map.insert("arg0".to_string(), "address".to_string());
        type_cleanup_postprocessor(&mut statement, &mut state).unwrap();
        assert_eq!(statement.render(RenderTarget::Solidity), "return arg0;");
    }

    #[test]
    fn removes_redundant_address_conversion() {
        let mut statement = Statement::Return(Expr::Call {
            callee: "address".to_string(),
            args: vec![Expr::identifier("msg.sender")],
        });
        type_cleanup_postprocessor(&mut statement, &mut PostprocessorState::default()).unwrap();
        assert_eq!(statement.render(RenderTarget::Solidity), "return msg.sender;");
    }

    #[test]
    fn keeps_narrowing_cast() {
        let mut statement = Statement::Return(Expr::Cast {
            ty: "uint8".to_string(),
            value: Box::new(Expr::identifier("arg0")),
        });
        let mut state = PostprocessorState::default();
        state.memory_type_map.insert("arg0".to_string(), "uint256".to_string());
        type_cleanup_postprocessor(&mut statement, &mut state).unwrap();
        assert_eq!(statement.render(RenderTarget::Solidity), "return uint8(arg0);");
    }
}
