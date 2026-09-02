use hashbrown::HashMap;

use crate::{
    core::{
        ir::{Expr, Statement, StoragePath},
        postprocess::PostprocessorState,
    },
    interfaces::AnalyzedFunction,
    Error,
};

#[derive(Clone, Copy, Debug, Default)]
struct StringStorageEvidence {
    direct: bool,
    dynamic: bool,
    marker: bool,
}

fn classify_path(path: &StoragePath, evidence: &mut StringStorageEvidence) {
    match path {
        StoragePath::Slot { .. } => evidence.direct = true,
        StoragePath::Mapping { parent, .. } | StoragePath::Field { parent, .. } => {
            classify_path(parent, evidence);
        }
        StoragePath::DynamicArray { parent, .. } => {
            evidence.dynamic = true;
            classify_path(parent, evidence);
        }
        StoragePath::PackedField { parent, bit_offset, bit_width } => {
            if *bit_offset == 0 && *bit_width == 8 {
                evidence.marker = true;
            }
            classify_path(parent, evidence);
        }
    }
}

fn storage_root(expr: &Expr) -> Option<Expr> {
    let mut root = None;
    let mut expr = expr.clone();
    expr.visit_mut(&mut |expr| {
        if let Expr::StorageAccess(path) = expr {
            root = Some(path.root().clone());
        }
    });
    root
}

fn marker_root(expr: &Expr) -> Option<Expr> {
    let Expr::Binary { op: crate::core::ir::BinaryOp::BitAnd, lhs, rhs } = expr else {
        return None
    };
    match (&**lhs, &**rhs) {
        (Expr::Literal(value), subject) | (subject, Expr::Literal(value))
            if *value == alloy::primitives::U256::from(1) =>
        {
            storage_root(subject)
        }
        _ => None,
    }
}

fn has_side_effects(statement: &Statement) -> bool {
    match statement {
        Statement::Assign { target: Expr::StorageAccess(_), .. } |
        Statement::ExternalCall { .. } |
        Statement::Emit { .. } |
        Statement::AssemblyAssign { .. } => true,
        Statement::Assign { target: Expr::Index { base, .. }, .. }
            if base.render() == "transient" =>
        {
            true
        }
        Statement::Expression(Expr::Call { callee, .. }) if callee == "selfdestruct" => true,
        Statement::IfElse { then_body, else_body, .. } => {
            then_body.iter().chain(else_body).any(has_side_effects)
        }
        _ => false,
    }
}

/// Associates compiler-generated storage string/bytes decoders with their canonical root.
///
/// Solidity's decoder reads the packed root marker and, for long values, words rooted at
/// `keccak256(slot)`. Requiring both forms avoids classifying arbitrary one-slot view functions as
/// generated getters.
pub(crate) fn detect_string_storage_getter(
    function: &AnalyzedFunction,
    state: &mut PostprocessorState,
) -> Result<(), Error> {
    let string_like = matches!(
        function.returns.as_deref(),
        Some(returns) if returns.starts_with("string") ||
            (returns.starts_with("bytes") && returns != "bytes32")
    );
    if !string_like ||
        !function.arguments.is_empty() ||
        !function.view ||
        function.statements.iter().any(has_side_effects)
    {
        return Ok(())
    }

    let mut roots: HashMap<Expr, StringStorageEvidence> = HashMap::new();
    let mut abi_encoded_return = false;
    for statement in &function.statements {
        let mut statement = statement.clone();
        statement.visit_exprs_mut(&mut |expr| {
            if let Expr::StorageAccess(path) = expr {
                classify_path(path, roots.entry(path.root().clone()).or_default());
            }
            if let Some(root) = marker_root(expr) {
                roots.entry(root).or_default().marker = true;
            }
            if matches!(expr, Expr::Call { callee, .. } if callee == "abi.encodePacked") {
                abi_encoded_return = true;
            }
        });
    }

    if roots.len() == 1 {
        let (root, evidence) = roots.into_iter().next().expect("checked one root");
        if evidence.direct && evidence.marker && (evidence.dynamic || abi_encoded_return) {
            state.maybe_getter_for = Some(root);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use alloy::primitives::U256;

    use super::*;

    #[test]
    fn recognizes_solidity_string_decoder_evidence() {
        let root = Expr::Literal(U256::from(2));
        let mut function = AnalyzedFunction::new("06fdde03", false);
        function.view = true;
        function.returns = Some("string memory".to_string());
        function.statements = vec![
            Statement::Expression(Expr::StorageAccess(Box::new(StoragePath::PackedField {
                parent: Box::new(StoragePath::Slot { slot: Box::new(root.clone()) }),
                bit_offset: 0,
                bit_width: 8,
            }))),
            Statement::Return(Expr::StorageAccess(Box::new(StoragePath::DynamicArray {
                parent: Box::new(StoragePath::Slot { slot: Box::new(root.clone()) }),
                index: Box::new(Expr::identifier("index")),
            }))),
        ];
        let mut state = PostprocessorState::default();
        detect_string_storage_getter(&function, &mut state).unwrap();
        assert_eq!(state.maybe_getter_for, Some(root));
    }

    #[test]
    fn rejects_computed_string_view() {
        let mut function = AnalyzedFunction::new("00000000", false);
        function.view = true;
        function.returns = Some("string memory".to_string());
        function.statements = vec![Statement::Return(Expr::StringLiteral("constant".to_string()))];
        let mut state = PostprocessorState::default();
        detect_string_storage_getter(&function, &mut state).unwrap();
        assert!(state.maybe_getter_for.is_none());
    }
}
