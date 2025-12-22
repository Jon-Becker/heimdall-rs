use alloy::primitives::U256;

use crate::core::stack::StackFrame;

/// Check if a condition is tautologically false (e.g., "0 > 1", "(0 > 0x01)").
/// These cannot be valid loop conditions and should be skipped.
pub(crate) fn is_tautologically_false_condition(condition: &str) -> bool {
    // Strip outer whitespace first
    let mut trimmed = condition.trim();

    // Remove leading negation (!) - we're looking at the inner condition
    if trimmed.starts_with('!') {
        trimmed = trimmed[1..].trim();
    }

    // Strip all outer parentheses (could be nested like "((...))")
    while trimmed.starts_with('(') && trimmed.ends_with(')') {
        trimmed = &trimmed[1..trimmed.len() - 1];
        trimmed = trimmed.trim();
    }

    // Pattern: "0 > X" where X > 0 (always false for unsigned)
    if trimmed.starts_with("0 >") || trimmed.starts_with("0x0 >") || trimmed.starts_with("0x00 >") {
        return true;
    }

    // Pattern: "X < 0" (always false for unsigned)
    if trimmed.ends_with("< 0") || trimmed.ends_with("< 0x0") || trimmed.ends_with("< 0x00") {
        return true;
    }

    // Pattern: constant comparisons that are always false
    // e.g., "0 > 0x01", "1 > 2", etc.
    for op in [" > ", " >= ", " < ", " <= "] {
        if let Some(pos) = trimmed.find(op) {
            let lhs = trimmed[..pos].trim();
            let rhs = trimmed[pos + op.len()..].trim();

            // Try to parse both sides as numbers
            if let (Some(lhs_val), Some(rhs_val)) = (parse_const(lhs), parse_const(rhs)) {
                let result = match op {
                    " > " => lhs_val > rhs_val,
                    " >= " => lhs_val >= rhs_val,
                    " < " => lhs_val < rhs_val,
                    " <= " => lhs_val <= rhs_val,
                    _ => return false,
                };
                // If the result is always false, this isn't a valid loop condition
                if !result {
                    return true;
                }
            }
        }
    }

    false
}

/// Parse a constant value (decimal or hex)
fn parse_const(s: &str) -> Option<u64> {
    let trimmed = s.trim();
    if let Some(hex_str) = trimmed.strip_prefix("0x") {
        u64::from_str_radix(hex_str, 16).ok()
    } else {
        trimmed.parse::<u64>().ok()
    }
}

/// Represents a detected loop in the control flow
#[derive(Clone, Debug, Default)]
pub struct LoopInfo {
    /// Program counter of the loop header (JUMPDEST target)
    pub header_pc: u128,

    /// Program counter of the conditional jump (JUMPI)
    pub condition_pc: u128,

    /// The solidified loop condition (e.g., "i < arg0")
    pub condition: String,

    /// The negated condition for while-loop form
    pub exit_condition: String,

    /// Detected induction variable name, if any
    pub induction_var: Option<InductionVariable>,

    /// Whether this appears to be a bounded loop (for) vs unbounded (while)
    pub is_bounded: bool,

    /// Storage slots modified within the loop
    pub modified_storage: Vec<U256>,

    /// Memory locations modified within the loop
    pub modified_memory: Vec<U256>,
}

/// Represents a loop induction variable (counter)
#[derive(Clone, Debug)]
pub struct InductionVariable {
    /// The variable identifier (e.g., "var_a" or stack position)
    pub name: String,

    /// Initial value expression
    pub init: String,

    /// Step expression (usually "+ 1" or "- 1")
    pub step: String,

    /// Bound expression (e.g., "arg0")
    pub bound: Option<String>,

    /// Whether counting up or down
    pub direction: InductionDirection,
}

/// Direction of loop induction variable
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum InductionDirection {
    /// Incrementing (i++)
    Ascending,
    /// Decrementing (i--)
    Descending,
    /// Unknown direction
    #[default]
    Unknown,
}

