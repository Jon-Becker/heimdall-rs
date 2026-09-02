use heimdall_common::utils::strings::base26_encode;

use crate::{
    core::{
        ir::{BinaryOp, Expr, Statement, StoragePath},
        postprocess::PostprocessorState,
    },
    Error,
};

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

fn same_layout(a: &StoragePath, b: &StoragePath) -> bool {
    match (a, b) {
        (StoragePath::Slot { slot: a }, StoragePath::Slot { slot: b }) => a == b,
        (StoragePath::Mapping { parent: a, .. }, StoragePath::Mapping { parent: b, .. }) |
        (
            StoragePath::DynamicArray { parent: a, .. },
            StoragePath::DynamicArray { parent: b, .. },
        ) => same_layout(a, b),
        (
            StoragePath::Field { parent: a, offset: a_offset },
            StoragePath::Field { parent: b, offset: b_offset },
        ) => a_offset == b_offset && same_layout(a, b),
        (
            StoragePath::PackedField { parent: a, bit_offset: a_offset, bit_width: a_width },
            StoragePath::PackedField { parent: b, bit_offset: b_offset, bit_width: b_width },
        ) => a_offset == b_offset && a_width == b_width && same_layout(a, b),
        _ => false,
    }
}

fn is_collection(path: &StoragePath) -> bool {
    match path {
        StoragePath::Slot { .. } => false,
        StoragePath::Mapping { .. } | StoragePath::DynamicArray { .. } => true,
        StoragePath::Field { parent, .. } | StoragePath::PackedField { parent, .. } => {
            is_collection(parent)
        }
    }
}

fn replacement_root(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Identifier(name) => Some(name.clone()),
        Expr::Index { base, .. } | Expr::Member { base, .. } => replacement_root(base),
        _ => None,
    }
}

fn render_path(path: &StoragePath, root: &str) -> Expr {
    match path {
        StoragePath::Slot { .. } => Expr::identifier(root),
        StoragePath::Mapping { parent, key } => {
            Expr::Index { base: Box::new(render_path(parent, root)), index: key.clone() }
        }
        StoragePath::DynamicArray { parent, index } => {
            Expr::Index { base: Box::new(render_path(parent, root)), index: index.clone() }
        }
        StoragePath::Field { parent, offset } => Expr::Member {
            base: Box::new(render_path(parent, root)),
            member: format!("field_{offset}"),
        },
        StoragePath::PackedField { parent, bit_offset, .. } => {
            if is_collection(parent) {
                Expr::Member {
                    base: Box::new(render_path(parent, root)),
                    member: format!("field_{bit_offset}"),
                }
            } else {
                Expr::identifier(root)
            }
        }
    }
}

fn storage_type(path: &StoragePath, leaf: String, state: &PostprocessorState) -> String {
    match path {
        StoragePath::Slot { .. } => leaf,
        StoragePath::Mapping { parent, key } => storage_type(
            parent,
            format!("mapping({} => {leaf})", expression_type(key, state)),
            state,
        ),
        StoragePath::DynamicArray { parent, .. } => {
            storage_type(parent, format!("{leaf}[]"), state)
        }
        // Struct synthesis will replace this placeholder in a subsequent layout pass.
        StoragePath::Field { parent, .. } => storage_type(parent, "bytes32".to_string(), state),
        StoragePath::PackedField { parent, bit_width, .. } => {
            let packed_type =
                if *bit_width == 160 { "address".to_string() } else { format!("uint{bit_width}") };
            storage_type(parent, packed_type, state)
        }
    }
}

/// Names semantic storage paths consistently and infers mapping/array declarations.
pub(crate) fn storage_postprocessor(
    statement: &mut Statement,
    state: &mut PostprocessorState,
) -> Result<(), Error> {
    let written_path = match statement {
        Statement::Assign { target: Expr::StorageAccess(path), .. } => Some((**path).clone()),
        _ => None,
    };

    let mut observed_paths = Vec::new();
    statement.visit_exprs_mut(&mut |expr| {
        let original = expr.clone();
        let Expr::StorageAccess(path) = expr else { return };
        let root = state
            .storage_map
            .iter()
            .find_map(|(known, replacement)| match known {
                Expr::StorageAccess(known_path) if same_layout(path, known_path) => {
                    replacement_root(replacement)
                }
                _ => None,
            })
            .unwrap_or_else(|| {
                let suffix = base26_encode(state.storage_map.len() + 1);
                if is_collection(path) {
                    format!("storage_map_{suffix}")
                } else {
                    format!("store_{suffix}")
                }
            });
        let replacement = render_path(path, &root);
        observed_paths.push((root, (**path).clone()));
        state.storage_map.insert(original, replacement.clone());
        *expr = replacement;
    });

    for (root, path) in observed_paths {
        let inferred = storage_type(&path, "bytes32".to_string(), state);
        state.storage_type_map.entry(root).or_insert(inferred);
    }

    let (target, value) = match statement {
        Statement::Assign { target, value } | Statement::DeclareAssign { target, value, .. } => {
            (target, value)
        }
        _ => return Ok(()),
    };
    let Some(root) = replacement_root(target) else { return Ok(()) };
    if !root.starts_with("store_") && !root.starts_with("storage_map_") {
        return Ok(())
    }

    state.variable_map.insert(target.clone(), value.clone());
    if let Some(path) = written_path {
        let ty = storage_type(&path, expression_type(value, state), state);
        state.storage_type_map.insert(root, ty);
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
            target: Expr::StorageAccess(Box::new(StoragePath::Slot {
                slot: Box::new(Expr::Literal(U256::ZERO)),
            })),
            value: Expr::identifier("arg0"),
        };
        let mut state = PostprocessorState::default();
        state.memory_type_map.insert("arg0".to_string(), "address".to_string());
        storage_postprocessor(&mut statement, &mut state).unwrap();
        assert_eq!(statement.render(RenderTarget::Solidity), "store_a = arg0;");
        assert_eq!(state.storage_type_map.get("store_a"), Some(&"address".to_string()));
    }

    #[test]
    fn reuses_mapping_name_across_keys() {
        let path = |key| StoragePath::Mapping {
            parent: Box::new(StoragePath::Slot { slot: Box::new(Expr::Literal(U256::from(5))) }),
            key: Box::new(Expr::identifier(key)),
        };
        let mut first = Statement::Return(Expr::StorageAccess(Box::new(path("arg0"))));
        let mut second = Statement::Return(Expr::StorageAccess(Box::new(path("arg1"))));
        let mut state = PostprocessorState::default();
        storage_postprocessor(&mut first, &mut state).unwrap();
        storage_postprocessor(&mut second, &mut state).unwrap();
        assert_eq!(first.render(RenderTarget::Solidity), "return storage_map_a[arg0];");
        assert_eq!(second.render(RenderTarget::Solidity), "return storage_map_a[arg1];");
    }
}
