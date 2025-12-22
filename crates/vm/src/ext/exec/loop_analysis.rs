use alloy::primitives::U256;

use crate::core::{stack::StackFrame, vm::State};

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

    /// Operations captured from one iteration of the loop body
    pub body_operations: Vec<State>,

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
#[derive(Clone, Debug, PartialEq, Default)]
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
        Self {
            header_pc,
            condition_pc,
            exit_condition: negate_condition(&condition),
            condition,
            induction_var: None,
            body_operations: Vec::new(),
            is_bounded: false,
            modified_storage: Vec::new(),
            modified_memory: Vec::new(),
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

/// Attempt to detect an induction variable from the stack diff
pub(super) fn detect_induction_variable(
    stack_diff: &[StackFrame],
    jump_condition: &Option<String>,
) -> Option<InductionVariable> {
    // Look for increment/decrement patterns in the stack diff
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

    None
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
}