impl LoopInfo {
    /// Create a new LoopInfo from PC positions and condition
    pub fn new(header_pc: u128, condition_pc: u128, condition: String) -> Self {
        // Normalize the condition (unwrap ISZERO, fix operand order, etc.)
        let normalized = normalize_loop_condition(&condition);
        Self {
            header_pc,
            condition_pc,
            exit_condition: negate_condition(&normalized),
            condition: normalized,
            induction_var: None,
            is_bounded: false,
            modified_storage: Vec::new(),
            modified_memory: Vec::new(),
        }
    }

    /// Set the loop counter name based on nesting depth.
    ///
    /// For nested loops, this assigns unique names (i, j, k, l, m, n, ...).
    /// This should be called when the loop is about to be emitted, with the
    /// current nesting depth as the parameter.
    pub fn set_counter_name_for_depth(&mut self, depth: usize) {
        let counter_name = counter_name_for_depth(depth);

        // Update the induction variable name if present
        if let Some(ref mut iv) = self.induction_var {
            let old_name = iv.name.clone();
            iv.name = counter_name.clone();

            // Update the condition to use the new name
            if self.condition.contains(&old_name) {
                self.condition = self.condition.replace(&old_name, &counter_name);
                self.exit_condition = self.exit_condition.replace(&old_name, &counter_name);
            }
        } else {
            // Even without an induction variable, update the condition if it uses "i"
            if self.condition.contains("i ") || self.condition.starts_with("i ") {
                self.condition = self.condition.replacen("i ", &format!("{} ", counter_name), 1);
                self.exit_condition =
                    self.exit_condition.replacen("i ", &format!("{} ", counter_name), 1);
            }
        }
    }

    /// Generate Solidity loop construct
    pub fn to_solidity(&self) -> String {
        match &self.induction_var {
            Some(iv) if self.is_bounded => {
                // for-loop form
                let init = format!("uint256 {} = {}", iv.name, iv.init);
                let cond = self.condition.clone();
                let step = match iv.direction {
                    InductionDirection::Ascending => format!("{}++", iv.name),
                    InductionDirection::Descending => format!("{}--", iv.name),
                    InductionDirection::Unknown => format!("{} {}", iv.name, iv.step),
                };
                format!("for ({init}; {cond}; {step}) {{")
            }
            _ => {
                // while-loop form
                format!("while ({}) {{", self.condition)
            }
        }
    }
}

/// Generate a counter variable name for the given nesting depth.
///
/// Returns: i, j, k, l, m, n for depths 0-5, then idx6, idx7, etc.
fn counter_name_for_depth(depth: usize) -> String {
    const COUNTER_NAMES: [&str; 6] = ["i", "j", "k", "l", "m", "n"];
    if depth < COUNTER_NAMES.len() {
        COUNTER_NAMES[depth].to_string()
    } else {
        format!("idx{}", depth)
    }
}

