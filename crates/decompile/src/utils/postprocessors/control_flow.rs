use crate::{
    core::{
        ir::{BinaryOp, Expr, Statement, UnaryOp},
        postprocess::PostprocessorState,
    },
    interfaces::AnalyzedFunction,
    Error,
};

fn parse_block(flat: &[Statement], cursor: &mut usize) -> Vec<Statement> {
    let mut block = Vec::new();
    while *cursor < flat.len() {
        match &flat[*cursor] {
            Statement::Else | Statement::CloseBlock => break,
            Statement::If { condition } => {
                let condition = condition.clone();
                *cursor += 1;
                let then_body = parse_block(flat, cursor);
                let else_body = if matches!(flat.get(*cursor), Some(Statement::Else)) {
                    *cursor += 1;
                    parse_block(flat, cursor)
                } else {
                    Vec::new()
                };
                if matches!(flat.get(*cursor), Some(Statement::CloseBlock)) {
                    *cursor += 1;
                }
                block.push(Statement::IfElse { condition, then_body, else_body });
            }
            statement => {
                block.push(statement.clone());
                *cursor += 1;
            }
        }
    }
    block
}

fn terminating(statement: &Statement) -> bool {
    matches!(statement, Statement::Return(_) | Statement::Revert(_))
}

fn revert_reason(block: &[Statement]) -> Option<Option<Expr>> {
    match block {
        [Statement::Revert(reason)] => Some(reason.clone()),
        _ => None,
    }
}

fn negate(condition: Expr) -> Expr {
    Expr::Unary { op: UnaryOp::LogicalNot, value: Box::new(condition) }.simplify()
}

fn push_require(output: &mut Vec<Statement>, condition: Expr, reason: Option<Expr>) {
    if condition.render() == "!msg.value" {
        return;
    }
    if condition.render() == "success" &&
        matches!(
            output.last(),
            Some(Statement::ExternalCall { function, .. }) if function == "transfer"
        )
    {
        return;
    }
    output.push(Statement::Require { condition, reason });
}

fn combine_nested_conditions(block: Vec<Statement>) -> Vec<Statement> {
    block
        .into_iter()
        .map(|statement| match statement {
            Statement::IfElse { condition, then_body, else_body } => {
                let mut then_body = combine_nested_conditions(then_body);
                let else_body = combine_nested_conditions(else_body);
                if else_body.is_empty() && then_body.len() == 1 {
                    match then_body.remove(0) {
                        Statement::IfElse {
                            condition: nested,
                            then_body: nested_then,
                            else_body: nested_else,
                        } if nested_else.is_empty() => {
                            return Statement::IfElse {
                                condition: Expr::binary(BinaryOp::LogicalAnd, condition, nested),
                                then_body: nested_then,
                                else_body: Vec::new(),
                            }
                        }
                        nested => then_body = vec![nested],
                    }
                }
                Statement::IfElse { condition, then_body, else_body }
            }
            statement => statement,
        })
        .collect()
}

fn simplify_block(block: Vec<Statement>) -> Vec<Statement> {
    let mut output = Vec::new();
    for statement in block {
        let Statement::IfElse { mut condition, then_body, else_body } = statement else {
            let redundant_nonpayable_guard = matches!(
                &statement,
                Statement::Require { condition, .. } if condition.render() == "!msg.value"
            );
            let redundant_transfer_check = matches!(
                &statement,
                Statement::Require { condition, .. } if condition.render() == "success"
            ) && matches!(
                output.last(),
                Some(Statement::ExternalCall { function, .. }) if function == "transfer"
            );
            let is_terminating = terminating(&statement);
            if !matches!(statement, Statement::Noop) &&
                !redundant_nonpayable_guard &&
                !redundant_transfer_check
            {
                output.push(statement);
            }
            if is_terminating {
                break;
            }
            continue
        };

        let mut then_body = simplify_block(then_body);
        let mut else_body = simplify_block(else_body);

        let mut common_tail = Vec::new();
        while !then_body.is_empty() && !else_body.is_empty() && then_body.last() == else_body.last()
        {
            common_tail.push(then_body.pop().expect("checked non-empty branch"));
            else_body.pop();
        }
        common_tail.reverse();

        if else_body.is_empty() && then_body.len() == 1 {
            match then_body.remove(0) {
                Statement::IfElse {
                    condition: nested_condition,
                    then_body: nested_then,
                    else_body: nested_else,
                } if nested_else.is_empty() => {
                    condition = Expr::binary(BinaryOp::LogicalAnd, condition, nested_condition);
                    then_body = nested_then;
                }
                nested => then_body = vec![nested],
            }
        }

        if let Some(reason) = revert_reason(&then_body) {
            push_require(&mut output, negate(condition), reason);
            output.extend(else_body);
        } else if let Some(reason) = revert_reason(&else_body) {
            push_require(&mut output, condition, reason);
            output.extend(then_body);
        } else if then_body.is_empty() {
            if !else_body.is_empty() {
                output.push(Statement::IfElse {
                    condition: negate(condition),
                    then_body: else_body,
                    else_body: Vec::new(),
                });
            }
        } else {
            output.push(Statement::IfElse { condition, then_body, else_body });
        }
        output.extend(common_tail);
    }
    output
}

