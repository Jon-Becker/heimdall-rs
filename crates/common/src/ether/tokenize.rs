//! Tokenizer for expressions

use std::fmt::{Display, Formatter};

/// A token represents a single unit of an expression
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Token {
    /// A literal value, for example, "0x1234"
    Literal(String),
    /// A variable, for example, "a"
    Variable(String),
    /// An operator, for example, "+"
    Operator(String),
    /// An expression, for example, "(a + b)"
    Expression(Vec<Token>),
}

impl Display for Token {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Token::Literal(literal) => write!(f, "{literal}"),
            Token::Variable(variable) => write!(f, "{variable}"),
            Token::Operator(operator) => write!(f, "{operator}"),
            Token::Expression(tokens) => {
                write!(f, "(")?;
                for token in tokens.iter() {
                    write!(f, "{token} ")?;
                }
                write!(f, ")")
            }
        }
    }
}

impl Token {
    /// Simplifies an expression by:
    /// - removing unnecessary parentheses
    /// - removing redundant operators
    /// - evaluating literals
    ///
    /// Examples:
    /// - "a + (b + c)" -> "a + b + c"
    /// - "0x1234 + 0x5678" -> "0x68AC"
    /// - "0x01 * 0x02" -> "0x02"
    /// - "((a + b)) * c -> "(a + b) * c"
    ///
    /// Note: if the set of parentheses will change the order of operations, they will not be
    /// removed. A list of operators that change the order of operations are:
    /// - "*"
    /// - "/"
    /// - "%"
    /// - "<<"
    /// - ">>"
    /// - "&"
    /// - "|"
    /// - "^"
    /// - "~"
    pub fn simplify(&self) -> Token {
        match self {
            Token::Expression(tokens) => {
                let mut stack = Vec::new();
                let mut simplified = Vec::new();

                for token in tokens.iter() {
                    match token {
                        Token::Expression(inner_tokens) if inner_tokens.len() == 1 => {
                            stack.push(inner_tokens[0].simplify());
                        }
                        Token::Operator(op) if op == "&" || op == "|" || op == "^" => {
                            if let Some(Token::Operator(prev_op)) = stack.last() {
                                if op == prev_op {
                                    continue; // Skip redundant operators
                                }
                            }
                            stack.push(token.clone());
                        }
                        Token::Literal(lit) => {
                            if lit.len() <= 2 {
                                stack.push(token.clone());
                            } else if let Ok(num) = u64::from_str_radix(&lit[2..], 16) {
                                stack.push(Token::Literal(format!("0x{num:X}")));
                            } else {
                                stack.push(token.clone());
                            }
                        }
                        _ => stack.push(token.clone()),
                    }
                }

                // Flatten the stack and handle unnecessary parentheses
                while let Some(token) = stack.pop() {
                    match token {
                        Token::Expression(inner_tokens) => {
                            if inner_tokens.len() == 1 {
                                simplified.push(inner_tokens.into_iter().next().unwrap());
                            } else {
                                simplified.push(Token::Expression(inner_tokens));
                            }
                        }
                        _ => simplified.push(token),
                    }
                }

                simplified.reverse();
                Token::Expression(simplified)
            }
            _ => self.clone(),
        }
    }
}