/// Normalize a loop condition extracted from JUMPI.
///
/// EVM uses inverted logic for conditional jumps: JUMPI jumps when condition is TRUE.
/// For loops like `while (i < limit)`, the bytecode is typically:
///   LT(i, limit) -> ISZERO -> JUMPI
/// This means "jump out of loop when i >= limit".
///
/// The raw solidified condition comes out as `!(i < limit)` or malformed `!i < limit`.
/// This function normalizes it back to `i < limit` for proper loop representation.
fn normalize_loop_condition(condition: &str) -> String {
    let trimmed = condition.trim();

    // Pattern 1: "!(comparison)" - properly wrapped negation
    // e.g., "!(i < arg0)" -> "i < arg0"
    if trimmed.starts_with("!(") && trimmed.ends_with(')') {
        let inner = &trimmed[2..trimmed.len() - 1];
        // Only unwrap if inner contains a comparison operator
        if contains_comparison(inner) {
            return inner.trim().to_string();
        }
    }

    // Pattern 2: "!X op Y" - malformed negation where ! applies to left operand only
    // e.g., "!0x01 < arg0" -> need to extract and fix
    // e.g., "!0 > 0x01" -> need to extract and fix
    if trimmed.starts_with('!') && !trimmed.starts_with("!=") {
        let rest = &trimmed[1..];

        // Find the comparison operator
        for (op, normalized_op) in [
            (" < ", " < "),
            (" > ", " > "),
            (" <= ", " <= "),
            (" >= ", " >= "),
            (" == ", " == "),
            (" != ", " != "),
        ] {
            if let Some(pos) = rest.find(op) {
                let lhs = rest[..pos].trim();
                let rhs = rest[pos + op.len()..].trim();

                // The `!` was incorrectly applied to lhs - this is the loop counter
                // For ISZERO(LT(counter, limit)), the condition should be counter < limit
                // The lhs here might be "0" or "0x01" (representing the counter value)

                // If lhs is a simple value and rhs looks like an argument, swap the comparison
                // to get proper "counter < limit" form
                if is_likely_counter_value(lhs) && is_likely_bound(rhs) {
                    // This was ISZERO(LT(counter, limit)) solidified incorrectly
                    // The actual condition is: counter < limit
                    return format!("i{}{}", normalized_op, rhs);
                }

                // If rhs is a counter value and lhs looks like a bound, it's reversed
                // e.g., "!arg0 > 0x01" -> "i < arg0"
                if is_likely_counter_value(rhs) && is_likely_bound(lhs) {
                    // Flip the comparison operator for proper form
                    let flipped_op = match normalized_op {
                        " < " => " > ",
                        " > " => " < ",
                        " <= " => " >= ",
                        " >= " => " <= ",
                        other => other,
                    };
                    return format!("i{}{}", flipped_op, lhs);
                }

                // If both look like counter values (constants), this is likely an
                // internal compiler check - use a simple normalized form
                if is_likely_counter_value(lhs) && is_likely_counter_value(rhs) {
                    // For patterns like "!0 > 0x01", interpret as a loop check
                    // The actual counter would be tracked separately
                    return format!("i{}{}", normalized_op, rhs);
                }

                // Otherwise, try to reconstruct sensibly
                return format!("{}{}{}", lhs, normalized_op, rhs);
            }
        }
    }

    // Pattern 3: Already looks correct or can't be normalized further
    trimmed.to_string()
}

/// Check if a string contains a comparison operator
fn contains_comparison(s: &str) -> bool {
    s.contains(" < ") ||
        s.contains(" > ") ||
        s.contains(" <= ") ||
        s.contains(" >= ") ||
        s.contains(" == ") ||
        s.contains(" != ")
}

/// Check if a value looks like a loop counter value (small number or zero)
fn is_likely_counter_value(s: &str) -> bool {
    let trimmed = s.trim().trim_start_matches('(').trim_end_matches(')');
    // Match "0", "0x0", "0x00", "0x01", "1", etc.
    trimmed == "0" ||
        trimmed == "1" ||
        trimmed == "0x0" ||
        trimmed == "0x00" ||
        trimmed == "0x01" ||
        trimmed == "0x1" ||
        trimmed.parse::<u64>().map(|n| n <= 1).unwrap_or(false)
}

/// Check if a value looks like a loop bound (argument or variable)
fn is_likely_bound(s: &str) -> bool {
    let trimmed = s.trim();
    // Arguments look like "arg0", "arg1", etc.
    // Variables look like "var_a", "var_b", etc.
    // Also could be storage reads like "storage[0x00]"
    trimmed.starts_with("arg") ||
        trimmed.starts_with("var_") ||
        trimmed.contains("storage[") ||
        trimmed.contains("memory[")
}

/// Negate a boolean condition for loop exit
fn negate_condition(condition: &str) -> String {
    let trimmed = condition.trim();

    // Handle already-negated conditions
    if trimmed.starts_with('!') && !trimmed.starts_with("!=") {
        // Remove the negation
        return trimmed[1..].trim_start_matches('(').trim_end_matches(')').to_string();
    }

    // Handle comparison operators
    if trimmed.contains(">=") {
        return trimmed.replace(">=", "<");
    }
    if trimmed.contains("<=") {
        return trimmed.replace("<=", ">");
    }
    if trimmed.contains("==") {
        return trimmed.replace("==", "!=");
    }
    if trimmed.contains("!=") {
        return trimmed.replace("!=", "==");
    }
    if trimmed.contains(" > ") {
        return trimmed.replace(" > ", " <= ");
    }
    if trimmed.contains(" < ") {
        return trimmed.replace(" < ", " >= ");
    }

    // Default: wrap with negation
    format!("!({})", condition)
}

