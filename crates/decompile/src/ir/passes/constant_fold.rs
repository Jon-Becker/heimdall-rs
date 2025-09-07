use alloy::primitives::U256;

use crate::{
    ir::types::{BinOp, Block, Expr, Function, Stmt, Terminator, UnOp},
    Error,
};

pub fn run(mut func: Function) -> Result<Function, Error> {
    for block in &mut func.blocks {
        fold_block(block)?;
    }
    Ok(func)
}

fn fold_block(block: &mut Block) -> Result<(), Error> {
    for stmt in &mut block.stmts {
        fold_stmt(stmt)?;
    }
    if let Some(term) = &mut block.terminator {
        fold_terminator(term)?;
    }
    Ok(())
}

fn fold_stmt(stmt: &mut Stmt) -> Result<(), Error> {
    match stmt {
        Stmt::Assign(_, expr) => {
            *expr = fold_expr(expr.clone())?;
        }
        Stmt::Store(_, slot, value) => {
            *slot = fold_expr(slot.clone())?;
            *value = fold_expr(value.clone())?;
        }
        Stmt::If(cond, then_block, else_block) => {
            *cond = fold_expr(cond.clone())?;
            fold_block(then_block)?;
            if let Some(else_b) = else_block {
                fold_block(else_b)?;
            }
        }
        Stmt::While(cond, body) => {
            *cond = fold_expr(cond.clone())?;
            fold_block(body)?;
        }
        Stmt::Return(exprs) | Stmt::Revert(exprs) => {
            for expr in exprs {
                *expr = fold_expr(expr.clone())?;
            }
        }
        Stmt::Log(_, topics) => {
            for topic in topics {
                *topic = fold_expr(topic.clone())?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn fold_terminator(term: &mut Terminator) -> Result<(), Error> {
    match term {
        Terminator::Return(exprs) | Terminator::Revert(exprs) => {
            for expr in exprs {
                *expr = fold_expr(expr.clone())?;
            }
        }
        Terminator::ConditionalJump(cond, _, _) => {
            *cond = fold_expr(cond.clone())?;
        }
        _ => {}
    }
    Ok(())
}

pub fn fold_expr(expr: Expr) -> Result<Expr, Error> {
    match expr {
        Expr::BinOp(op, left, right) => {
            let left = fold_expr(*left)?;
            let right = fold_expr(*right)?;

            // Try to fold constants
            if let (Expr::Const(l), Expr::Const(r)) = (&left, &right) {
                match op {
                    BinOp::Add => return Ok(Expr::Const(l.wrapping_add(*r))),
                    BinOp::Sub => return Ok(Expr::Const(l.wrapping_sub(*r))),
                    BinOp::Mul => return Ok(Expr::Const(l.wrapping_mul(*r))),
                    BinOp::Div => {
                        if !r.is_zero() {
                            return Ok(Expr::Const(l / r));
                        }
                    }
                    BinOp::Mod => {
                        if !r.is_zero() {
                            return Ok(Expr::Const(l % r));
                        }
                    }
                    BinOp::Exp => {
                        if let Ok(exp) = (*r).try_into() {
                            let exp: u32 = exp;
                            return Ok(Expr::Const(l.pow(U256::from(exp))));
                        }
                    }
                    BinOp::And => return Ok(Expr::Const(l & r)),
                    BinOp::Or => return Ok(Expr::Const(l | r)),
                    BinOp::Xor => return Ok(Expr::Const(l ^ r)),
                    BinOp::Shl => {
                        if let Ok(shift) = (*r).try_into() {
                            let shift: usize = shift;
                            if shift < 256 {
                                return Ok(Expr::Const(*l << shift));
                            }
                        }
                    }
                    BinOp::Shr => {
                        if let Ok(shift) = (*r).try_into() {
                            let shift: usize = shift;
                            if shift < 256 {
                                return Ok(Expr::Const(*l >> shift));
                            }
                        }
                    }
                    BinOp::Eq => {
                        return Ok(Expr::Const(if l == r { U256::from(1) } else { U256::ZERO }))
                    }
                    BinOp::Ne => {
                        return Ok(Expr::Const(if l != r { U256::from(1) } else { U256::ZERO }))
                    }
                    BinOp::Lt => {
                        return Ok(Expr::Const(if l < r { U256::from(1) } else { U256::ZERO }))
                    }
                    BinOp::Le => {
                        return Ok(Expr::Const(if l <= r { U256::from(1) } else { U256::ZERO }))
                    }
                    BinOp::Gt => {
                        return Ok(Expr::Const(if l > r { U256::from(1) } else { U256::ZERO }))
                    }
                    BinOp::Ge => {
                        return Ok(Expr::Const(if l >= r { U256::from(1) } else { U256::ZERO }))
                    }
                    _ => {}
                }
            }

            Ok(Expr::BinOp(op, Box::new(left), Box::new(right)))
        }
        Expr::UnOp(op, operand) => {
            let operand = fold_expr(*operand)?;

            if let Expr::Const(val) = &operand {
                match op {
                    UnOp::Not => return Ok(Expr::Const(!val)),
                    UnOp::IsZero => {
                        return Ok(Expr::Const(if val.is_zero() {
                            U256::from(1)
                        } else {
                            U256::ZERO
                        }))
                    }
                    UnOp::Neg => return Ok(Expr::Const(U256::ZERO.wrapping_sub(*val))),
                }
            }

            Ok(Expr::UnOp(op, Box::new(operand)))
        }
        Expr::Ternary(cond, then_expr, else_expr) => {
            let cond = fold_expr(*cond)?;
            let then_expr = fold_expr(*then_expr)?;
            let else_expr = fold_expr(*else_expr)?;

            if let Expr::Const(c) = &cond {
                if c.is_zero() {
                    return Ok(else_expr);
                } else {
                    return Ok(then_expr);
                }
            }

            Ok(Expr::Ternary(
                Box::new(cond),
                Box::new(then_expr),
                Box::new(else_expr),
            ))
        }
        Expr::Cast(ty, inner) => {
            let inner = fold_expr(*inner)?;
            Ok(Expr::Cast(ty, Box::new(inner)))
        }
        Expr::Load(ty, addr) => {
            let addr = fold_expr(*addr)?;
            Ok(Expr::Load(ty, Box::new(addr)))
        }
        _ => Ok(expr),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fold_add() {
        let expr = Expr::BinOp(
            BinOp::Add,
            Box::new(Expr::Const(U256::from(10))),
            Box::new(Expr::Const(U256::from(20))),
        );
        let result = fold_expr(expr).unwrap();
        assert_eq!(result, Expr::Const(U256::from(30)));
    }

    #[test]
    fn test_fold_nested() {
        let expr = Expr::BinOp(
            BinOp::Mul,
            Box::new(Expr::BinOp(
                BinOp::Add,
                Box::new(Expr::Const(U256::from(2))),
                Box::new(Expr::Const(U256::from(3))),
            )),
            Box::new(Expr::Const(U256::from(4))),
        );
        let result = fold_expr(expr).unwrap();
        assert_eq!(result, Expr::Const(U256::from(20)));
    }
}