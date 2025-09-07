use alloy::primitives::U256;

use crate::{
    ir::types::{BinOp, Block, Expr, Function, Stmt, Terminator, UnOp},
    Error,
};

pub fn run(mut func: Function) -> Result<Function, Error> {
    for block in &mut func.blocks {
        simplify_block(block)?;
    }
    Ok(func)
}

fn simplify_block(block: &mut Block) -> Result<(), Error> {
    for stmt in &mut block.stmts {
        simplify_stmt(stmt)?;
    }
    if let Some(term) = &mut block.terminator {
        simplify_terminator(term)?;
    }
    Ok(())
}

fn simplify_stmt(stmt: &mut Stmt) -> Result<(), Error> {
    match stmt {
        Stmt::Assign(_, expr) => {
            *expr = simplify_expr(expr.clone())?;
        }
        Stmt::Store(_, slot, value) => {
            *slot = simplify_expr(slot.clone())?;
            *value = simplify_expr(value.clone())?;
        }
        Stmt::If(cond, then_block, else_block) => {
            *cond = simplify_expr(cond.clone())?;
            simplify_block(then_block)?;
            if let Some(else_b) = else_block {
                simplify_block(else_b)?;
            }
        }
        Stmt::While(cond, body) => {
            *cond = simplify_expr(cond.clone())?;
            simplify_block(body)?;
        }
        Stmt::Return(exprs) | Stmt::Revert(exprs) => {
            for expr in exprs {
                *expr = simplify_expr(expr.clone())?;
            }
        }
        Stmt::Log(_, topics) => {
            for topic in topics {
                *topic = simplify_expr(topic.clone())?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn simplify_terminator(term: &mut Terminator) -> Result<(), Error> {
    match term {
        Terminator::Return(exprs) | Terminator::Revert(exprs) => {
            for expr in exprs {
                *expr = simplify_expr(expr.clone())?;
            }
        }
        Terminator::ConditionalJump(cond, _, _) => {
            *cond = simplify_expr(cond.clone())?;
        }
        _ => {}
    }
    Ok(())
}

pub fn simplify_expr(expr: Expr) -> Result<Expr, Error> {
    match expr {
        Expr::BinOp(op, left, right) => {
            let left = simplify_expr(*left)?;
            let right = simplify_expr(*right)?;

            // x + 0 = x
            if matches!(op, BinOp::Add) && is_zero(&right) {
                return Ok(left);
            }
            // 0 + x = x
            if matches!(op, BinOp::Add) && is_zero(&left) {
                return Ok(right);
            }

            // x - 0 = x
            if matches!(op, BinOp::Sub) && is_zero(&right) {
                return Ok(left);
            }
            // x - x = 0
            if matches!(op, BinOp::Sub) && left == right {
                return Ok(Expr::Const(U256::ZERO));
            }

            // x * 0 = 0
            if matches!(op, BinOp::Mul) && (is_zero(&left) || is_zero(&right)) {
                return Ok(Expr::Const(U256::ZERO));
            }
            // x * 1 = x
            if matches!(op, BinOp::Mul) && is_one(&right) {
                return Ok(left);
            }
            // 1 * x = x
            if matches!(op, BinOp::Mul) && is_one(&left) {
                return Ok(right);
            }

            // x / 1 = x
            if matches!(op, BinOp::Div) && is_one(&right) {
                return Ok(left);
            }
            // 0 / x = 0 (if x != 0)
            if matches!(op, BinOp::Div) && is_zero(&left) && !is_zero(&right) {
                return Ok(Expr::Const(U256::ZERO));
            }
            // x / x = 1 (if x != 0)
            if matches!(op, BinOp::Div) && left == right && !is_zero(&left) {
                return Ok(Expr::Const(U256::from(1)));
            }

            // x % 1 = 0
            if matches!(op, BinOp::Mod) && is_one(&right) {
                return Ok(Expr::Const(U256::ZERO));
            }
            // x % x = 0 (if x != 0)
            if matches!(op, BinOp::Mod) && left == right && !is_zero(&left) {
                return Ok(Expr::Const(U256::ZERO));
            }

            // x ** 0 = 1
            if matches!(op, BinOp::Exp) && is_zero(&right) {
                return Ok(Expr::Const(U256::from(1)));
            }
            // x ** 1 = x
            if matches!(op, BinOp::Exp) && is_one(&right) {
                return Ok(left);
            }
            // 0 ** x = 0 (if x != 0)
            if matches!(op, BinOp::Exp) && is_zero(&left) && !is_zero(&right) {
                return Ok(Expr::Const(U256::ZERO));
            }
            // 1 ** x = 1
            if matches!(op, BinOp::Exp) && is_one(&left) {
                return Ok(Expr::Const(U256::from(1)));
            }

            Ok(Expr::BinOp(op, Box::new(left), Box::new(right)))
        }
        Expr::UnOp(op, operand) => {
            let operand = simplify_expr(*operand)?;

            // Double negation: !!x = x
            if matches!(op, UnOp::IsZero) {
                if let Expr::UnOp(UnOp::IsZero, inner) = &operand {
                    // !!x = x for boolean context
                    // But need to ensure x is 0 or 1
                    // For now, don't simplify to be safe
                }
            }

            // Not of constant
            if matches!(op, UnOp::Not) && is_all_ones(&operand) {
                return Ok(Expr::Const(U256::ZERO));
            }
            if matches!(op, UnOp::Not) && is_zero(&operand) {
                return Ok(Expr::Const(U256::MAX));
            }

            Ok(Expr::UnOp(op, Box::new(operand)))
        }
        Expr::Ternary(cond, then_expr, else_expr) => {
            let cond = simplify_expr(*cond)?;
            let then_expr = simplify_expr(*then_expr)?;
            let else_expr = simplify_expr(*else_expr)?;

            // If both branches are the same, return the expression
            if then_expr == else_expr {
                return Ok(then_expr);
            }

            Ok(Expr::Ternary(
                Box::new(cond),
                Box::new(then_expr),
                Box::new(else_expr),
            ))
        }
        Expr::Cast(ty, inner) => {
            let inner = simplify_expr(*inner)?;
            // TODO: Simplify redundant casts
            Ok(Expr::Cast(ty, Box::new(inner)))
        }
        Expr::Load(ty, addr) => {
            let addr = simplify_expr(*addr)?;
            Ok(Expr::Load(ty, Box::new(addr)))
        }
        _ => Ok(expr),
    }
}

fn is_zero(expr: &Expr) -> bool {
    matches!(expr, Expr::Const(val) if val.is_zero())
}

fn is_one(expr: &Expr) -> bool {
    matches!(expr, Expr::Const(val) if *val == U256::from(1))
}

fn is_all_ones(expr: &Expr) -> bool {
    matches!(expr, Expr::Const(val) if *val == U256::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simplify_add_zero() {
        let expr = Expr::BinOp(
            BinOp::Add,
            Box::new(Expr::Var("x".to_string())),
            Box::new(Expr::Const(U256::ZERO)),
        );
        let result = simplify_expr(expr).unwrap();
        assert_eq!(result, Expr::Var("x".to_string()));
    }

    #[test]
    fn test_simplify_mul_one() {
        let expr = Expr::BinOp(
            BinOp::Mul,
            Box::new(Expr::Var("x".to_string())),
            Box::new(Expr::Const(U256::from(1))),
        );
        let result = simplify_expr(expr).unwrap();
        assert_eq!(result, Expr::Var("x".to_string()));
    }

    #[test]
    fn test_simplify_sub_self() {
        let expr = Expr::BinOp(
            BinOp::Sub,
            Box::new(Expr::Var("x".to_string())),
            Box::new(Expr::Var("x".to_string())),
        );
        let result = simplify_expr(expr).unwrap();
        assert_eq!(result, Expr::Const(U256::ZERO));
    }
}