/// Attempt to detect an induction variable from the stack diff and/or condition
pub(super) fn detect_induction_variable(
    stack_diff: &[StackFrame],
    jump_condition: &Option<String>,
) -> Option<InductionVariable> {
    // First, try to detect from stack diff (most accurate when it works)
    for frame in stack_diff {
        let solidified = frame.operation.solidify();

        // Check for increment pattern (e.g., "var_a + 0x01" or "something + 1")
        if let Some(var_name) = extract_increment_var(&solidified) {
            let bound = extract_bound_from_condition(jump_condition, &var_name);

            return Some(InductionVariable {
                name: simplify_var_name(&var_name),
                init: "0".to_string(),
                step: "+ 1".to_string(),
                bound,
                direction: InductionDirection::Ascending,
            });
        }

        // Check for decrement pattern (e.g., "var_a - 0x01" or "something - 1")
        if let Some(var_name) = extract_decrement_var(&solidified) {
            let bound = extract_bound_from_condition(jump_condition, &var_name);

            return Some(InductionVariable {
                name: simplify_var_name(&var_name),
                init: bound.unwrap_or_else(|| "?".to_string()),
                step: "- 1".to_string(),
                bound: Some("0".to_string()),
                direction: InductionDirection::Descending,
            });
        }
    }

    // Fallback: try to infer from the condition itself
    // This handles cases where stack diff doesn't capture the increment
    if let Some(cond) = jump_condition {
        if let Some(iv) = infer_induction_from_condition(cond) {
            return Some(iv);
        }
    }

    None
}

/// Infer an induction variable from a loop condition pattern.
///
/// For conditions like "i < arg0" or "var_a < limit", we can infer
/// that the left-hand side is likely an induction variable.
fn infer_induction_from_condition(condition: &str) -> Option<InductionVariable> {
    let normalized = normalize_loop_condition(condition);

    // Look for patterns like "X < Y" where X is the counter and Y is the bound
    for op in [" < ", " <= "] {
        if let Some(pos) = normalized.find(op) {
            let lhs = normalized[..pos].trim();
            let rhs = normalized[pos + op.len()..].trim();

            // LHS should look like a variable, not a constant
            if !lhs.is_empty() && !is_hex_constant(lhs) && !is_decimal_constant(lhs) {
                return Some(InductionVariable {
                    name: simplify_var_name(lhs),
                    init: "0".to_string(),
                    step: "+ 1".to_string(),
                    bound: Some(rhs.to_string()),
                    direction: InductionDirection::Ascending,
                });
            }
        }
    }

    // Look for decrementing patterns like "X > Y" or "X >= Y"
    for op in [" > ", " >= "] {
        if let Some(pos) = normalized.find(op) {
            let lhs = normalized[..pos].trim();
            let rhs = normalized[pos + op.len()..].trim();

            // LHS should look like a variable
            if !lhs.is_empty() && !is_hex_constant(lhs) && !is_decimal_constant(lhs) {
                // For "i > 0", the counter decrements from some init value to 0
                let bound_val = if rhs == "0" || rhs == "0x0" || rhs == "0x00" { "0" } else { rhs };

                return Some(InductionVariable {
                    name: simplify_var_name(lhs),
                    init: "?".to_string(), // Unknown init for decrementing
                    step: "- 1".to_string(),
                    bound: Some(bound_val.to_string()),
                    direction: InductionDirection::Descending,
                });
            }
        }
    }

    None
}

