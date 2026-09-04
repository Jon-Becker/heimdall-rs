use crate::{
    core::{ir::Statement, postprocess::PostprocessorState},
    Error,
};

/// Simplifies typed bitwise expressions before source rendering.
///
/// Low, byte-aligned masks are promoted to casts by [`Expr::simplify`]. Non-contiguous masks are
/// deliberately preserved because replacing them with casts changes their semantics.
pub(crate) fn bitwise_mask_postprocessor(
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
    fn converts_low_mask_to_cast() {
        let mut statement = Statement::Expression(Expr::Binary {
            op: BinaryOp::BitAnd,
            lhs: Box::new(Expr::identifier("arg0")),
            rhs: Box::new(Expr::Literal(U256::from(0xffff_u64))),
        });
        bitwise_mask_postprocessor(&mut statement, &mut PostprocessorState::default()).unwrap();
        assert_eq!(statement.render(RenderTarget::Solidity), "uint16(arg0);");
    }

    #[test]
    fn preserves_non_contiguous_mask() {
        let mut statement = Statement::Expression(Expr::Binary {
            op: BinaryOp::BitAnd,
            lhs: Box::new(Expr::identifier("arg0")),
            rhs: Box::new(Expr::Literal(U256::from(0xff00_u64))),
        });
        bitwise_mask_postprocessor(&mut statement, &mut PostprocessorState::default()).unwrap();
        assert_eq!(statement.render(RenderTarget::Solidity), "arg0 & 0xff00;");
    }
}
