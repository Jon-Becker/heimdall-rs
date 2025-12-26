use crate::{core::postprocess::PostprocessorState, Error};

/// Extract a var_X pattern from a string, returning the full match if found.
/// E.g., "for (uint256 var_a = 0" -> Some("var_a")
fn extract_loop_var(s: &str) -> Option<&str> {
    // Find "var_" and extract the variable name
    let mut start = 0;
    while let Some(pos) = s[start..].find("var_") {
        let abs_pos = start + pos;
        // Check word boundary before
        if abs_pos > 0 {
            let prev_char = s.as_bytes()[abs_pos - 1];
            if prev_char.is_ascii_alphanumeric() || prev_char == b'_' {
                start = abs_pos + 4;
                continue;
            }
        }
        // Find end of variable name (hex chars only after var_)
        let rest = &s[abs_pos + 4..];
        let end = rest.find(|c: char| !c.is_ascii_hexdigit()).unwrap_or(rest.len());
        if end > 0 {
            return Some(&s[abs_pos..abs_pos + 4 + end]);
        }
        start = abs_pos + 4;
    }
    None
}

/// Check if require statement is tautological (e.g., require(x == x))
fn is_tautological_require_match(line: &str) -> bool {
    // Find "require("
    let Some(req_pos) = line.find("require(") else { return false };
    let after_req = &line[req_pos + 8..];

    // Find " == " operator
    let Some(eq_pos) = after_req.find(" == ") else { return false };

    // Extract LHS (before ==) and RHS (after ==)
    let lhs = after_req[..eq_pos].trim();
    let rhs_start = eq_pos + 4;
    let rhs_rest = &after_req[rhs_start..];

    // RHS ends at comma, paren, or semicolon
    let rhs_end = rhs_rest.find([',', ')', ';']).unwrap_or(rhs_rest.len());
    let rhs = rhs_rest[..rhs_end].trim();

    // Check if LHS == RHS (tautology)
    !lhs.is_empty() && lhs == rhs
}

/// Check if line contains impossible check patterns like require(!0 < x)
fn is_impossible_check(line: &str) -> bool {
    if !line.contains("require(") {
        return false;
    }

    // Pattern 1: require(!0 < x), require(!0x0 < x), etc.
    for negated_zero in ["!0 ", "!0x0 ", "!0x00 ", "!0x01 ", "!1 "] {
        if line.contains(negated_zero) {
            // Check if followed by comparison operator
            if let Some(pos) = line.find(negated_zero) {
                let after = &line[pos + negated_zero.len()..];
                if after.starts_with('<') || after.starts_with('>') {
                    return true;
                }
            }
        }
    }

    // Pattern 2: require(!((0 > x))) or require(!(0 > x))
    if line.contains("!(") || line.contains("! (") {
        // Look for negation wrapping zero comparison
        for zero_val in ["(0 ", "(0x0 ", "(0x00 "] {
            if line.contains(zero_val) {
                return true;
            }
        }
    }

    false
}

/// Check if line is a panic code assignment (var_a = 0x11, memory[x] = 0x11)
fn is_panic_code_assignment(line: &str) -> bool {
    let trimmed = line.trim();

    // Must contain " = " and end with ";"
    let Some(eq_pos) = trimmed.find(" = ") else { return false };
    if !trimmed.ends_with(';') {
        return false;
    }

    let lhs = trimmed[..eq_pos].trim();
    let rhs = trimmed[eq_pos + 3..trimmed.len() - 1].trim();

    // Check LHS is var_X or memory[...]
    let valid_lhs = lhs.starts_with("var_") || (lhs.starts_with("memory[") && lhs.ends_with(']'));

    // Check RHS is panic code
    let is_panic_code = matches!(rhs, "0x11" | "0x12" | "17" | "18");

    valid_lhs && is_panic_code
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
        if let Some(old_var) = extract_loop_var(line) {
            let old_var_owned = old_var.to_string();
            // Store the mapping for later replacement in the loop body
            state.memory_map.insert(old_var_owned.clone(), var_name.to_string());
            *line = line.replace(&old_var_owned, var_name);
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

    // Check for panic patterns (0x4e487b71 selector or Panic() calls)
    if line.contains("0x4e487b71") || line.contains("Panic(") {
        line.clear();
        return Ok(());
    }

    // Check for panic code assignments (memory[0] = 0x11, var_a = 0x11)
    if is_panic_code_assignment(line) {
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

    // Pattern 1: require(x == x) - tautological equality
    is_tautological_require_match(line) || is_impossible_check(line)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_loop_var() {
        assert_eq!(extract_loop_var("for (uint256 var_a = 0"), Some("var_a"));
        assert_eq!(extract_loop_var("var_abc + 1"), Some("var_abc"));
        assert_eq!(extract_loop_var("some_var_a"), None); // not a word boundary
        assert_eq!(extract_loop_var("no match here"), None);
    }

    #[test]
    fn test_is_tautological_require() {
        assert!(is_tautological_require("require(arg0 == arg0);"));
        assert!(is_tautological_require("require(x == x, \"error\");"));
        assert!(!is_tautological_require("require(arg0 == arg1);"));
        assert!(!is_tautological_require("if (x == x) {"));
    }

    #[test]
    fn test_impossible_check() {
        // Should match inverted loop conditions - format 1
        assert!(is_impossible_check("require(!0 < arg0);"));
        assert!(is_impossible_check("require(!0x0 < arg0);"));
        assert!(is_impossible_check("require(!0x00 < arg0);"));
        assert!(is_impossible_check("require(!0x01 < arg0);"));
        assert!(is_impossible_check("require(!1 < arg0);"));

        // Should match format 2: require(!((0 > x)))
        assert!(is_impossible_check("require(!((0 > 0x01)));"));
        assert!(is_impossible_check("require(!(0 > 0x01));"));
        assert!(is_impossible_check("require(!((0x0 >= 1)));"));

        // Should not match normal requires
        assert!(!is_impossible_check("require(arg0 > 0);"));
        assert!(!is_impossible_check("require(x < y);"));
    }

    #[test]
    fn test_panic_code_assignment() {
        // Should match panic code assignments - var format
        assert!(is_panic_code_assignment("var_a = 0x11;"));
        assert!(is_panic_code_assignment("var_b = 0x12;"));
        assert!(is_panic_code_assignment("  var_abc = 17;"));
        assert!(is_panic_code_assignment("var_x = 18;"));

        // Should match panic code assignments - memory format
        assert!(is_panic_code_assignment("memory[0] = 0x11;"));
        assert!(is_panic_code_assignment("memory[0x40] = 0x12;"));
        assert!(is_panic_code_assignment("  memory[0x00] = 17;"));

        // Should not match other assignments
        assert!(!is_panic_code_assignment("var_a = 0x10;"));
        assert!(!is_panic_code_assignment("number = 0x11;"));
        assert!(!is_panic_code_assignment("storage[0] = 0x11;"));
    }
}
