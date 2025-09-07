use alloy::primitives::U256;

use crate::{
    ir::types::{BinOp, Block, Expr, Function, SolidityType, Stmt, Terminator},
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

            // Bitwise AND simplifications
            if matches!(op, BinOp::And) {
                // x & 0 = 0
                if is_zero(&right) || is_zero(&left) {
                    return Ok(Expr::Const(U256::ZERO));
                }
                // x & MAX = x
                if is_all_ones(&right) {
                    return Ok(left);
                }
                if is_all_ones(&left) {
                    return Ok(right);
                }
                // x & x = x
                if left == right {
                    return Ok(left);
                }

                // Check for mask patterns that can become casts
                if let Expr::Const(mask) = &right {
                    if let Some(cast_type) = detect_mask_cast(mask) {
                        return Ok(Expr::Cast(cast_type, Box::new(left)));
                    }
                }
            }

            // Bitwise OR simplifications
            if matches!(op, BinOp::Or) {
                // x | 0 = x
                if is_zero(&right) {
                    return Ok(left);
                }
                if is_zero(&left) {
                    return Ok(right);
                }
                // x | MAX = MAX
                if is_all_ones(&right) || is_all_ones(&left) {
                    return Ok(Expr::Const(U256::MAX));
                }
                // x | x = x
                if left == right {
                    return Ok(left);
                }
            }

            // Bitwise XOR simplifications
            if matches!(op, BinOp::Xor) {
                // x ^ 0 = x
                if is_zero(&right) {
                    return Ok(left);
                }
                if is_zero(&left) {
                    return Ok(right);
                }
                // x ^ x = 0
                if left == right {
                    return Ok(Expr::Const(U256::ZERO));
                }
                // x ^ MAX = ~x
                if is_all_ones(&right) {
                    return Ok(Expr::UnOp(crate::ir::types::UnOp::Not, Box::new(left)));
                }
                if is_all_ones(&left) {
                    return Ok(Expr::UnOp(crate::ir::types::UnOp::Not, Box::new(right)));
                }
            }

            // Shift simplifications
            if matches!(op, BinOp::Shl | BinOp::Shr | BinOp::Sar) {
                // x << 0 = x, x >> 0 = x
                if is_zero(&right) {
                    return Ok(left);
                }
                // 0 << x = 0, 0 >> x = 0
                if is_zero(&left) {
                    return Ok(Expr::Const(U256::ZERO));
                }
            }

            Ok(Expr::BinOp(op, Box::new(left), Box::new(right)))
        }
        Expr::Cast(ty, inner) => {
            let inner = simplify_expr(*inner)?;
            
            // Remove redundant casts
            if let Expr::Cast(inner_ty, inner_expr) = &inner {
                if ty == *inner_ty {
                    return Ok(inner.clone());
                }
            }
            
            Ok(Expr::Cast(ty, Box::new(inner)))
        }
        _ => Ok(expr),
    }
}

fn is_zero(expr: &Expr) -> bool {
    matches!(expr, Expr::Const(val) if val.is_zero())
}

fn is_all_ones(expr: &Expr) -> bool {
    matches!(expr, Expr::Const(val) if *val == U256::MAX)
}

fn detect_mask_cast(mask: &U256) -> Option<SolidityType> {
    // Check for address mask: 0x000000000000000000000000FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF
    let address_mask = U256::from_be_bytes([
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
        0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
    ]);
    if *mask == address_mask {
        return Some(SolidityType::Address);
    }

    // Check for bytes masks
    let mask_bytes = mask.to_be_bytes_vec();
    let bytes = mask_bytes.as_slice();
    
    // Count leading 0xFF bytes
    let mut ff_count = 0;
    for byte in bytes.iter() {
        if *byte == 0xFF {
            ff_count += 1;
        } else if *byte == 0 {
            // After FFs, should only be zeros
            break;
        } else {
            // Not a clean mask
            return None;
        }
    }

    // Check if remaining bytes are all zero
    for i in ff_count..32 {
        if bytes[i] != 0 {
            return None;
        }
    }

    if ff_count > 0 && ff_count <= 32 {
        return Some(SolidityType::Bytes(ff_count));
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_and_zero() {
        let expr = Expr::BinOp(
            BinOp::And,
            Box::new(Expr::Var("x".to_string())),
            Box::new(Expr::Const(U256::ZERO)),
        );
        let result = simplify_expr(expr).unwrap();
        assert_eq!(result, Expr::Const(U256::ZERO));
    }

    #[test]
    fn test_or_zero() {
        let expr = Expr::BinOp(
            BinOp::Or,
            Box::new(Expr::Var("x".to_string())),
            Box::new(Expr::Const(U256::ZERO)),
        );
        let result = simplify_expr(expr).unwrap();
        assert_eq!(result, Expr::Var("x".to_string()));
    }

    #[test]
    fn test_xor_self() {
        let expr = Expr::BinOp(
            BinOp::Xor,
            Box::new(Expr::Var("x".to_string())),
            Box::new(Expr::Var("x".to_string())),
        );
        let result = simplify_expr(expr).unwrap();
        assert_eq!(result, Expr::Const(U256::ZERO));
    }

    #[test]
    fn test_address_mask() {
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
        let result = simplify_expr(expr).unwrap();
        assert!(matches!(result, Expr::Cast(SolidityType::Address, _)));
    }
}