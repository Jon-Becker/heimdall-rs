use std::collections::{HashMap, HashSet};

use crate::{
    core::{
        ir::{Expr, Statement},
        postprocess::PostprocessorState,
    },
    interfaces::AnalyzedFunction,
    Error,
};

#[derive(Debug)]
struct Assignment {
    statement_idx: usize,
    variable: String,
}

fn local_assignment(statement: &Statement) -> Option<&str> {
    let target = match statement {
        Statement::Assign { target, .. } | Statement::DeclareAssign { target, .. } => target,
        _ => return None,
    };
    match target {
        Expr::Identifier(name) if name.starts_with("var_") => Some(name),
        _ => None,
    }
}

fn statement_usages(statement: &Statement) -> HashSet<String> {
    let mut usages = HashSet::new();
    let mut collect = |expr: &Expr| expr.collect_identifiers(&mut usages);

    match statement {
        Statement::Assign { target, value } | Statement::DeclareAssign { target, value, .. } => {
            collect(target);
            collect(value);
            if let Expr::Identifier(name) = target {
                usages.remove(name);
            }
        }
        Statement::If { condition } => collect(condition),
        Statement::IfRevertElse { condition, offset, size } => {
            collect(condition);
            collect(offset);
            collect(size);
        }
        Statement::Require { condition, reason } => {
            collect(condition);
            if let Some(reason) = reason {
                collect(reason);
            }
        }
        Statement::Return(value) | Statement::Expression(value) => collect(value),
        Statement::Emit { args, .. } | Statement::AssemblyAssign { args, .. } => {
            for arg in args {
                collect(arg);
            }
        }
        Statement::ExternalCall { address, args, gas, value, .. } => {
            collect(address);
            for arg in args {
                collect(arg);
            }
            if let Some(gas) = gas {
                collect(gas);
            }
            if let Some(value) = value {
                collect(value);
            }
        }
        Statement::CloseBlock => {}
    }

    usages
}

/// Eliminates local assignments that are overwritten or never subsequently referenced.
pub(crate) fn eliminate_dead_variables(
    function: &mut AnalyzedFunction,
    _: &mut PostprocessorState,
) -> Result<(), Error> {
    let assignments = function
        .statements
        .iter()
        .enumerate()
        .filter_map(|(statement_idx, statement)| {
            local_assignment(statement)
                .map(|variable| Assignment { statement_idx, variable: variable.to_string() })
        })
        .collect::<Vec<_>>();
    let usages = function.statements.iter().map(statement_usages).collect::<Vec<_>>();

    let mut assignments_by_variable: HashMap<&str, Vec<usize>> = HashMap::new();
    for assignment in &assignments {
        assignments_by_variable
            .entry(&assignment.variable)
            .or_default()
            .push(assignment.statement_idx);
    }

    let mut dead = HashSet::new();
    for assignment in &assignments {
        let end = assignments_by_variable
            .get(assignment.variable.as_str())
            .and_then(|indices| indices.iter().find(|&&idx| idx > assignment.statement_idx))
            .copied()
            .unwrap_or(function.statements.len());
        let used = usages[assignment.statement_idx + 1..end]
            .iter()
            .any(|names| names.contains(&assignment.variable));
        if !used {
            dead.insert(assignment.statement_idx);
        }
    }

    function.statements = function
        .statements
        .drain(..)
        .enumerate()
        .filter_map(|(idx, statement)| (!dead.contains(&idx)).then_some(statement))
        .collect();
    Ok(())
}

#[cfg(test)]
mod tests {
    use alloy::primitives::U256;

    use super::*;

    fn assignment(name: &str, value: Expr) -> Statement {
        Statement::DeclareAssign {
            ty: "uint256".to_string(),
            target: Expr::identifier(name),
            value,
        }
    }

    #[test]
    fn removes_unused_assignment() {
        let mut function = AnalyzedFunction::new("00000000", false);
        function.statements = vec![
            assignment("var_a", Expr::Literal(U256::from(1))),
            Statement::Return(Expr::Literal(U256::from(1))),
        ];
        eliminate_dead_variables(&mut function, &mut PostprocessorState::default()).unwrap();
        assert_eq!(function.statements.len(), 1);
    }

    #[test]
    fn retains_used_assignment() {
        let mut function = AnalyzedFunction::new("00000000", false);
        function.statements = vec![
            assignment("var_a", Expr::identifier("arg0")),
            Statement::Return(Expr::identifier("var_a")),
        ];
        eliminate_dead_variables(&mut function, &mut PostprocessorState::default()).unwrap();
        assert_eq!(function.statements.len(), 2);
    }

    #[test]
    fn retains_usage_in_storage_target() {
        let mut function = AnalyzedFunction::new("00000000", false);
        function.statements = vec![
            assignment("var_a", Expr::identifier("arg0")),
            Statement::Assign {
                target: Expr::index("storage", Expr::identifier("var_a")),
                value: Expr::Literal(U256::from(1)),
            },
        ];
        eliminate_dead_variables(&mut function, &mut PostprocessorState::default()).unwrap();
        assert_eq!(function.statements.len(), 2);
    }
}
