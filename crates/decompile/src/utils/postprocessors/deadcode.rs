use fancy_regex::Regex;
use hashbrown::{HashMap, HashSet};
use std::sync::LazyLock;

use crate::{core::postprocess::PostprocessorState, interfaces::AnalyzedFunction, Error};

/// Regex to match variable assignments like `var_a = expr;` or `type var_a = expr;`
/// The type prefix must NOT start with "var_" to avoid greedy matching issues.
static VAR_ASSIGNMENT_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:(?!var_)\w+\s+)?(var_[a-z]+)\s*=").unwrap());

/// Regex to match variable usages (var_a, var_b, etc.)
static VAR_USAGE_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\bvar_[a-z]+\b").unwrap());

/// Represents a variable assignment
#[derive(Debug)]
struct Assignment {
    /// Line index in the logic vector
    line_idx: usize,
    /// The variable name being assigned
    var_name: String,
}

/// Removes empty lines from the function logic.
///
/// This pass should run last, after all other postprocessors have completed,
/// to clean up lines that were cleared by other passes.
pub(crate) fn remove_empty_lines(
    function: &mut AnalyzedFunction,
    _state: &mut PostprocessorState,
) -> Result<(), Error> {
    function.logic.retain(|line| !line.trim().is_empty());
    Ok(())
}

/// Eliminates dead variable assignments from the function logic.
///
/// A variable assignment is considered dead if:
/// 1. The variable is never used after the assignment, OR
/// 2. The variable is re-assigned before being used
///
/// This pass runs after all line-by-line postprocessors have completed.
pub(crate) fn eliminate_dead_variables(
    function: &mut AnalyzedFunction,
    _state: &mut PostprocessorState,
) -> Result<(), Error> {
    // Collect all variable assignments
    let assignments = collect_assignments(&function.logic);

    // Build usage information for each line
    let line_usages = collect_usages(&function.logic);

    // Find lines with dead assignments
    let dead_lines = find_dead_assignments(&assignments, &line_usages, function.logic.len());

    // Clear dead lines
    for line_idx in dead_lines {
        function.logic[line_idx].clear();
    }

    Ok(())
}

/// Collect all variable assignments from the logic
fn collect_assignments(logic: &[String]) -> Vec<Assignment> {
    let mut assignments = Vec::new();

    for (line_idx, line) in logic.iter().enumerate() {
        let trimmed = line.trim();

        // Skip lines that have side effects we shouldn't remove
        if has_side_effects(trimmed) {
            continue;
        }

        if let Ok(Some(caps)) = VAR_ASSIGNMENT_REGEX.captures(trimmed) {
            if let Some(var_match) = caps.get(1) {
                assignments.push(Assignment { line_idx, var_name: var_match.as_str().to_string() });
            }
        }
    }

    assignments
}

/// Collect variable usages for each line, returning a map of line_idx -> set of vars used
fn collect_usages(logic: &[String]) -> Vec<HashSet<String>> {
    logic
        .iter()
        .map(|line| {
            let trimmed = line.trim();

            // For assignment lines, only count usages on the RHS
            if let Ok(Some(caps)) = VAR_ASSIGNMENT_REGEX.captures(trimmed) {
                if let Some(var_match) = caps.get(1) {
                    let assigned_var = var_match.as_str();
                    // Find the RHS (everything after the first =)
                    if let Some(eq_pos) = trimmed.find('=') {
                        let rhs = &trimmed[eq_pos + 1..];
                        return VAR_USAGE_REGEX
                            .find_iter(rhs)
                            .filter_map(|m| m.ok())
                            .map(|m| m.as_str().to_string())
                            .filter(|v| v != assigned_var) // Don't count self-references on RHS
                            .collect();
                    }
                }
            }

            // For non-assignment lines, count all variable usages
            VAR_USAGE_REGEX
                .find_iter(trimmed)
                .filter_map(|m| m.ok())
                .map(|m| m.as_str().to_string())
                .collect()
        })
        .collect()
}