/// Check if a string is a hexadecimal constant
fn is_hex_constant(s: &str) -> bool {
    let trimmed = s.trim();
    trimmed.starts_with("0x") &&
        trimmed[2..].chars().all(|c| c.is_ascii_hexdigit()) &&
        trimmed.len() > 2
}

/// Check if a string is a decimal constant
fn is_decimal_constant(s: &str) -> bool {
    let trimmed = s.trim();
    !trimmed.is_empty() && trimmed.chars().all(|c| c.is_ascii_digit())
}

/// Extract variable name from increment pattern like "var_a + 0x01" or "var_a + 1"
fn extract_increment_var(solidified: &str) -> Option<String> {
    // Match patterns like "X + 0x01", "X + 0x1", "X + 1"
    let patterns = [" + 0x01", " + 0x1", " + 1"];
    for pattern in patterns {
        if solidified.ends_with(pattern) {
            let var_name = solidified.strip_suffix(pattern)?;
            return Some(var_name.trim().to_string());
        }
    }
    // Also check for wrapped patterns like "(X + 0x01)"
    let trimmed = solidified.trim();
    if trimmed.starts_with('(') && trimmed.ends_with(')') {
        let inner = &trimmed[1..trimmed.len() - 1];
        return extract_increment_var(inner);
    }
    None
}

/// Extract variable name from decrement pattern like "var_a - 0x01" or "var_a - 1"
fn extract_decrement_var(solidified: &str) -> Option<String> {
    // Match patterns like "X - 0x01", "X - 0x1", "X - 1"
    let patterns = [" - 0x01", " - 0x1", " - 1"];
    for pattern in patterns {
        if solidified.ends_with(pattern) {
            let var_name = solidified.strip_suffix(pattern)?;
            return Some(var_name.trim().to_string());
        }
    }
    // Also check for wrapped patterns
    let trimmed = solidified.trim();
    if trimmed.starts_with('(') && trimmed.ends_with(')') {
        let inner = &trimmed[1..trimmed.len() - 1];
        return extract_decrement_var(inner);
    }
    None
}

/// Extract the loop bound from a condition like "i < loops"
fn extract_bound_from_condition(condition: &Option<String>, var_name: &str) -> Option<String> {
    let cond = condition.as_ref()?;

    // Look for comparison patterns
    let operators = ["<", ">", "<=", ">=", "==", "!="];
    for op in operators {
        if cond.contains(op) {
            let parts: Vec<&str> = cond.split(op).collect();
            if parts.len() == 2 {
                let lhs = parts[0].trim();
                let rhs = parts[1].trim();

                // Check if var_name appears on either side
                if lhs.contains(var_name) || similar_var(lhs, var_name) {
                    return Some(rhs.to_string());
                }
                if rhs.contains(var_name) || similar_var(rhs, var_name) {
                    return Some(lhs.to_string());
                }
            }
        }
    }

    None
}

/// Check if two variable references might be the same
fn similar_var(a: &str, b: &str) -> bool {
    let a_simple = simplify_var_name(a);
    let b_simple = simplify_var_name(b);
    a_simple == b_simple
}

/// Simplify variable names for comparison
fn simplify_var_name(name: &str) -> String {
    name.trim().trim_start_matches('(').trim_end_matches(')').trim().to_string()
}

