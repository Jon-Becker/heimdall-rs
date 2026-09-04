use std::collections::HashSet;

use crate::{
    core::{
        ir::{Expr, Statement},
        postprocess::PostprocessorState,
    },
    interfaces::AnalyzedFunction,
    Error,
};

fn assignment(statement: &Statement) -> Option<(&str, &Expr)> {
    match statement {
        Statement::Assign { target: Expr::Identifier(name), value } |
        Statement::DeclareAssign { target: Expr::Identifier(name), value, .. }
            if name.starts_with("var_") =>
        {
            Some((name, value))
        }
        _ => None,
    }
}

fn is_trivial_alias(expr: &Expr) -> bool {
    matches!(expr, Expr::Identifier(_) | Expr::Literal(_) | Expr::Bool(_) | Expr::StringLiteral(_))
}

fn is_pure_inline_candidate(expr: &Expr) -> bool {
    match expr {
        Expr::Empty |
        Expr::Identifier(_) |
        Expr::Literal(_) |
        Expr::Bool(_) |
        Expr::StringLiteral(_) => true,
        Expr::Unary { value, .. } | Expr::Cast { value, .. } => is_pure_inline_candidate(value),
        Expr::Binary { lhs, rhs, .. } => {
            is_pure_inline_candidate(lhs) && is_pure_inline_candidate(rhs)
        }
        Expr::Member { base, .. } => is_pure_inline_candidate(base),
        Expr::Call { callee, args }
            if matches!(callee.as_str(), "address" | "blockhash" | "keccak256") =>
        {
            args.iter().all(is_pure_inline_candidate)
        }
        Expr::Raw(_) |
        Expr::Index { .. } |
        Expr::Slice { .. } |
        Expr::Keccak { .. } |
        Expr::StorageAccess(_) |
        Expr::Call { .. } => false,
    }
}

fn usage_count(statement: &Statement, variable: &str) -> usize {
    let mut count: usize = 0;
    let mut statement = statement.clone();
    statement.visit_exprs_mut(&mut |expr| {
        if matches!(expr, Expr::Identifier(name) if name == variable) {
            count += 1;
        }
    });

    if matches!(
        statement,
        Statement::Assign { target: Expr::Identifier(ref name), .. } |
        Statement::DeclareAssign { target: Expr::Identifier(ref name), .. }
            if name == variable
    ) {
        count = count.saturating_sub(1);
    }
    count
}

/// Inlines pure local values used exactly once before their next assignment.
pub(crate) fn inline_single_use_variables(
    function: &mut AnalyzedFunction,
    _: &mut PostprocessorState,
) -> Result<(), Error> {
    for assignment_idx in 0..function.statements.len() {
        let Some((variable, value)) = assignment(&function.statements[assignment_idx]) else {
            continue
        };
        let variable = variable.to_string();
        let value = value.clone();
        if !is_pure_inline_candidate(&value) {
            continue
        }
        let mut identifiers = HashSet::new();
        value.collect_identifiers(&mut identifiers);
        if identifiers.contains(&variable) {
            continue
        }

        let end = (assignment_idx + 1..function.statements.len())
            .find(|&idx| {
                assignment(&function.statements[idx]).is_some_and(|(name, _)| name == variable)
            })
            .unwrap_or(function.statements.len());
        let uses = function.statements[assignment_idx + 1..end]
            .iter()
            .map(|statement| usage_count(statement, &variable))
            .sum::<usize>();
        if uses == 0 || (uses != 1 && !is_trivial_alias(&value)) {
            continue
        }

        for statement in &mut function.statements[assignment_idx + 1..end] {
            statement.visit_exprs_mut(&mut |expr| {
                if matches!(expr, Expr::Identifier(name) if name == &variable) {
                    *expr = value.clone();
                }
            });
        }
        function.statements[assignment_idx] = Statement::Noop;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use alloy::primitives::U256;

    use super::*;
    use crate::core::ir::{RenderTarget, Statement};

    #[test]
    fn inlines_single_use_cast() {
        let mut function = AnalyzedFunction::new("00000000", false);
        function.statements = vec![
            Statement::DeclareAssign {
                ty: "address".to_string(),
                target: Expr::identifier("var_a"),
                value: Expr::Cast {
                    ty: "address".to_string(),
                    value: Box::new(Expr::identifier("arg0")),
                },
            },
            Statement::Return(Expr::index("storage_map_a", Expr::identifier("var_a"))),
        ];
        inline_single_use_variables(&mut function, &mut PostprocessorState::default()).unwrap();
        assert!(matches!(function.statements[0], Statement::Noop));
        assert_eq!(
            function.statements[1].render(RenderTarget::Solidity),
            "return storage_map_a[address(arg0)];"
        );
    }

    #[test]
    fn inlines_single_use_pure_builtin() {
        let mut function = AnalyzedFunction::new("00000000", false);
        function.statements = vec![
            Statement::DeclareAssign {
                ty: "bytes32".to_string(),
                target: Expr::identifier("var_a"),
                value: Expr::Call {
                    callee: "blockhash".to_string(),
                    args: vec![Expr::identifier("arg0")],
                },
            },
            Statement::Return(Expr::identifier("var_a")),
        ];
        inline_single_use_variables(&mut function, &mut PostprocessorState::default()).unwrap();
        assert_eq!(
            function.statements[1].render(RenderTarget::Solidity),
            "return blockhash(arg0);"
        );
    }

    #[test]
    fn does_not_inline_multiple_uses() {
        let mut function = AnalyzedFunction::new("00000000", false);
        function.statements = vec![
            Statement::Assign {
                target: Expr::identifier("var_a"),
                value: Expr::binary(
                    crate::core::ir::BinaryOp::Add,
                    Expr::identifier("arg0"),
                    Expr::Literal(U256::from(1)),
                ),
            },
            Statement::Return(Expr::binary(
                crate::core::ir::BinaryOp::Add,
                Expr::identifier("var_a"),
                Expr::identifier("var_a"),
            )),
        ];
        inline_single_use_variables(&mut function, &mut PostprocessorState::default()).unwrap();
        assert!(!matches!(function.statements[0], Statement::Noop));
    }
}
