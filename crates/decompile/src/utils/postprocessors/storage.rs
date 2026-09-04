use alloy::primitives::U256;
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
            if matches!(name.as_str(), "msg.sender" | "tx.origin" | "address(this)") {
                "address".to_string()
            } else {
                state.memory_type_map.get(name).cloned().unwrap_or_else(|| "bytes32".to_string())
            }
        }
        Expr::Binary { op, .. }
            if !matches!(op, BinaryOp::BitAnd | BinaryOp::BitOr | BinaryOp::BitXor) =>
        {
            "uint256".to_string()
        }
        Expr::Bool(_) => "bool".to_string(),
        Expr::Literal(_) => "uint256".to_string(),
        Expr::Keccak { .. } => "bytes32".to_string(),
        Expr::Call { callee, .. } if callee == "address" => "address".to_string(),
        _ => "bytes32".to_string(),
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

fn naming_key(path: &StoragePath) -> Expr {
    match path {
        StoragePath::PackedField { parent, bit_offset, bit_width } if !is_collection(parent) => {
            Expr::Call {
                callee: "packed_slot".to_string(),
                args: vec![
                    parent.root().clone(),
                    Expr::Literal(U256::from(*bit_offset as u64)),
                    Expr::Literal(U256::from(*bit_width as u64)),
                ],
            }
        }
        _ => path.root().clone(),
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

fn storage_type(
    path: &StoragePath,
    leaf: String,
    state: &PostprocessorState,
    collapse_dynamic: bool,
) -> String {
    match path {
        StoragePath::Slot { .. } => leaf,
        StoragePath::Mapping { parent, key } => storage_type(
            parent,
            format!("mapping({} => {leaf})", expression_type(key, state)),
            state,
            collapse_dynamic,
        ),
        StoragePath::DynamicArray { parent, .. } => storage_type(
            parent,
            if collapse_dynamic { leaf } else { format!("{leaf}[]") },
            state,
            collapse_dynamic,
        ),
        // Struct synthesis will replace this placeholder in a subsequent layout pass.
        StoragePath::Field { parent, .. } => {
            storage_type(parent, "bytes32".to_string(), state, collapse_dynamic)
        }
        StoragePath::PackedField { parent, bit_width, .. } => {
            let packed_type =
                if *bit_width == 160 { "address".to_string() } else { format!("uint{bit_width}") };
            storage_type(parent, packed_type, state, collapse_dynamic)
        }
    }
}

/// Names semantic storage paths consistently and infers mapping/array declarations.
pub(crate) fn storage_postprocessor(
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

    let written_path = match statement {
        Statement::Assign { target: Expr::StorageAccess(path), .. } => Some((**path).clone()),
        _ => None,
    };

    let mut observed_paths = Vec::new();
    statement.visit_exprs_mut(&mut |expr| {
        let original = expr.clone();
        let Expr::StorageAccess(path) = expr else { return };
        let key = if state.storage_type_hints.contains_key(path.root()) {
            path.root().clone()
        } else {
            naming_key(path)
        };
        let root = state.storage_roots.get(&key).cloned().unwrap_or_else(|| {
            let suffix = base26_encode(state.storage_roots.len() + 1);
            let root = if is_collection(path) {
                format!("storage_map_{suffix}")
            } else {
                format!("store_{suffix}")
            };
            state.storage_roots.insert(key, root.clone());
            root
        });
        state.storage_root_slots.entry(root.clone()).or_insert_with(|| path.root().clone());
        let replacement = render_path(path, &root);
        observed_paths.push((root, (**path).clone()));
        state.storage_map.insert(original, replacement.clone());
        *expr = replacement;
    });

    for (root, path) in observed_paths {
        let hint = state.storage_type_hints.get(path.root()).cloned();
        let inferred = storage_type(
            &path,
            hint.clone().unwrap_or_else(|| "bytes32".to_string()),
            state,
            hint.is_some(),
        );
        let existing = state.storage_type_map.get(&root);
        if hint.is_some() || existing.is_none() || existing.is_some_and(|ty| ty == "bytes32") {
            state.storage_type_map.insert(root, inferred);
        }
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

    // Only record the assignment for future substitution if it is at the top level.
    if state.conditional_depth == 0 {
        state.variable_map.insert(target.clone(), value.clone());
    }
    if let Some(path) = written_path {
        let hint = state.storage_type_hints.get(path.root()).cloned();
        let ty = storage_type(
            &path,
            hint.clone().unwrap_or_else(|| expression_type(value, state)),
            state,
            hint.is_some(),
        );
        state.storage_type_map.insert(root, ty);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
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
    fn infers_nested_mapping_type() {
        let path = StoragePath::Mapping {
            parent: Box::new(StoragePath::Mapping {
                parent: Box::new(StoragePath::Slot {
                    slot: Box::new(Expr::Literal(U256::from(5))),
                }),
                key: Box::new(Expr::identifier("arg0")),
            }),
            key: Box::new(Expr::identifier("arg1")),
        };
        let mut statement = Statement::Assign {
            target: Expr::StorageAccess(Box::new(path)),
            value: Expr::Literal(U256::from(1)),
        };
        let mut state = PostprocessorState::default();
        state.memory_type_map.insert("arg0".to_string(), "address".to_string());
        state.memory_type_map.insert("arg1".to_string(), "address".to_string());
        storage_postprocessor(&mut statement, &mut state).unwrap();
        assert_eq!(
            state.storage_type_map.get("storage_map_a"),
            Some(&"mapping(address => mapping(address => uint256))".to_string())
        );
    }

    #[test]
    fn unifies_direct_and_dynamic_views_of_string_root() {
        let root = Expr::Literal(U256::from(2));
        let mut direct = Statement::Return(Expr::StorageAccess(Box::new(StoragePath::Slot {
            slot: Box::new(root.clone()),
        })));
        let mut dynamic =
            Statement::Return(Expr::StorageAccess(Box::new(StoragePath::DynamicArray {
                parent: Box::new(StoragePath::Slot { slot: Box::new(root.clone()) }),
                index: Box::new(Expr::identifier("arg0")),
            })));
        let mut state = PostprocessorState::default();
        state.storage_type_hints.insert(root, "string".to_string());
        storage_postprocessor(&mut direct, &mut state).unwrap();
        storage_postprocessor(&mut dynamic, &mut state).unwrap();
        assert_eq!(state.storage_roots.len(), 1);
        assert_eq!(direct.render(RenderTarget::Solidity), "return store_a;");
        assert_eq!(dynamic.render(RenderTarget::Solidity), "return store_a[arg0];");
        assert_eq!(state.storage_type_map.get("store_a"), Some(&"string".to_string()));
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