/// Check if a line has side effects that shouldn't be removed
fn has_side_effects(line: &str) -> bool {
    // Storage/transient writes (LHS contains storage/store/transient/tstore before =)
    // We need to check if storage access is on the LEFT side of assignment (write)
    // vs on the RIGHT side (read, which has no side effect)
    let is_storage_write = if let Some(eq_pos) = line.find('=') {
        // Check for compound assignment operators - the char before = should not be another
        // operator
        let before_eq = if eq_pos > 0 { line.chars().nth(eq_pos - 1) } else { None };
        let is_simple_assign = !matches!(before_eq, Some('!' | '<' | '>' | '='));
        if is_simple_assign {
            let lhs = &line[..eq_pos];
            lhs.contains("storage") ||
                lhs.contains("store_") ||
                lhs.contains("transient") ||
                lhs.contains("tstore_")
        } else {
            false
        }
    } else {
        false
    };

    is_storage_write ||
    // Events and errors
    line.contains("emit ") ||
    line.contains("revert") ||
    line.contains("require") ||
    line.contains("assert") ||
    // External calls - use patterns that won't match msg.sender
    line.contains(".call(") ||
    line.contains(".call{") ||
    line.contains(".delegatecall(") ||
    line.contains(".staticcall(") ||
    line.contains(".transfer(") ||
    line.contains(".send(") ||
    // Control flow
    line.starts_with("if") ||
    line.starts_with("} else") ||
    line.starts_with("for") ||
    line.starts_with("while") ||
    line.starts_with("return") ||
    line.starts_with('}') ||
    line.starts_with('{') ||
    // Selfdestruct
    line.contains("selfdestruct")
}

/// Find all line indices with dead variable assignments
fn find_dead_assignments(
    assignments: &[Assignment],
    line_usages: &[HashSet<String>],
    total_lines: usize,
) -> HashSet<usize> {
    let mut dead_lines = HashSet::new();

    // Build a map of variable -> list of assignment line indices (in order)
    let mut var_assignments: HashMap<String, Vec<usize>> = HashMap::new();
    for assignment in assignments {
        var_assignments.entry(assignment.var_name.clone()).or_default().push(assignment.line_idx);
    }

    // For each assignment, check if it's dead
    for assignment in assignments {
        let next_assignment_idx = var_assignments
            .get(&assignment.var_name)
            .and_then(|indices| indices.iter().find(|&&idx| idx > assignment.line_idx).copied());

        // The range to check for usages: from assignment+1 to next_assignment (or end)
        let end_idx = next_assignment_idx.unwrap_or(total_lines);

        // Check if the variable is used in any line in this range
        let is_used = (assignment.line_idx + 1..end_idx).any(|idx| {
            line_usages.get(idx).is_some_and(|usages| usages.contains(&assignment.var_name))
        });

        if !is_used {
            dead_lines.insert(assignment.line_idx);
        }
    }

    dead_lines
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_function() -> AnalyzedFunction {
        AnalyzedFunction::new("00000000", false)
    }

    #[test]
    fn test_simple_dead_variable() {
        let mut function = test_function();
        function.logic = vec!["uint256 var_a = 0x01;".to_string(), "return 0x01;".to_string()];

        let mut state = PostprocessorState::default();
        eliminate_dead_variables(&mut function, &mut state).unwrap();

        // var_a is never used, so the assignment should be cleared
        assert!(function.logic[0].is_empty());
        assert!(!function.logic[1].is_empty());
    }

    #[test]
    fn test_overwritten_variable() {
        let mut function = test_function();
        function.logic = vec![
            "address var_a = address(msg.sender);".to_string(),
            "var_a = address(arg0);".to_string(),
            "return var_a;".to_string(),
        ];

        let mut state = PostprocessorState::default();
        eliminate_dead_variables(&mut function, &mut state).unwrap();

        // First assignment is dead (overwritten before use)
        assert!(function.logic[0].is_empty());
        // Second assignment is live (used in return)
        assert!(!function.logic[1].is_empty());
        assert!(!function.logic[2].is_empty());
    }

    #[test]
    fn test_used_variable() {
        let mut function = test_function();
        function.logic = vec!["uint256 var_a = arg0;".to_string(), "return var_a;".to_string()];

        let mut state = PostprocessorState::default();
        eliminate_dead_variables(&mut function, &mut state).unwrap();

        // var_a is used, so it should not be cleared
        assert!(!function.logic[0].is_empty());
        assert!(!function.logic[1].is_empty());
    }

    #[test]
    fn test_storage_side_effect() {
        let mut function = test_function();
        function.logic = vec!["storage[0x00] = arg0;".to_string(), "return 0x01;".to_string()];

        let mut state = PostprocessorState::default();
        eliminate_dead_variables(&mut function, &mut state).unwrap();

        // Storage writes should never be removed
        assert!(!function.logic[0].is_empty());
    }

    #[test]
    fn test_variable_used_in_another_assignment() {
        let mut function = test_function();
        function.logic = vec![
            "uint256 var_a = arg0;".to_string(),
            "uint256 var_b = var_a;".to_string(),
            "return var_b;".to_string(),
        ];

        let mut state = PostprocessorState::default();
        eliminate_dead_variables(&mut function, &mut state).unwrap();

        // var_a is used in var_b's assignment, so it's live
        assert!(!function.logic[0].is_empty());
        // var_b is used in return, so it's live
        assert!(!function.logic[1].is_empty());
    }
}
