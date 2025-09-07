use alloy::primitives::U256;

use crate::{
    ir::types::{BinOp, Block, Expr, Function, Stmt, Terminator},
    Error,
};

pub fn run(mut func: Function) -> Result<Function, Error> {
    for block in &mut func.blocks {
        reduce_block(block)?;
    }
    Ok(func)
}

fn reduce_block(block: &mut Block) -> Result<(), Error> {
    for stmt in &mut block.stmts {
        reduce_stmt(stmt)?;
    }
    if let Some(term) = &mut block.terminator {
        reduce_terminator(term)?;
    }
    Ok(())
}

fn reduce_stmt(stmt: &mut Stmt) -> Result<(), Error> {
    match stmt {
        Stmt::Assign(_, expr) => {
            *expr = reduce_expr(expr.clone())?;
        }
        Stmt::Store(_, slot, value) => {
            *slot = reduce_expr(slot.clone())?;
            *value = reduce_expr(value.clone())?;
        }
        Stmt::If(cond, then_block, else_block) => {
            *cond = reduce_expr(cond.clone())?;
            reduce_block(then_block)?;
            if let Some(else_b) = else_block {
                reduce_block(else_b)?;
            }
        }
        Stmt::While(cond, body) => {
            *cond = reduce_expr(cond.clone())?;
            reduce_block(body)?;
        }
        Stmt::Return(exprs) | Stmt::Revert(exprs) => {
            for expr in exprs {
                *expr = reduce_expr(expr.clone())?;
            }
        }
        Stmt::Log(_, topics) => {
            for topic in topics {
                *topic = reduce_expr(topic.clone())?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn reduce_terminator(term: &mut Terminator) -> Result<(), Error> {
    match term {
        Terminator::Return(exprs) | Terminator::Revert(exprs) => {
            for expr in exprs {
                *expr = reduce_expr(expr.clone())?;
            }
        }
        Terminator::ConditionalJump(cond, _, _) => {
            *cond = reduce_expr(cond.clone())?;
        }
        _ => {}
    }
    Ok(())
}

pub fn reduce_expr(expr: Expr) -> Result<Expr, Error> {
    match expr {
        Expr::BinOp(op, left, right) => {
            let left = reduce_expr(*left)?;
            let right = reduce_expr(*right)?;

            // Multiplication by power of 2 -> left shift
            if matches!(op, BinOp::Mul) {
                if let Expr::Const(val) = &right {
                    if let Some(shift) = is_power_of_two(val) {
                        return Ok(Expr::BinOp(
                            BinOp::Shl,
                            Box::new(left),
                            Box::new(Expr::Const(U256::from(shift))),
                        ));
                    }
                }
                if let Expr::Const(val) = &left {
                    if let Some(shift) = is_power_of_two(val) {
                        return Ok(Expr::BinOp(
                            BinOp::Shl,
                            Box::new(right),
                            Box::new(Expr::Const(U256::from(shift))),
                        ));
                    }
                }
            }

            // Division by power of 2 -> right shift
            if matches!(op, BinOp::Div) {
                if let Expr::Const(val) = &right {
                    if let Some(shift) = is_power_of_two(val) {
                        return Ok(Expr::BinOp(
                            BinOp::Shr,
                            Box::new(left),
                            Box::new(Expr::Const(U256::from(shift))),
                        ));
                    }
                }
            }

            // Modulo by power of 2 -> bitwise AND with (value - 1)
            if matches!(op, BinOp::Mod) {
                if let Expr::Const(val) = &right {
                    if let Some(shift) = is_power_of_two(val) {
                        let mask = val - U256::from(1);
                        return Ok(Expr::BinOp(
                            BinOp::And,
                            Box::new(left),
                            Box::new(Expr::Const(mask)),
                        ));
                    }
                }
            }

            // x * -1 -> -x
            if matches!(op, BinOp::Mul) {
                if is_minus_one(&right) {
                    return Ok(Expr::UnOp(crate::ir::types::UnOp::Neg, Box::new(left)));
                }
                if is_minus_one(&left) {
                    return Ok(Expr::UnOp(crate::ir::types::UnOp::Neg, Box::new(right)));
                }
            }

            Ok(Expr::BinOp(op, Box::new(left), Box::new(right)))
        }
        Expr::UnOp(op, operand) => {
            let operand = reduce_expr(*operand)?;
            Ok(Expr::UnOp(op, Box::new(operand)))
        }
        Expr::Ternary(cond, then_expr, else_expr) => {
            let cond = reduce_expr(*cond)?;
            let then_expr = reduce_expr(*then_expr)?;
            let else_expr = reduce_expr(*else_expr)?;
            Ok(Expr::Ternary(
                Box::new(cond),
                Box::new(then_expr),
                Box::new(else_expr),
            ))
        }
        Expr::Cast(ty, inner) => {
            let inner = reduce_expr(*inner)?;
            Ok(Expr::Cast(ty, Box::new(inner)))
        }
        Expr::Load(ty, addr) => {
            let addr = reduce_expr(*addr)?;
            Ok(Expr::Load(ty, Box::new(addr)))
        }
        _ => Ok(expr),
    }
}

fn is_power_of_two(val: &U256) -> Option<u32> {
    if val.is_zero() {
        return None;
    }
    
    // Check if only one bit is set
    let minus_one = val - U256::from(1);
    if (val & minus_one) != U256::ZERO {
        return None;
    }
    
    // Count trailing zeros to find the power
    let mut shift = 0;
    let mut temp = *val;
    while temp > U256::from(1) {
        temp = temp >> 1;
        shift += 1;
    }
    
    Some(shift)
}

fn is_minus_one(expr: &Expr) -> bool {
    matches!(expr, Expr::Const(val) if *val == U256::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mul_power_of_two() {
        let expr = Expr::BinOp(
            BinOp::Mul,
            Box::new(Expr::Var("x".to_string())),
            Box::new(Expr::Const(U256::from(8))),
        );
        let result = reduce_expr(expr).unwrap();
        assert!(matches!(result, Expr::BinOp(BinOp::Shl, _, _)));
    }

    #[test]
    fn test_div_power_of_two() {
        let expr = Expr::BinOp(
            BinOp::Div,
            Box::new(Expr::Var("x".to_string())),
            Box::new(Expr::Const(U256::from(16))),
        );
        let result = reduce_expr(expr).unwrap();
        assert!(matches!(result, Expr::BinOp(BinOp::Shr, _, _)));
    }

    #[test]
    fn test_mod_power_of_two() {
        let expr = Expr::BinOp(
            BinOp::Mod,
            Box::new(Expr::Var("x".to_string())),
            Box::new(Expr::Const(U256::from(256))),
        );
        let result = reduce_expr(expr).unwrap();
        assert!(matches!(result, Expr::BinOp(BinOp::And, _, _)));
    }
}