/// Tokenizes an expression into a vector of tokens
///
/// Rules:
/// - Whitespace is ignored, but may be used to separate tokens
/// - Operators are treated as their own tokens
/// - Variables are treated as their own tokens
/// - Literals are hex strings, for example, "0x1234"
/// - When entering a set of parentheses, a new expression is started
/// - When exiting a set of parentheses, the current expression is ended
///
/// Examples:
/// - "a + b" -> [Variable("a"), Operator("+"), Variable("b")]
/// - "a + (b + c)" -> [Variable("a"), Operator("+"), Expression([Variable("b"), Operator("+"),
///   Variable("c")])]
/// - "0x1234 + 0x5678" -> [Literal("0x1234"), Operator("+"), Literal("0x5678")]
/// - "a + b * c" -> [Variable("a"), Operator("+"), Variable("b"), Operator("*"), Variable("c")]
/// - "a + (b * (c - d))" -> [Variable("a"), Operator("+"), Expression([Variable("b"),
///   Operator("*"), Expression([Variable("c"), Operator("-"), Variable("d")])])]
pub fn tokenize(s: &str) -> Token {
    let mut tokens = Vec::new();
    let mut iter = s.chars().peekable();

    while let Some(&ch) = iter.peek() {
        match ch {
            '+' | '-' | '*' | '/' | '=' | '>' | '<' | '!' | '&' | '|' | ';' | '%' | '^' | '~' => {
                let mut op = ch.to_string();
                iter.next();
                if let Some(&next_ch) = iter.peek() {
                    if (ch == '=' && (next_ch == '=' || next_ch == '>')) ||
                        (ch == '&' && next_ch == '&') ||
                        (ch == '|' && next_ch == '|') ||
                        (ch == '<' && next_ch == '=') ||
                        (ch == '>' && next_ch == '=') ||
                        (ch == '!' && next_ch == '=') ||
                        (ch == '+' && next_ch == '+') ||
                        (ch == '-' && next_ch == '-') ||
                        (ch == '*' && next_ch == '*') ||
                        (ch == '>' && next_ch == '>') ||
                        (ch == '<' && next_ch == '<')
                    {
                        op.push(next_ch);
                        iter.next();
                    }
                }
                tokens.push(Token::Operator(op));
            }
            'a'..='z' | 'A'..='Z' | '.' | '[' | ']' | '_' => {
                let variable = parse_variable(&mut iter);
                tokens.push(Token::Variable(variable));
            }
            '0' => {
                let literal = parse_literal(&mut iter);
                tokens.push(Token::Literal(literal));
            }
            '(' => {
                iter.next();
                let expr = tokenize(&consume_parentheses(&mut iter));
                tokens.push(expr);
            }
            ')' => {
                iter.next();
                break;
            }
            _ => {
                iter.next();
            }
        }
    }

    Token::Expression(tokens)
}

fn parse_literal(iter: &mut std::iter::Peekable<std::str::Chars<'_>>) -> String {
    let mut literal = String::new();

    while let Some(&ch) = iter.peek() {
        match ch {
            '0'..='9' | 'a'..='f' | 'A'..='F' | 'x' => {
                literal.push(ch);
                iter.next();
            }
            _ => break,
        }
    }

    // literal validation
    if literal.starts_with("0x") &&
        literal.len() > 2 &&
        literal[2..].chars().all(|c| c.is_ascii_hexdigit())
    {
        return literal;
    }

    // rewind
    iter.by_ref().take(literal.len()).for_each(|_| {});
    String::from("0")
}

fn parse_variable(iter: &mut std::iter::Peekable<std::str::Chars<'_>>) -> String {
    let mut variable = String::new();
    while let Some(&ch) = iter.peek() {
        match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '.' | '[' | ']' | '_' => {
                variable.push(ch);
                iter.next();
            }
            _ => break,
        }
    }
    variable
}

