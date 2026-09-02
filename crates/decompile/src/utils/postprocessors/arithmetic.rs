use crate::{
    core::{ir::Statement, postprocess::PostprocessorState},
    Error,
};

/// Simplifies typed arithmetic expressions before source rendering.
///
/// Parentheses are emitted from operator precedence by the renderer, so this pass no longer needs
/// to parse and remove parentheses from generated source text.
pub(crate) fn arithmetic_postprocessor(
    statement: &mut Statement,
    _: &mut PostprocessorState,
) -> Result<(), Error> {
    statement.visit_exprs_mut(&mut |expr| {
        *expr = expr.clone().simplify();
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use alloy::primitives::U256;

    use super::*;
    use crate::core::ir::{BinaryOp, Expr, RenderTarget};

    #[test]
    fn removes_additive_identity() {
        let mut statement = Statement::Return(Expr::Binary {
            op: BinaryOp::Add,
            lhs: Box::new(Expr::identifier("arg0")),
            rhs: Box::new(Expr::Literal(U256::ZERO)),
        });
        arithmetic_postprocessor(&mut statement, &mut PostprocessorState::default()).unwrap();
        assert_eq!(statement.render(RenderTarget::Solidity), "return arg0;");
    }

    #[test]
    fn retains_required_parentheses() {
        let mut statement = Statement::Return(Expr::Binary {
            op: BinaryOp::Mul,
            lhs: Box::new(Expr::Binary {
                op: BinaryOp::Add,
                lhs: Box::new(Expr::identifier("a")),
                rhs: Box::new(Expr::identifier("b")),
            }),
            rhs: Box::new(Expr::identifier("c")),
        });
        arithmetic_postprocessor(&mut statement, &mut PostprocessorState::default()).unwrap();
        assert_eq!(statement.render(RenderTarget::Solidity), "return (a + b) * c;");
    }
}
