use heimdall_common::utils::strings::base26_encode;

use crate::{
    core::{
        ir::{BinaryOp, Expr, Statement},
        postprocess::PostprocessorState,
    },
    Error,
};

fn is_storage_base(expr: &Expr) -> bool {
    matches!(expr, Expr::Raw(name) | Expr::Identifier(name) if name == "storage")
}

fn expression_type(expr: &Expr, state: &PostprocessorState) -> String {
    match expr {
        Expr::Cast { ty, .. } => ty.clone(),
        Expr::Identifier(name) => {
            state.memory_type_map.get(name).cloned().unwrap_or_else(|| "bytes32".to_string())
        }
        Expr::Binary { op, .. }
            if !matches!(op, BinaryOp::BitAnd | BinaryOp::BitOr | BinaryOp::BitXor) =>
        {
            "uint256".to_string()
        }
        Expr::Literal(_) => "uint256".to_string(),
        _ => "bytes32".to_string(),
    }
}

fn mapping_key(slot: &Expr) -> Option<Expr> {
    match slot {
        Expr::Call { callee, args } if callee == "keccak256" => args.first().cloned(),
        _ => None,
    }
}

/// Replaces storage accesses with stable state-variable references and infers their types.
pub(crate) fn storage_postprocessor(
    statement: &mut Statement,
    state: &mut PostprocessorState,
) -> Result<(), Error> {
    statement.visit_exprs_mut(&mut |expr| {
        let Expr::Index { base, index } = expr else { return };
        if !is_storage_base(base) {
            return;
        }

        let storage_loc = Expr::Index { base: base.clone(), index: index.clone() };
        let replacement = state.storage_map.get(&storage_loc).cloned().unwrap_or_else(|| {
            let suffix = base26_encode(state.storage_map.len() + 1);
            let replacement = if let Some(key) = mapping_key(index) {
                Expr::Index {
                    base: Box::new(Expr::identifier(format!("storage_map_{suffix}"))),
                    index: Box::new(key),
                }
            } else {
                Expr::identifier(format!("store_{suffix}"))
            };
            state.storage_map.insert(storage_loc, replacement.clone());
            replacement
        });
        *expr = replacement;
    });

    let (target, value) = match statement {
        Statement::Assign { target, value } | Statement::DeclareAssign { target, value, .. } => {
            (target, value)
        }
        _ => return Ok(()),
    };
    let root = match target {
        Expr::Identifier(name) => name.clone(),
        Expr::Index { base, .. } => match &**base {
            Expr::Identifier(name) => name.clone(),
            _ => return Ok(()),
        },
        _ => return Ok(()),
    };
    if !root.starts_with("store_") && !root.starts_with("storage_map_") {
        return Ok(())
    }

    state.variable_map.insert(target.clone(), value.clone());
    if root.starts_with("storage_map_") {
        let key_type = match target {
            Expr::Index { index, .. } => expression_type(index, state),
            _ => "bytes32".to_string(),
        };
        state
            .storage_type_map
            .insert(root, format!("mapping({key_type} => {})", expression_type(value, state)));
    } else {
        state.storage_type_map.insert(root, expression_type(value, state));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use alloy::primitives::U256;

    use super::*;
    use crate::core::ir::RenderTarget;

    #[test]
    fn names_storage_slot_and_infers_value_type() {
        let mut statement = Statement::Assign {
            target: Expr::index("storage", Expr::Literal(U256::ZERO)),
            value: Expr::identifier("arg0"),
        };
        let mut state = PostprocessorState::default();
        state.memory_type_map.insert("arg0".to_string(), "address".to_string());
        storage_postprocessor(&mut statement, &mut state).unwrap();
        assert_eq!(statement.render(RenderTarget::Solidity), "store_a = arg0;");
        assert_eq!(state.storage_type_map.get("store_a"), Some(&"address".to_string()));
    }

    #[test]
    fn recognizes_mapping_access() {
        let mut statement = Statement::Return(Expr::index(
            "storage",
            Expr::Call { callee: "keccak256".to_string(), args: vec![Expr::identifier("arg0")] },
        ));
        let mut state = PostprocessorState::default();
        storage_postprocessor(&mut statement, &mut state).unwrap();
        assert_eq!(statement.render(RenderTarget::Solidity), "return storage_map_a[arg0];");
    }
}