/// Converts flattened branch markers into nested bodies, hoists common tails, and removes
/// statements unreachable after return/revert within each branch.
pub(crate) fn structure_control_flow(
    function: &mut AnalyzedFunction,
    _: &mut PostprocessorState,
) -> Result<(), Error> {
    let flat = function.statements.clone();
    let mut cursor = 0;
    function.statements =
        combine_nested_conditions(simplify_block(parse_block(&flat, &mut cursor)));
    Ok(())
}

#[cfg(test)]
mod tests {
    use alloy::primitives::U256;

    use super::*;
    use crate::core::ir::RenderTarget;

    #[test]
    fn hoists_common_branch_tail() {
        let common = Statement::Return(Expr::Bool(true));
        let mut function = AnalyzedFunction::new("00000000", false);
        function.statements = vec![
            Statement::If { condition: Expr::identifier("condition") },
            Statement::Assign {
                target: Expr::identifier("x"),
                value: Expr::Literal(U256::from(1)),
            },
            common.clone(),
            Statement::Else,
            Statement::Assign {
                target: Expr::identifier("x"),
                value: Expr::Literal(U256::from(2)),
            },
            common.clone(),
            Statement::CloseBlock,
        ];
        structure_control_flow(&mut function, &mut PostprocessorState::default()).unwrap();
        assert_eq!(function.statements.last(), Some(&common));
        assert!(matches!(
            &function.statements[0],
            Statement::IfElse { then_body, else_body, .. }
                if then_body.len() == 1 && else_body.len() == 1
        ));
    }

    #[test]
    fn combines_nested_conditions() {
        let mut function = AnalyzedFunction::new("00000000", false);
        function.statements = vec![
            Statement::If { condition: Expr::identifier("a") },
            Statement::If { condition: Expr::identifier("b") },
            Statement::Return(Expr::Bool(true)),
            Statement::Else,
            Statement::CloseBlock,
            Statement::Else,
            Statement::CloseBlock,
        ];
        structure_control_flow(&mut function, &mut PostprocessorState::default()).unwrap();
        assert!(matches!(
            &function.statements[0],
            Statement::IfElse { condition, .. } if condition.render() == "a && b"
        ));
    }

    #[test]
    fn removes_redundant_nonpayable_guard() {
        let mut function = AnalyzedFunction::new("00000000", false);
        function.statements = vec![
            Statement::If { condition: Expr::identifier("msg.value") },
            Statement::Revert(None),
            Statement::Else,
            Statement::Return(Expr::Bool(true)),
            Statement::CloseBlock,
        ];
        structure_control_flow(&mut function, &mut PostprocessorState::default()).unwrap();
        assert_eq!(function.statements, vec![Statement::Return(Expr::Bool(true))]);
    }

    #[test]
    fn promotes_revert_branch_to_require() {
        let mut function = AnalyzedFunction::new("00000000", false);
        function.statements = vec![
            Statement::If { condition: Expr::identifier("failed") },
            Statement::Revert(None),
            Statement::Else,
            Statement::Return(Expr::Literal(U256::from(1))),
            Statement::CloseBlock,
        ];
        structure_control_flow(&mut function, &mut PostprocessorState::default()).unwrap();
        assert_eq!(function.statements[0].render(RenderTarget::Solidity), "require(!failed);");
    }
}