/// Extract storage slots that are modified in the loop
pub(super) fn extract_modified_storage(stack_diff: &[StackFrame]) -> Vec<U256> {
    let mut slots = Vec::new();

    for frame in stack_diff {
        let solidified = frame.operation.solidify();

        // Look for storage[X] patterns
        if solidified.contains("storage[") {
            if let Some(start) = solidified.find("storage[") {
                if let Some(end) = solidified[start..].find(']') {
                    let slot_str = &solidified[start + 8..start + end];
                    if let Some(hex_str) = slot_str.strip_prefix("0x") {
                        if let Ok(slot) = U256::from_str_radix(hex_str, 16) {
                            slots.push(slot);
                        }
                    } else if let Ok(slot) = U256::from_str_radix(slot_str, 16) {
                        slots.push(slot);
                    }
                }
            }
        }
    }

    slots
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_loop_condition() {
        // Pattern 1: Properly wrapped negation
        assert_eq!(normalize_loop_condition("!(i < arg0)"), "i < arg0");
        assert_eq!(normalize_loop_condition("!(x > 10)"), "x > 10");

        // Pattern 2: Malformed negation with counter value
        assert_eq!(normalize_loop_condition("!0x01 < arg0"), "i < arg0");
        assert_eq!(normalize_loop_condition("!0 < arg0"), "i < arg0");
        assert_eq!(normalize_loop_condition("!1 < arg0"), "i < arg0");

        // Pattern 3: Already correct
        assert_eq!(normalize_loop_condition("i < loops"), "i < loops");
        assert_eq!(normalize_loop_condition("var_a < arg0"), "var_a < arg0");
    }

    #[test]
    fn test_negate_condition() {
        assert_eq!(negate_condition("i < 10"), "i >= 10");
        assert_eq!(negate_condition("i > 10"), "i <= 10");
        assert_eq!(negate_condition("i <= 10"), "i > 10");
        assert_eq!(negate_condition("i >= 10"), "i < 10");
        assert_eq!(negate_condition("i == 10"), "i != 10");
        assert_eq!(negate_condition("i != 10"), "i == 10");
        assert_eq!(negate_condition("!(x)"), "x");
        assert_eq!(negate_condition("foo"), "!(foo)");
    }

    #[test]
    fn test_extract_increment_var() {
        assert_eq!(extract_increment_var("var_a + 0x01"), Some("var_a".to_string()));
        assert_eq!(extract_increment_var("var_a + 1"), Some("var_a".to_string()));
        assert_eq!(extract_increment_var("(i + 0x01)"), Some("i".to_string()));
        assert_eq!(extract_increment_var("var_a - 1"), None);
    }

    #[test]
    fn test_infer_induction_from_condition() {
        // Ascending loop
        let iv = infer_induction_from_condition("i < arg0").unwrap();
        assert_eq!(iv.name, "i");
        assert_eq!(iv.bound, Some("arg0".to_string()));
        assert_eq!(iv.direction, InductionDirection::Ascending);

        // Descending loop
        let iv = infer_induction_from_condition("i > 0").unwrap();
        assert_eq!(iv.name, "i");
        assert_eq!(iv.bound, Some("0".to_string()));
        assert_eq!(iv.direction, InductionDirection::Descending);

        // Should not infer from constants
        assert!(infer_induction_from_condition("0x01 < arg0").is_none());
        assert!(infer_induction_from_condition("10 < 20").is_none());
    }

    #[test]
    fn test_loop_info_to_solidity() {
        let mut info = LoopInfo::new(100, 200, "i < loops".to_string());
        assert_eq!(info.to_solidity(), "while (i < loops) {");

        info.is_bounded = true;
        info.induction_var = Some(InductionVariable {
            name: "i".to_string(),
            init: "0".to_string(),
            step: "+ 1".to_string(),
            bound: Some("loops".to_string()),
            direction: InductionDirection::Ascending,
        });
        assert_eq!(info.to_solidity(), "for (uint256 i = 0; i < loops; i++) {");
    }

    #[test]
    fn test_loop_info_normalizes_condition() {
        // Test that LoopInfo::new normalizes malformed conditions
        let info = LoopInfo::new(100, 200, "!0x01 < arg0".to_string());
        assert_eq!(info.condition, "i < arg0");

        let info = LoopInfo::new(100, 200, "!(i < arg0)".to_string());
        assert_eq!(info.condition, "i < arg0");
    }

    #[test]
    fn test_is_hex_constant() {
        assert!(is_hex_constant("0x01"));
        assert!(is_hex_constant("0xff"));
        assert!(is_hex_constant("0x1234abcd"));
        assert!(!is_hex_constant("0x")); // Too short
        assert!(!is_hex_constant("i"));
        assert!(!is_hex_constant("arg0"));
    }

    #[test]
    fn test_is_decimal_constant() {
        assert!(is_decimal_constant("0"));
        assert!(is_decimal_constant("1"));
        assert!(is_decimal_constant("123"));
        assert!(!is_decimal_constant(""));
        assert!(!is_decimal_constant("i"));
        assert!(!is_decimal_constant("0x01"));
    }

    #[test]
    fn test_counter_name_for_depth() {
        assert_eq!(counter_name_for_depth(0), "i");
        assert_eq!(counter_name_for_depth(1), "j");
        assert_eq!(counter_name_for_depth(2), "k");
        assert_eq!(counter_name_for_depth(3), "l");
        assert_eq!(counter_name_for_depth(4), "m");
        assert_eq!(counter_name_for_depth(5), "n");
        assert_eq!(counter_name_for_depth(6), "idx6");
        assert_eq!(counter_name_for_depth(10), "idx10");
    }

    #[test]
    fn test_set_counter_name_for_depth() {
        // Test renaming with induction variable
        let mut info = LoopInfo::new(100, 200, "!0x01 < arg0".to_string());
        info.is_bounded = true;
        info.induction_var = Some(InductionVariable {
            name: "i".to_string(),
            init: "0".to_string(),
            step: "+ 1".to_string(),
            bound: Some("arg0".to_string()),
            direction: InductionDirection::Ascending,
        });

        // Outer loop (depth 0) should use "i"
        info.set_counter_name_for_depth(0);
        assert_eq!(info.induction_var.as_ref().unwrap().name, "i");
        assert_eq!(info.condition, "i < arg0");

        // Inner loop (depth 1) should use "j"
        let mut inner_info = LoopInfo::new(100, 200, "!0x01 < arg0".to_string());
        inner_info.is_bounded = true;
        inner_info.induction_var = Some(InductionVariable {
            name: "i".to_string(),
            init: "0".to_string(),
            step: "+ 1".to_string(),
            bound: Some("arg0".to_string()),
            direction: InductionDirection::Ascending,
        });
        inner_info.set_counter_name_for_depth(1);
        assert_eq!(inner_info.induction_var.as_ref().unwrap().name, "j");
        assert_eq!(inner_info.condition, "j < arg0");
    }

    #[test]
    fn test_nested_loop_to_solidity() {
        // Outer loop
        let mut outer = LoopInfo::new(100, 200, "!0x01 < arg0".to_string());
        outer.is_bounded = true;
        outer.induction_var = Some(InductionVariable {
            name: "i".to_string(),
            init: "0".to_string(),
            step: "+ 1".to_string(),
            bound: Some("arg0".to_string()),
            direction: InductionDirection::Ascending,
        });
        outer.set_counter_name_for_depth(0);
        assert_eq!(outer.to_solidity(), "for (uint256 i = 0; i < arg0; i++) {");

        // Inner loop
        let mut inner = LoopInfo::new(150, 250, "!0x01 < arg0".to_string());
        inner.is_bounded = true;
        inner.induction_var = Some(InductionVariable {
            name: "i".to_string(),
            init: "0".to_string(),
            step: "+ 1".to_string(),
            bound: Some("arg0".to_string()),
            direction: InductionDirection::Ascending,
        });
        inner.set_counter_name_for_depth(1);
        assert_eq!(inner.to_solidity(), "for (uint256 j = 0; j < arg0; j++) {");
    }

    #[test]
    fn test_is_tautologically_false_condition() {
        // Always false conditions
        assert!(is_tautologically_false_condition("0 > 0x01"));
        assert!(is_tautologically_false_condition("(0 > 0x01)"));
        assert!(is_tautologically_false_condition("0 > 1"));
        assert!(is_tautologically_false_condition("1 > 2"));
        assert!(is_tautologically_false_condition("0 >= 1"));
        assert!(is_tautologically_false_condition("5 < 3"));

        // Valid loop conditions (not always false)
        assert!(!is_tautologically_false_condition("i < arg0"));
        assert!(!is_tautologically_false_condition("0x01 < arg0"));
        assert!(!is_tautologically_false_condition("1 < 2"));
        assert!(!is_tautologically_false_condition("i > 0"));
    }
}