fn consume_parentheses(iter: &mut std::iter::Peekable<std::str::Chars<'_>>) -> String {
    let mut expression = String::new();
    let mut parentheses_count = 1;

    while let Some(&ch) = iter.peek() {
        match ch {
            '(' => {
                expression.push(ch);
                iter.next();
                parentheses_count += 1;
            }
            ')' => {
                expression.push(ch);
                iter.next();
                parentheses_count -= 1;
                if parentheses_count == 0 {
                    break;
                }
            }
            _ => {
                expression.push(ch);
                iter.next();
            }
        }
    }

    expression
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_tokenize() {
        let s = "a + b * c";
        let result = tokenize(s);
        assert_eq!(
            result,
            Token::Expression(vec![
                Token::Variable("a".to_string()),
                Token::Operator("+".to_string()),
                Token::Variable("b".to_string()),
                Token::Operator("*".to_string()),
                Token::Variable("c".to_string()),
            ])
        );
    }

    #[test]
    fn test_tokenize_double_op() {
        let s = "a + b * c == d";
        let result = tokenize(s);
        assert_eq!(
            result,
            Token::Expression(vec![
                Token::Variable("a".to_string()),
                Token::Operator("+".to_string()),
                Token::Variable("b".to_string()),
                Token::Operator("*".to_string()),
                Token::Variable("c".to_string()),
                Token::Operator("==".to_string()),
                Token::Variable("d".to_string()),
            ])
        );
    }

    #[test]
    fn test_tokenize_long_var() {
        let s = "aaaaa + b * c";
        let result = tokenize(s);
        assert_eq!(
            result,
            Token::Expression(vec![
                Token::Variable("aaaaa".to_string()),
                Token::Operator("+".to_string()),
                Token::Variable("b".to_string()),
                Token::Operator("*".to_string()),
                Token::Variable("c".to_string()),
            ])
        );
    }

    #[test]
    fn test_tokenize_literal() {
        let s = "0x1234 + 0x5678";
        let result = tokenize(s);
        assert_eq!(
            result,
            Token::Expression(vec![
                Token::Literal("0x1234".to_string()),
                Token::Operator("+".to_string()),
                Token::Literal("0x5678".to_string()),
            ])
        );
    }

    #[test]
    fn test_tokenize_parentheses() {
        let s = "a + (b * (c - d))";
        let result = tokenize(s);
        assert_eq!(
            result,
            Token::Expression(vec![
                Token::Variable("a".to_string()),
                Token::Operator("+".to_string()),
                Token::Expression(vec![
                    Token::Variable("b".to_string()),
                    Token::Operator("*".to_string()),
                    Token::Expression(vec![
                        Token::Variable("c".to_string()),
                        Token::Operator("-".to_string()),
                        Token::Variable("d".to_string()),
                    ]),
                ]),
            ])
        );
    }

    #[test]
    fn test_tokenize_simplify_expr_none() {
        let s = "a + b * c";
        let result = tokenize(s);
        assert_eq!(
            result.simplify(),
            Token::Expression(vec![
                Token::Variable("a".to_string()),
                Token::Operator("+".to_string()),
                Token::Variable("b".to_string()),
                Token::Operator("*".to_string()),
                Token::Variable("c".to_string()),
            ])
        );
    }

    #[test]
    fn test_tokenize_simplify_expr_keeps_necessary_parens() {
        let s = "(a + b) * c";
        let result = tokenize(s);
        assert_eq!(
            result.simplify(),
            Token::Expression(vec![
                Token::Expression(vec![
                    Token::Variable("a".to_string()),
                    Token::Operator("+".to_string()),
                    Token::Variable("b".to_string()),
                ]),
                Token::Operator("*".to_string()),
                Token::Variable("c".to_string()),
            ])
        );
    }

    #[test]
    fn test_tokenize_simplify_expr_removes_double_parens() {
        let s = "((a + b)) * c";
        let result = tokenize(s);
        assert_eq!(
            result.simplify(),
            Token::Expression(vec![
                Token::Expression(vec![
                    Token::Variable("a".to_string()),
                    Token::Operator("+".to_string()),
                    Token::Variable("b".to_string()),
                ]),
                Token::Operator("*".to_string()),
                Token::Variable("c".to_string()),
            ])
        );
    }

    #[test]
    fn test_tokenize_simplify_expr_keeps_necessary_parens_2() {
        let s = "(a + b) * (c + d)";
        let result = tokenize(s);
        assert_eq!(
            result.simplify(),
            Token::Expression(vec![
                Token::Expression(vec![
                    Token::Variable("a".to_string()),
                    Token::Operator("+".to_string()),
                    Token::Variable("b".to_string()),
                ]),
                Token::Operator("*".to_string()),
                Token::Expression(vec![
                    Token::Variable("c".to_string()),
                    Token::Operator("+".to_string()),
                    Token::Variable("d".to_string()),
                ]),
            ])
        );
    }
}
