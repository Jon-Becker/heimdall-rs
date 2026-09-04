use alloy::primitives::U256;

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

fn zero_length_guard(condition: &Expr) -> bool {
    matches!(
        condition,
        Expr::Binary {
            op: BinaryOp::Ge,
            lhs,
            ..
        } if matches!(&**lhs, Expr::Literal(value) if value.is_zero())
    )
}

fn abi_encoded_return(block: &[Statement]) -> Option<Statement> {
    match block {
        [statement @ Statement::Return(Expr::Call { callee, .. })]
            if callee == "abi.encodePacked" =>
        {
            Some(statement.clone())
        }
        _ => None,
    }
}

fn is_trivial_guard(condition: &Expr) -> bool {
    matches!(condition, Expr::Bool(true))
}

fn is_nonpayable_guard(condition: &Expr) -> bool {
    matches!(
        condition,
        Expr::Unary { op: UnaryOp::LogicalNot, value }
            if matches!(&**value, Expr::Identifier(name) if name == "msg.value")
    )
}

fn is_success_identifier(condition: &Expr) -> bool {
    matches!(condition, Expr::Identifier(name) if name == "success")
}

fn is_calldata_length(expr: &Expr) -> bool {
    matches!(expr, Expr::Identifier(name) if name == "msg.data.length")
}

fn is_argument(expr: &Expr) -> bool {
    matches!(expr, Expr::Identifier(name) if name.strip_prefix("arg").is_some_and(|index| !index.is_empty() && index.bytes().all(|byte| byte.is_ascii_digit())) )
}

/// Returns whether an expression is composed exclusively from ABI offset arithmetic and whether
/// it contains evidence that it is relative to calldata rather than a user-defined numeric check.
fn calldata_offset(expr: &Expr) -> Option<bool> {
    match expr {
        Expr::Literal(value) => Some(*value == U256::from(4)),
        Expr::Identifier(_) if is_argument(expr) => Some(false),
        Expr::Cast { value, .. } => calldata_offset(value),
        Expr::Index { base, index } if matches!(&**base, Expr::Identifier(name) if name == "msg.data") => {
            calldata_offset(index).map(|_| true)
        }
        Expr::Binary { op, lhs, rhs }
            if matches!(op, BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Shl) =>
        {
            Some(calldata_offset(lhs)? || calldata_offset(rhs)?)
        }
        _ => None,
    }
}

/// Match only canonical ABI bounds checks, not arbitrary conditions that mention calldata size.
fn is_calldata_validity_guard(condition: &Expr) -> bool {
    let Expr::Binary { op, lhs, rhs } = condition else { return false };

    match op {
        // `msg.data.length - offset >= required`.
        BinaryOp::Ge | BinaryOp::Gt => {
            let Expr::Binary { op: BinaryOp::Sub, lhs: available, rhs: offset } = &**lhs else {
                return false;
            };
            is_calldata_length(available) &&
                calldata_offset(offset).is_some_and(|evidence| evidence) &&
                calldata_offset(rhs).is_some()
        }
        // `offset < msg.data.length`.
        BinaryOp::Lt | BinaryOp::Le => {
            is_calldata_length(rhs) && calldata_offset(lhs).is_some_and(|evidence| evidence)
        }
        _ => false,
    }
}

/// Remove only proven ABI bounds-check conjuncts, preserving every other condition.
fn remove_calldata_validity_conjuncts(condition: Expr) -> Expr {
    let Expr::Binary { op: BinaryOp::LogicalAnd, lhs, rhs } = condition else {
        return condition;
    };
    let lhs = remove_calldata_validity_conjuncts(*lhs);
    let rhs = remove_calldata_validity_conjuncts(*rhs);

    if is_calldata_validity_guard(&lhs) {
        rhs
    } else if is_calldata_validity_guard(&rhs) {
        lhs
    } else {
        Expr::binary(BinaryOp::LogicalAnd, lhs, rhs)
    }
}

