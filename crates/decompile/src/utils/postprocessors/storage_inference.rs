use alloy::primitives::U256;

use crate::{
    core::{
        ir::{BinaryOp, Expr, Statement, StoragePath},
        postprocess::PostprocessorState,
    },
    Error,
};

fn literal_usize(expr: &Expr) -> Option<usize> {
    match expr {
        Expr::Literal(value) => (*value).try_into().ok(),
        _ => None,
    }
}

fn resolve_keccak(expr: &mut Expr, memory: &hashbrown::HashMap<U256, Expr>) {
    let Expr::Keccak { offset, size, preimage } = expr else { return };
    let (Some(offset), Some(size)) = (literal_usize(offset), literal_usize(size)) else { return };
    if size == 0 || size > 512 || !size.is_multiple_of(32) {
        return;
    }

    let words = (0..size / 32)
        .map(|word| memory.get(&U256::from(offset + word * 32)).cloned())
        .collect::<Option<Vec<_>>>();
    if let Some(words) = words {
        *preimage = Some(words);
    }
}

fn path_from_keccak(expr: &Expr) -> Option<StoragePath> {
    let Expr::Keccak { preimage: Some(words), .. } = expr else { return None };
    match words.as_slice() {
        [root] => Some(StoragePath::DynamicArray {
            parent: Box::new(path_from_slot(root.clone())),
            index: Box::new(Expr::Literal(U256::ZERO)),
        }),
        [key, root] => Some(StoragePath::Mapping {
            parent: Box::new(path_from_slot(root.clone())),
            key: Box::new(key.clone()),
        }),
        _ => None,
    }
}

fn cast_width(ty: &str) -> Option<u16> {
    match ty {
        "address" => Some(160),
        "bool" => Some(8),
        _ => ty
            .strip_prefix("uint")
            .or_else(|| ty.strip_prefix("int"))
            .and_then(|width| width.parse().ok())
            .or_else(|| {
                ty.strip_prefix("bytes")
                    .and_then(|width| width.parse::<u16>().ok())
                    .map(|width| width * 8)
            }),
    }
}

fn packed_path(expr: &Expr) -> Option<StoragePath> {
    let Expr::Cast { ty, value } = expr else { return None };
    let width = cast_width(ty)?;
    if width >= 256 {
        return None;
    }

    match &**value {
        Expr::StorageAccess(path) => {
            Some(StoragePath::PackedField { parent: path.clone(), bit_offset: 0, bit_width: width })
        }
        Expr::Binary { op: BinaryOp::Shr, lhs, rhs } => {
            let Expr::StorageAccess(path) = &**lhs else { return None };
            let Expr::Literal(offset) = &**rhs else { return None };
            Some(StoragePath::PackedField {
                parent: path.clone(),
                bit_offset: (*offset).try_into().ok()?,
                bit_width: width,
            })
        }
        _ => None,
    }
}

fn path_from_slot(slot: Expr) -> StoragePath {
    if let Some(path) = path_from_keccak(&slot) {
        return path;
    }

    if let Expr::Binary { op: BinaryOp::Add, lhs, rhs } = &slot {
        for (hash, offset) in [(&**lhs, &**rhs), (&**rhs, &**lhs)] {
            let Expr::Keccak { preimage: Some(words), .. } = hash else { continue };
            match words.as_slice() {
                [root] => {
                    return StoragePath::DynamicArray {
                        parent: Box::new(path_from_slot(root.clone())),
                        index: Box::new(offset.clone()),
                    }
                }
                [key, root] => {
                    let parent = StoragePath::Mapping {
                        parent: Box::new(path_from_slot(root.clone())),
                        key: Box::new(key.clone()),
                    };
                    if let Expr::Literal(field_offset) = offset {
                        return StoragePath::Field {
                            parent: Box::new(parent),
                            offset: *field_offset,
                        };
                    }
                }
                _ => {}
            }
        }
    }

    StoragePath::Slot { slot: Box::new(slot) }
}

/// Resolves SHA3 memory preimages and classifies storage expressions into semantic paths.
pub(crate) fn storage_inference_postprocessor(
    statement: &mut Statement,
    state: &mut PostprocessorState,
) -> Result<(), Error> {
    statement.visit_exprs_mut(&mut |expr| {
        resolve_keccak(expr, &state.symbolic_memory);
        if let Expr::StorageAccess(path) = expr {
            if let StoragePath::Slot { slot } = &**path {
                *path = Box::new(path_from_slot(*slot.clone()));
            }
        }
        if let Some(path) = packed_path(expr) {
            *expr = Expr::StorageAccess(Box::new(path));
        }
    });

    if let Statement::Assign { target: Expr::Index { base, index }, value } = statement {
        if base.render() == "memory" {
            if let Expr::Literal(offset) = &**index {
                state.symbolic_memory.insert(*offset, value.clone());
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::ir::RenderTarget;

    fn memory_write(offset: u64, value: Expr) -> Statement {
        Statement::Assign {
            target: Expr::index("memory", Expr::Literal(U256::from(offset))),
            value,
        }
    }

    #[test]
    fn resolves_mapping_preimage() {
        let mut state = PostprocessorState::default();
        storage_inference_postprocessor(
            &mut memory_write(0, Expr::identifier("msg.sender")),
            &mut state,
        )
        .unwrap();
        storage_inference_postprocessor(
            &mut memory_write(32, Expr::Literal(U256::from(5))),
            &mut state,
        )
        .unwrap();

        let mut load = Statement::Return(Expr::StorageAccess(Box::new(StoragePath::Slot {
            slot: Box::new(Expr::Keccak {
                offset: Box::new(Expr::Literal(U256::ZERO)),
                size: Box::new(Expr::Literal(U256::from(64))),
                preimage: None,
            }),
        })));
        storage_inference_postprocessor(&mut load, &mut state).unwrap();
        assert_eq!(
            load.render(RenderTarget::Solidity),
            "return storage[keccak256(msg.sender, 0x05)];"
        );
        assert!(matches!(
            load,
            Statement::Return(Expr::StorageAccess(path))
                if matches!(*path, StoragePath::Mapping { .. })
        ));
    }

    #[test]
    fn recognizes_packed_field() {
        let mut statement = Statement::Return(Expr::Cast {
            ty: "address".to_string(),
            value: Box::new(Expr::Binary {
                op: BinaryOp::Shr,
                lhs: Box::new(Expr::StorageAccess(Box::new(StoragePath::Slot {
                    slot: Box::new(Expr::Literal(U256::from(3))),
                }))),
                rhs: Box::new(Expr::Literal(U256::from(32))),
            }),
        });
        storage_inference_postprocessor(&mut statement, &mut PostprocessorState::default())
            .unwrap();
        assert!(matches!(
            statement,
            Statement::Return(Expr::StorageAccess(path))
                if matches!(*path, StoragePath::PackedField {
                    bit_offset: 32,
                    bit_width: 160,
                    ..
                })
        ));
    }

    #[test]
    fn recognizes_dynamic_array_index() {
        let hash = Expr::Keccak {
            offset: Box::new(Expr::Literal(U256::ZERO)),
            size: Box::new(Expr::Literal(U256::from(32))),
            preimage: Some(vec![Expr::Literal(U256::from(7))]),
        };
        let path = path_from_slot(Expr::binary(BinaryOp::Add, hash, Expr::identifier("arg0")));
        assert!(matches!(path, StoragePath::DynamicArray { .. }));
    }
}
