use heimdall_common::utils::strings::base26_encode;

use crate::{
    core::{
        ir::{BinaryOp, Expr, Statement},
        postprocess::PostprocessorState,
    },
    Error,
};

fn is_memory_base(expr: &Expr) -> bool {
    matches!(expr, Expr::Raw(name) | Expr::Identifier(name) if name == "memory")
}

fn infer_type(expr: &Expr, state: &PostprocessorState) -> Option<String> {
    match expr {
        Expr::Cast { ty, .. } => Some(ty.clone()),
        Expr::Identifier(name) => state.memory_type_map.get(name).cloned(),
        Expr::Binary { op, .. } => Some(
            if matches!(
                op,
                BinaryOp::BitAnd |
                    BinaryOp::BitOr |
                    BinaryOp::BitXor |
                    BinaryOp::Shl |
                    BinaryOp::Shr
            ) {
                "bytes32"
            } else {
                "uint256"
            }
            .to_string(),
        ),
        Expr::Literal(_) => Some("uint256".to_string()),
        Expr::Unary { .. } => Some("bytes32".to_string()),
        Expr::Call { args, .. } => args.iter().find_map(|arg| infer_type(arg, state)),
        Expr::Index { base, index } => infer_type(base, state).or_else(|| infer_type(index, state)),
        Expr::Slice { base, start, end } => infer_type(base, state)
            .or_else(|| infer_type(start, state))
            .or_else(|| infer_type(end, state)),
        Expr::Member { base, .. } => infer_type(base, state),
        _ => None,
    }
}

/// Replaces memory accesses with stable local variables and records assignment/type information.
pub(crate) fn memory_postprocessor(
    statement: &mut Statement,
    state: &mut PostprocessorState,
) -> Result<(), Error> {
    // Track conditional nesting depth so we don't record block-local variables.
    match statement {
        Statement::If { .. } => state.conditional_depth += 1,
        Statement::Else | Statement::CloseBlock => {
            state.conditional_depth = state.conditional_depth.saturating_sub(1);
        }
        _ => {}
    }

    statement.visit_exprs_mut(&mut |expr| {
        let Expr::Index { base, .. } = expr else { return };
        if !is_memory_base(base) {
            return;
        }

        let memory_loc = expr.clone();
        let variable = state.memory_map.get(&memory_loc).cloned().unwrap_or_else(|| {
            let variable =
                Expr::identifier(format!("var_{}", base26_encode(state.memory_map.len() + 1)));
            state.memory_map.insert(memory_loc, variable.clone());
            variable
        });
        *expr = variable;
    });

    let Statement::Assign { target, value } = statement else { return Ok(()) };
    let Expr::Identifier(var_name) = target else { return Ok(()) };
    if !var_name.starts_with("var_") {
        return Ok(())
    }
    let var_name = var_name.clone();

    // Only record the assignment for future substitution if it is at the top level.
    if state.conditional_depth == 0 {
        state.variable_map.insert(Expr::identifier(&var_name), value.clone());
    }
    if let Some(ty) = infer_type(value, state) {
        state.memory_type_map.entry(var_name.clone()).or_insert_with(|| ty.clone());
        *statement = Statement::DeclareAssign {
            ty,
            target: Expr::identifier(var_name),
            value: value.clone(),
        };
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use alloy::primitives::U256;

    use super::*;
    use crate::core::ir::RenderTarget;

    #[test]
    fn promotes_memory_assignment_to_typed_variable() {
        let mut statement = Statement::Assign {
            target: Expr::index("memory", Expr::Literal(U256::from(32))),
            value: Expr::Binary {
                op: BinaryOp::Add,
                lhs: Box::new(Expr::identifier("arg0")),
                rhs: Box::new(Expr::Literal(U256::from(1))),
            },
        };
        let mut state = PostprocessorState::default();
        memory_postprocessor(&mut statement, &mut state).unwrap();
        assert_eq!(statement.render(RenderTarget::Solidity), "uint256 var_a = arg0 + 0x01;");
        assert_eq!(
            state.memory_map.get(&Expr::index("memory", Expr::Literal(U256::from(32)))),
            Some(&Expr::identifier("var_a"))
        );
    }
}