fn push_require(output: &mut Vec<Statement>, condition: Expr, reason: Option<Expr>) {
    if is_trivial_guard(&condition) ||
        is_nonpayable_guard(&condition) ||
        is_calldata_validity_guard(&condition)
    {
        return;
    }
    if is_success_identifier(&condition) &&
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
                let condition = remove_calldata_validity_conjuncts(condition);
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
                                condition: remove_calldata_validity_conjuncts(Expr::binary(
                                    BinaryOp::LogicalAnd,
                                    condition,
                                    nested,
                                )),
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
            let redundant_guard = matches!(
                &statement,
                Statement::Require { condition, .. }
                    if is_trivial_guard(condition) ||
                        is_nonpayable_guard(condition) ||
                        is_calldata_validity_guard(condition)
            );
            let redundant_transfer_check = matches!(
                &statement,
                Statement::Require { condition, .. } if is_success_identifier(condition)
            ) && matches!(
                output.last(),
                Some(Statement::ExternalCall { function, .. }) if function == "transfer"
            );
            let is_terminating = terminating(&statement);
            if !matches!(statement, Statement::Noop) &&
                !redundant_guard &&
                !redundant_transfer_check
            {
                output.push(statement);
            }
            if is_terminating {
                break;
            }
            continue;
        };

        condition = remove_calldata_validity_conjuncts(condition);
        let mut then_body = simplify_block(then_body);
        let mut else_body = simplify_block(else_body);

        // Trace deduplication can make a syntactically constant branch share continuation blocks
        // with its sibling. Keep the structured branch intact instead of applying branch-hoisting
        // rewrites whose dominance assumptions are not valid for that incomplete trace.
        if matches!(condition, Expr::Bool(true) | Expr::Bool(false)) {
            output.push(Statement::IfElse { condition, then_body, else_body });
            continue;
        }

        // Dynamic-array loops commonly expose the zero-length return arm while executor jump
        // deduplication truncates the non-empty arm. The same ABI encoding is the loop's eventual
        // continuation, so recover it as the terminal return instead of emitting a function that
        // only returns for an empty array.
        if else_body.is_empty() && zero_length_guard(&condition) {
            if let Some(statement) = abi_encoded_return(&then_body) {
                output.push(statement);
                continue;
            }
        }

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

        let then_is_revert = revert_reason(&then_body).is_some();
        let else_is_revert = revert_reason(&else_body).is_some();

        if then_is_revert && else_is_revert {
            // Both branches revert. Prefer the one with a reason string, since a bare
            // `revert()` is often a truncated success path.
            match (revert_reason(&then_body), revert_reason(&else_body)) {
                (Some(None), Some(Some(reason))) => {
                    push_require(&mut output, condition, Some(reason));
                }
                (Some(Some(reason)), Some(None)) => {
                    push_require(&mut output, negate(condition), Some(reason));
                }
                _ => {
                    output.push(Statement::IfElse { condition, then_body, else_body });
                }
            }
        } else if let Some(reason) = revert_reason(&then_body) {
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
    // Do not simplify the complete tree here: trace deduplication can leave incomplete branch
    // alternatives, and normalizing a condition before recovering those alternatives can change
    // their apparent structure.
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
    fn recovers_return_after_truncated_dynamic_loop() {
        let returned = Statement::Return(Expr::Call {
            callee: "abi.encodePacked".to_string(),
            args: vec![Expr::identifier("result")],
        });
        let mut function = AnalyzedFunction::new("00000000", false);
        function.statements = vec![
            Statement::If {
                condition: Expr::binary(
                    BinaryOp::Ge,
                    Expr::Literal(U256::ZERO),
                    Expr::identifier("length"),
                ),
            },
            returned.clone(),
            Statement::Else,
            Statement::CloseBlock,
        ];
        structure_control_flow(&mut function, &mut PostprocessorState::default()).unwrap();
        assert_eq!(function.statements, vec![returned]);
    }

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
    fn removes_strict_calldata_validity_checks() {
        let mut function = AnalyzedFunction::new("00000000", false);
        let offset =
            Expr::binary(BinaryOp::Add, Expr::Literal(U256::from(4)), Expr::identifier("arg0"));
        function.statements = vec![Statement::Require {
            condition: Expr::binary(
                BinaryOp::Ge,
                Expr::binary(BinaryOp::Sub, Expr::identifier("msg.data.length"), offset),
                Expr::Literal(U256::from(32)),
            ),
            reason: None,
        }];
        structure_control_flow(&mut function, &mut PostprocessorState::default()).unwrap();
        assert!(function.statements.is_empty());
    }

    #[test]
    fn removes_calldata_validity_conjunct_but_preserves_contract_condition() {
        let validity = Expr::binary(
            BinaryOp::Ge,
            Expr::binary(
                BinaryOp::Sub,
                Expr::identifier("msg.data.length"),
                Expr::Literal(U256::from(4)),
            ),
            Expr::Literal(U256::from(32)),
        );
        let mut function = AnalyzedFunction::new("00000000", false);
        function.statements = vec![
            Statement::If {
                condition: Expr::binary(
                    BinaryOp::LogicalAnd,
                    validity,
                    Expr::identifier("authorized"),
                ),
            },
            Statement::Return(Expr::Bool(true)),
            Statement::Else,
            Statement::Revert(None),
            Statement::CloseBlock,
        ];
        structure_control_flow(&mut function, &mut PostprocessorState::default()).unwrap();
        assert!(matches!(
            function.statements.as_slice(),
            [Statement::Require { condition: Expr::Identifier(name), .. }, Statement::Return(Expr::Bool(true))]
                if name == "authorized"
        ));
    }

    #[test]
    fn preserves_non_abi_calldata_length_checks() {
        let mut function = AnalyzedFunction::new("00000000", false);
        let guard = Statement::Require {
            condition: Expr::binary(
                BinaryOp::Gt,
                Expr::identifier("msg.data.length"),
                Expr::identifier("minimumPayloadLength"),
            ),
            reason: None,
        };
        function.statements = vec![guard.clone()];
        structure_control_flow(&mut function, &mut PostprocessorState::default()).unwrap();
        assert_eq!(function.statements, vec![guard]);
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
    fn prefers_revert_with_reason_when_both_branches_revert() {
        let mut function = AnalyzedFunction::new("00000000", false);
        function.statements = vec![
            Statement::If { condition: Expr::identifier("authorized") },
            Statement::Revert(None),
            Statement::Else,
            Statement::Revert(Some(Expr::StringLiteral("not-authorized".to_string()))),
            Statement::CloseBlock,
        ];
        structure_control_flow(&mut function, &mut PostprocessorState::default()).unwrap();
        assert_eq!(
            function.statements[0].render(RenderTarget::Solidity),
            "require(authorized, \"not-authorized\");"
        );
    }

    #[test]
    fn prefers_negated_condition_when_bare_revert_is_in_else() {
        let mut function = AnalyzedFunction::new("00000000", false);
        function.statements = vec![
            Statement::If { condition: Expr::identifier("ok") },
            Statement::Revert(Some(Expr::StringLiteral("bad".to_string()))),
            Statement::Else,
            Statement::Revert(None),
            Statement::CloseBlock,
        ];
        structure_control_flow(&mut function, &mut PostprocessorState::default()).unwrap();
        assert_eq!(function.statements[0].render(RenderTarget::Solidity), "require(!ok, \"bad\");");
    }

    #[test]
    fn removes_require_true_after_expression_simplification() {
        let mut function = AnalyzedFunction::new("00000000", false);
        function.statements =
            vec![Statement::Require { condition: Expr::Bool(true), reason: None }];
        structure_control_flow(&mut function, &mut PostprocessorState::default()).unwrap();
        assert!(function.statements.is_empty());
    }

    #[test]
    fn keeps_constant_branches_intact_when_recovering_incomplete_traces() {
        let mut function = AnalyzedFunction::new("00000000", false);
        function.statements = vec![
            Statement::If { condition: Expr::Bool(false) },
            Statement::Revert(Some(Expr::StringLiteral("unreachable".to_string()))),
            Statement::Else,
            Statement::Return(Expr::Bool(true)),
            Statement::CloseBlock,
        ];
        structure_control_flow(&mut function, &mut PostprocessorState::default()).unwrap();
        assert!(matches!(
            function.statements.as_slice(),
            [Statement::IfElse { condition: Expr::Bool(false), then_body, else_body }]
                if then_body.len() == 1 && else_body == &vec![Statement::Return(Expr::Bool(true))]
        ));
    }

    #[test]
    fn keeps_if_else_when_both_branches_have_reasoned_reverts() {
        let mut function = AnalyzedFunction::new("00000000", false);
        function.statements = vec![
            Statement::If { condition: Expr::identifier("x") },
            Statement::Revert(Some(Expr::StringLiteral("a".to_string()))),
            Statement::Else,
            Statement::Revert(Some(Expr::StringLiteral("b".to_string()))),
            Statement::CloseBlock,
        ];
        structure_control_flow(&mut function, &mut PostprocessorState::default()).unwrap();
        assert!(
            matches!(&function.statements[0], Statement::IfElse { then_body, else_body, .. }
                if then_body.len() == 1 && else_body.len() == 1
            ),
            "expected IfElse, got {:?}",
            function.statements[0]
        );
    }
}
