use crate::{
    core::{
        ir::{Expr, Statement},
        postprocess::PostprocessorState,
    },
    Error,
};

/// Replaces complete expression subtrees with previously assigned variables.
pub(crate) fn variable_postprocessor(
    statement: &mut Statement,
    state: &mut PostprocessorState,
) -> Result<(), Error> {
    let assignment_target = match statement {
        Statement::Assign { target, .. } | Statement::DeclareAssign { target, .. } => {
            Some(target.clone())
        }
        _ => None,
    };

    statement.visit_exprs_mut(&mut |expr| {
        let replacement = state.variable_map.iter().find_map(|(variable, value)| {
            let is_trivial = matches!(
                value,
                Expr::Identifier(_) | Expr::Literal(_) | Expr::Bool(_) | Expr::StringLiteral(_)
            );
            (!is_trivial && value == expr && assignment_target.as_ref() != Some(variable))
                .then_some(variable)
        });
        if let Some(variable) = replacement {
            *expr = variable.clone();
        }
    });

    Ok(())
}

#[cfg(test)]
mod tests {
    use alloy::primitives::U256;

    use super::*;
    use crate::core::ir::{BinaryOp, RenderTarget};

    #[test]
    fn replaces_matching_expression_subtree() {
        let mut statement = Statement::Return(Expr::Binary {
            op: BinaryOp::Mul,
            lhs: Box::new(Expr::Binary {
                op: BinaryOp::Add,
                lhs: Box::new(Expr::identifier("arg0")),
                rhs: Box::new(Expr::Literal(U256::from(1))),
            }),
            rhs: Box::new(Expr::Literal(U256::from(2))),
        });
        let mut state = PostprocessorState::default();
        state.variable_map.insert(
            Expr::identifier("var_a"),
            Expr::Binary {
                op: BinaryOp::Add,
                lhs: Box::new(Expr::identifier("arg0")),
                rhs: Box::new(Expr::Literal(U256::from(1))),
            },
        );
        variable_postprocessor(&mut statement, &mut state).unwrap();
        assert_eq!(statement.render(RenderTarget::Solidity), "return var_a * 0x02;");
    }
}
