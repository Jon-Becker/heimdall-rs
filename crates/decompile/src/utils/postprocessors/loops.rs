use fancy_regex::Regex;
use lazy_static::lazy_static;

use crate::{core::postprocess::PostprocessorState, Error};

lazy_static! {
    // Match loop counter variable patterns like "var_a", "var_bc", etc.
    static ref LOOP_VAR_PATTERN: Regex = Regex::new(r"\bvar_([a-f0-9]+)\b").unwrap();

    // Match tautological requires like "require(arg0 == arg0)"
    static ref TAUTOLOGICAL_REQUIRE: Regex = Regex::new(
        r"require\s*\(\s*(\w+)\s*==\s*\1\s*[,)]"
    ).unwrap();

    // Match impossible checks like "require(!0 < x)" or "require(!((0 > x)))"
    // These come from inverted loop conditions being misinterpreted as require statements
    // Format 1: require(!0 < x) - direct negation of zero
    // Format 2: require(!((0 > x))) - negation wrapping comparison
    static ref IMPOSSIBLE_CHECK: Regex = Regex::new(
        r"require\s*\(\s*(!0|!0x0|!0x00|!0x01|!1)\s*(<|<=|>|>=)|require\s*\(\s*!\s*\(\s*\(?\s*(0|0x0|0x00)\s*(>|>=|<|<=)"
    ).unwrap();

    // Match panic code assignments - both before and after variable renaming:
    // - Before: "memory[0] = 0x11;" or "memory[0x40] = 0x11;"
    // - After: "var_a = 0x11;"
    static ref PANIC_CODE_ASSIGNMENT: Regex = Regex::new(
        r"^\s*(var_[a-zA-Z0-9_]+|memory\[[^\]]+\])\s*=\s*(0x11|0x12|17|18)\s*;"
    ).unwrap();

    // Match Solidity 0.8+ panic codes
    static ref PANIC_PATTERN: Regex = Regex::new(
        r"0x4e487b71|Panic\s*\("
    ).unwrap();
}

/// Postprocess loop constructs for cleaner output - renames loop variables
pub(crate) fn loop_postprocessor(
    line: &mut String,
    state: &mut PostprocessorState,
) -> Result<(), Error> {
    // Track loop variable counter for renaming
    let loop_counter = state.memory_map.len();

    // Check if this is a for/while loop declaration
    if line.starts_with("for (") || line.starts_with("while (") {
        // Get a clean variable name based on nesting
        let var_name = match loop_counter {
            0 => "i",
            1 => "j",
            2 => "k",
            3 => "l",
            _ => return Ok(()), // Don't rename beyond 4 levels
        };

        // Find and replace the first loop variable
        if let Ok(Some(caps)) = LOOP_VAR_PATTERN.captures(line) {
            if let Some(old_var_match) = caps.get(0) {
                let old_var = old_var_match.as_str();
                // Store the mapping for later replacement in the loop body
                state
                    .memory_map
                    .insert(old_var.to_string(), var_name.to_string());
                *line = line.replace(old_var, var_name);
            }
        }
    }

    // Apply stored variable mappings to this line
    for (old_var, new_var) in &state.memory_map {
        if line.contains(old_var.as_str()) {
            *line = line.replace(old_var.as_str(), new_var.as_str());
        }
    }

    Ok(())
}

/// Remove redundant overflow checks from loop bodies
pub(crate) fn remove_overflow_checks(
    line: &mut String,
    _state: &mut PostprocessorState,
) -> Result<(), Error> {
    // Check for tautological requires
    if is_tautological_require(line) {
        line.clear();
        return Ok(());
    }

    // Check for panic patterns
    if PANIC_PATTERN.is_match(line).unwrap_or(false) {
        line.clear();
        return Ok(());
    }

    // Check for panic code assignments (memory[0] = 0x11, var_a = 0x11)
    if PANIC_CODE_ASSIGNMENT.is_match(line).unwrap_or(false) {
        line.clear();
        return Ok(());
    }

    Ok(())
}

/// Check if a require statement is always true (tautological)
fn is_tautological_require(line: &str) -> bool {
    if !line.contains("require(") {
        return false;
    }

    // Pattern: require(x == x)
    if TAUTOLOGICAL_REQUIRE.is_match(line).unwrap_or(false) {
        return true;
    }

    // Pattern: require(!0 < x) which is always true for unsigned
    if IMPOSSIBLE_CHECK.is_match(line).unwrap_or(false) {
        return true;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_tautological_require() {
        assert!(is_tautological_require("require(arg0 == arg0);"));
        assert!(is_tautological_require("require(x == x, \"error\");"));
        assert!(!is_tautological_require("require(arg0 == arg1);"));
        assert!(!is_tautological_require("if (x == x) {"));
    }

    #[test]
    fn test_impossible_check_regex() {
        // Should match inverted loop conditions - format 1
        assert!(IMPOSSIBLE_CHECK.is_match("require(!0 < arg0);").unwrap());
        assert!(IMPOSSIBLE_CHECK.is_match("require(!0x0 < arg0);").unwrap());
        assert!(IMPOSSIBLE_CHECK.is_match("require(!0x00 < arg0);").unwrap());
        assert!(IMPOSSIBLE_CHECK.is_match("require(!0x01 < arg0);").unwrap());
        assert!(IMPOSSIBLE_CHECK.is_match("require(!1 <= arg0);").unwrap());

        // Should match format 2: require(!((0 > x)))
        assert!(IMPOSSIBLE_CHECK.is_match("require(!((0 > 0x01)));").unwrap());
        assert!(IMPOSSIBLE_CHECK.is_match("require(!(0 > 0x01));").unwrap());
        assert!(IMPOSSIBLE_CHECK.is_match("require(!((0x0 >= 1)));").unwrap());

        // Should not match normal requires
        assert!(!IMPOSSIBLE_CHECK.is_match("require(arg0 > 0);").unwrap());
        assert!(!IMPOSSIBLE_CHECK.is_match("require(x < y);").unwrap());
    }

    #[test]
    fn test_panic_code_assignment_regex() {
        // Should match panic code assignments - var format
        assert!(PANIC_CODE_ASSIGNMENT.is_match("var_a = 0x11;").unwrap());
        assert!(PANIC_CODE_ASSIGNMENT.is_match("var_b = 0x12;").unwrap());
        assert!(PANIC_CODE_ASSIGNMENT.is_match("  var_abc = 17;").unwrap());
        assert!(PANIC_CODE_ASSIGNMENT.is_match("var_x = 18;").unwrap());

        // Should match panic code assignments - memory format
        assert!(PANIC_CODE_ASSIGNMENT.is_match("memory[0] = 0x11;").unwrap());
        assert!(PANIC_CODE_ASSIGNMENT.is_match("memory[0x40] = 0x12;").unwrap());
        assert!(PANIC_CODE_ASSIGNMENT.is_match("  memory[0x00] = 17;").unwrap());

        // Should not match other assignments
        assert!(!PANIC_CODE_ASSIGNMENT.is_match("var_a = 0x10;").unwrap());
        assert!(!PANIC_CODE_ASSIGNMENT.is_match("number = 0x11;").unwrap());
        assert!(!PANIC_CODE_ASSIGNMENT.is_match("storage[0] = 0x11;").unwrap());
    }
}
