use crate::core::stack::StackFrame;

/// Check if the stack diff and condition show evidence of iteration.
/// A real loop must have:
/// 1. A non-empty stack diff (something is changing)
/// 2. The diff shows meaningful iteration patterns OR
/// 3. The condition is NOT a storage-to-argument comparison (balance check)
///
/// This function is conservative - it returns false for patterns that look like
/// `require()` checks (storage compared to function arguments).
pub(crate) fn stack_diff_shows_iteration(stack_diff: &[StackFrame], condition: &str) -> bool {
    // Empty diff means no iteration - the stack is identical
    if stack_diff.is_empty() {
        return false;
    }

    // First, check if the condition looks like a balance/require check
    // These are NOT loops regardless of stack diff
    if looks_like_require_check(condition) {
        return false;
    }

    // Look for increment/decrement patterns in the stack diff operations
    for frame in stack_diff {
        let solidified = frame.operation.solidify();

        // Check for common loop counter patterns in the operation expression
        if solidified.contains(" + 0x01")
            || solidified.contains(" + 1)")
            || solidified.contains(" - 0x01")
            || solidified.contains(" - 1)")
            || solidified.contains(" + 0x20")
            || solidified.contains(" + 32)")
            || solidified.contains(" - 0x20")
            || solidified.contains(" - 32)")
        {
            return true;
        }
    }

    // If we have a non-empty diff but no clear increment pattern,
    // check if the condition uses a simple counter variable
    if condition_has_simple_counter(condition) {
        return true;
    }

    false
}

/// Check if a condition looks like a require/balance check.
/// These patterns compare storage values to function arguments and are NOT loops.
fn looks_like_require_check(condition: &str) -> bool {
    // Strip outer parens and negations to get to the core comparison
    let inner = strip_negations_and_parens(condition);

    // Check if it's a storage-to-argument comparison
    for op in [" < ", " > ", " <= ", " >= "] {
        if let Some(pos) = inner.find(op) {
            let lhs = inner[..pos].trim();
            let rhs = inner[pos + op.len()..].trim();

            // storage[...] compared to argN is a balance check
            let lhs_storage = lhs.contains("storage[");
            let rhs_storage = rhs.contains("storage[");
            let lhs_arg = is_arg_ref(lhs);
            let rhs_arg = is_arg_ref(rhs);

            if (lhs_storage && rhs_arg) || (lhs_arg && rhs_storage) {
                return true;
            }
        }
    }

    false
}

/// Check if a condition uses a simple counter variable pattern.
/// Loop conditions like "i < length" or "0x20 < memory[...].length" are valid.
/// Rejects constant-only comparisons like "0 > 0x01" (overflow checks).
fn condition_has_simple_counter(condition: &str) -> bool {
    let inner = strip_negations_and_parens(condition);

    // Look for patterns where one side is a simple counter value
    // Patterns: "counter < limit", "counter <= limit", "limit > counter", "limit >= counter"
    for op in [" < ", " <= ", " > ", " >= "] {
        if let Some(pos) = inner.find(op) {
            let lhs = inner[..pos].trim();
            let rhs = inner[pos + op.len()..].trim();

            // For < and <=, the counter is on the left
            // For > and >=, the counter is on the right
            let (counter_side, limit_side) = if op.contains('<') {
                (lhs, rhs)
            } else {
                (rhs, lhs)
            };

            // CRITICAL: If BOTH sides are constants, this is NOT a loop
            // (it's likely an overflow check like "0 > 0x01")
            // A real loop condition needs at least one variable (counter or bound)
            if is_small_constant(counter_side) && is_small_constant(limit_side) {
                continue;
            }

            // Counter should be simple (hex constant, decimal, or simple variable)
            // and NOT a storage access
            if !counter_side.contains("storage[") && !counter_side.contains("keccak") {
                // Check if it's a small constant (likely a counter) or simple var
                if is_small_constant(counter_side) || is_simple_var(counter_side) {
                    return true;
                }
            }
        }
    }

    false
}

/// Strip leading negations and outer parentheses from a condition
fn strip_negations_and_parens(condition: &str) -> &str {
    let mut s = condition.trim();
    loop {
        let prev = s;
        if s.starts_with('!') {
            s = s[1..].trim();
        }
        if s.starts_with('(') && s.ends_with(')') {
            // Check if parens are balanced
            let inner = &s[1..s.len() - 1];
            if inner.chars().filter(|&c| c == '(').count()
                == inner.chars().filter(|&c| c == ')').count()
            {
                s = inner.trim();
            } else {
                break;
            }
        }
        if s == prev {
            break;
        }
    }
    s
}

/// Check if expression is a function argument reference (argN)
fn is_arg_ref(s: &str) -> bool {
    let trimmed = s.trim().trim_start_matches('(').trim_end_matches(')').trim();
    if let Some(rest) = trimmed.strip_prefix("arg") {
        return rest.chars().all(|c| c.is_ascii_digit()) && !rest.is_empty();
    }
    false
}

/// Check if a string is a small constant (likely a loop counter value)
fn is_small_constant(s: &str) -> bool {
    let trimmed = s.trim().trim_start_matches('(').trim_end_matches(')').trim();
    // Hex constants like 0x20, 0x40, 0x00
    if let Some(hex) = trimmed.strip_prefix("0x") {
        if let Ok(val) = u64::from_str_radix(hex, 16) {
            return val <= 0x1000; // Reasonable loop counter range
        }
    }
    // Decimal constants
    if let Ok(val) = trimmed.parse::<u64>() {
        return val <= 4096;
    }
    false
}

/// Check if expression looks like a simple variable (not complex expression)
fn is_simple_var(s: &str) -> bool {
    let trimmed = s.trim().trim_start_matches('(').trim_end_matches(')').trim();
    // Simple patterns: i, j, var_a, memory[0x40], etc. but not complex expressions
    !trimmed.contains(" + ")
        && !trimmed.contains(" - ")
        && !trimmed.contains(" * ")
        && !trimmed.contains(" / ")
        && !trimmed.contains("storage[")
        && !trimmed.contains("keccak")
}

/// Check if a condition is tautologically true (e.g., "arg0 == arg0", "X == (address(X))").
/// These create infinite loops and should be skipped as invalid loop conditions.
/// Also handles bitmask patterns like "X == (X & 0xff...ff)" which are type-check equivalents.
pub(crate) fn is_tautologically_true_condition(condition: &str) -> bool {
    let mut trimmed = condition.trim();

    // Remove leading negation - if the inner condition is tautologically true,
    // then the negation makes it tautologically false (which is also invalid)
    let negated = trimmed.starts_with('!');
    if negated {
        trimmed = trimmed[1..].trim();
    }

    // Strip all outer parentheses
    while trimmed.starts_with('(') && trimmed.ends_with(')') {
        // Check for balanced parentheses before stripping
        let inner = &trimmed[1..trimmed.len() - 1];
        if is_balanced_parens(inner) {
            trimmed = inner.trim();
        } else {
            break;
        }
    }

    // Check for X == X patterns (tautologically true equality)
    if let Some(pos) = trimmed.find(" == ") {
        let lhs = trimmed[..pos].trim();
        let rhs = trimmed[pos + 4..].trim();

        // Normalize both sides and compare
        let lhs_normalized = normalize_for_comparison(lhs);
        let rhs_normalized = normalize_for_comparison(rhs);

        if lhs_normalized == rhs_normalized && !lhs_normalized.is_empty() {
            return true;
        }
    }

    // Check for X != X patterns (tautologically false inequality, but still invalid loop condition)
    if let Some(pos) = trimmed.find(" != ") {
        let lhs = trimmed[..pos].trim();
        let rhs = trimmed[pos + 4..].trim();

        let lhs_normalized = normalize_for_comparison(lhs);
        let rhs_normalized = normalize_for_comparison(rhs);

        if lhs_normalized == rhs_normalized && !lhs_normalized.is_empty() {
            return true;
        }
    }

    false
}

/// Normalize an expression for comparison by stripping type casts, bitmasks, and parentheses.
/// For example: "address(arg0)" -> "arg0", "(arg0)" -> "arg0"
/// Also handles bitmask patterns like "(arg0) & (0xff...ff)" which are equivalent to type casts.
fn normalize_for_comparison(expr: &str) -> String {
    let mut result = expr.trim().to_string();

    // Strip outer parentheses
    while result.starts_with('(') && result.ends_with(')') {
        let inner = &result[1..result.len() - 1];
        if is_balanced_parens(inner) {
            result = inner.trim().to_string();
        } else {
            break;
        }
    }

    // Strip common Solidity type casts: address(...), uint256(...), etc.
    let type_casts = [
        "address(",
        "uint256(",
        "uint128(",
        "uint96(",
        "uint64(",
        "uint32(",
        "uint16(",
        "uint8(",
        "int256(",
        "int128(",
        "int64(",
        "int32(",
        "int16(",
        "int8(",
        "bytes32(",
        "bytes20(",
        "bytes4(",
        "bool(",
    ];

    for cast in type_casts {
        if result.starts_with(cast) && result.ends_with(')') {
            let inner = &result[cast.len()..result.len() - 1];
            if is_balanced_parens(inner) {
                result = inner.trim().to_string();
                // Recursively normalize in case of nested casts
                return normalize_for_comparison(&result);
            }
        }
    }

    // Strip bitmask patterns like "(X) & (0xff...ff)" which are type-cast equivalents
    // These are used by Solidity to ensure values fit in certain types
    // Common masks:
    // - 0xffffffffffffffffffffffffffffffffffffffff (160 bits = address)
    // - 0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff (256 bits)
    // - 0xff (8 bits), 0xffff (16 bits), etc.
    if let Some(and_pos) = result.find(" & ") {
        let lhs = result[..and_pos].trim();
        let rhs = result[and_pos + 3..].trim();

        // Check if rhs is a bitmask (all f's hex value)
        if is_bitmask(rhs) {
            // Return the normalized lhs (the actual value being masked)
            return normalize_for_comparison(lhs);
        }
        // Also check if lhs is the mask and rhs is the value
        if is_bitmask(lhs) {
            return normalize_for_comparison(rhs);
        }
    }

    result
}

/// Check if a string represents a bitmask (0xff...ff pattern)
fn is_bitmask(s: &str) -> bool {
    let mut trimmed = s.trim();

    // Strip parentheses
    while trimmed.starts_with('(') && trimmed.ends_with(')') {
        let inner = &trimmed[1..trimmed.len() - 1];
        if is_balanced_parens(inner) {
            trimmed = inner.trim();
        } else {
            break;
        }
    }

    // Check for hex pattern 0xff...ff
    if let Some(hex_str) = trimmed.strip_prefix("0x") {
        // Must be non-empty and all 'f' characters (case insensitive)
        !hex_str.is_empty() && hex_str.chars().all(|c| c == 'f' || c == 'F')
    } else {
        false
    }
}

/// Check if parentheses are balanced in a string
fn is_balanced_parens(s: &str) -> bool {
    let mut depth = 0;
    for c in s.chars() {
        match c {
            '(' => depth += 1,
            ')' => {
                if depth == 0 {
                    return false;
                }
                depth -= 1;
            }
            _ => {}
        }
    }
    depth == 0
}

/// Check if a condition is tautologically false (e.g., "0 > 1", "(0 > 0x01)").
/// These cannot be valid loop conditions and should be skipped.
/// NOTE: We do NOT strip leading negation here because `!(0 > 1)` = TRUE, which is valid.
pub(crate) fn is_tautologically_false_condition(condition: &str) -> bool {
    // Strip outer whitespace first
    let mut trimmed = condition.trim();

    // If condition starts with negation, it's NOT tautologically false
    // because !(false) = true, which is a valid (though potentially infinite) loop
    if trimmed.starts_with('!') {
        return false;
    }

    // Strip all outer parentheses (could be nested like "((...))")
    while trimmed.starts_with('(') && trimmed.ends_with(')') {
        let inner = &trimmed[1..trimmed.len() - 1];
        if is_balanced_parens(inner) {
            trimmed = inner.trim();
        } else {
            break;
        }
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

    /// Detected induction variable name, if any
    pub induction_var: Option<InductionVariable>,

    /// Whether this appears to be a bounded loop (for) vs unbounded (while)
    pub is_bounded: bool,
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
        Self {
            header_pc,
            condition_pc,
            condition: normalize_loop_condition(&condition),
            induction_var: None,
            is_bounded: false,
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
            // Only clone/allocate if names differ
            if iv.name != counter_name.as_ref() {
                // Update the condition to use the new name
                if self.condition.contains(&iv.name) {
                    self.condition = self.condition.replace(&iv.name, counter_name.as_ref());
                }
                iv.name = counter_name.into_owned();
            }
        } else {
            // Even without an induction variable, update the condition if it uses "i"
            if self.condition.starts_with("i ") || self.condition.contains(" i ") {
                let replacement = format!("{} ", counter_name);
                self.condition = self.condition.replacen("i ", &replacement, 1);
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

/// Counter names for common nesting depths (avoids allocation for depths 0-9)
const COUNTER_NAMES: [&str; 10] = ["i", "j", "k", "l", "m", "n", "idx6", "idx7", "idx8", "idx9"];

/// Generate a counter variable name for the given nesting depth.
///
/// Returns: i, j, k, l, m, n for depths 0-5, then idx6, idx7, etc.
/// For depths 0-9, returns a static string (no allocation).
fn counter_name_for_depth(depth: usize) -> std::borrow::Cow<'static, str> {
    if depth < COUNTER_NAMES.len() {
        std::borrow::Cow::Borrowed(COUNTER_NAMES[depth])
    } else {
        std::borrow::Cow::Owned(format!("idx{}", depth))
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
        assert_eq!(counter_name_for_depth(0).as_ref(), "i");
        assert_eq!(counter_name_for_depth(1).as_ref(), "j");
        assert_eq!(counter_name_for_depth(2).as_ref(), "k");
        assert_eq!(counter_name_for_depth(3).as_ref(), "l");
        assert_eq!(counter_name_for_depth(4).as_ref(), "m");
        assert_eq!(counter_name_for_depth(5).as_ref(), "n");
        assert_eq!(counter_name_for_depth(6).as_ref(), "idx6");
        assert_eq!(counter_name_for_depth(10).as_ref(), "idx10");
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

    #[test]
    fn test_is_tautologically_true_condition() {
        // Always true conditions (X == X patterns)
        assert!(is_tautologically_true_condition("arg0 == arg0"));
        assert!(is_tautologically_true_condition("arg1 == arg1"));
        assert!(is_tautologically_true_condition("(arg0 == arg0)"));
        assert!(is_tautologically_true_condition("arg0 == (arg0)"));
        assert!(is_tautologically_true_condition("(arg0) == arg0"));

        // With type casts (should still be detected)
        assert!(is_tautologically_true_condition("arg0 == address(arg0)"));
        assert!(is_tautologically_true_condition("address(arg0) == arg0"));
        assert!(is_tautologically_true_condition("arg1 == (address(arg1))"));
        assert!(is_tautologically_true_condition("uint256(arg0) == arg0"));

        // X != X patterns (always false, but still invalid loop condition)
        assert!(is_tautologically_true_condition("arg0 != arg0"));
        assert!(is_tautologically_true_condition("arg1 != (address(arg1))"));

        // Valid loop conditions (not tautological)
        assert!(!is_tautologically_true_condition("i < arg0"));
        assert!(!is_tautologically_true_condition("arg0 == arg1"));
        assert!(!is_tautologically_true_condition("arg0 < arg0")); // This is false, but not X == X
        assert!(!is_tautologically_true_condition("i == 0"));
        assert!(!is_tautologically_true_condition("arg0 == 0x01"));
    }

    #[test]
    fn test_normalize_for_comparison() {
        // Simple expressions
        assert_eq!(normalize_for_comparison("arg0"), "arg0");
        assert_eq!(normalize_for_comparison("(arg0)"), "arg0");
        assert_eq!(normalize_for_comparison("((arg0))"), "arg0");

        // Type casts
        assert_eq!(normalize_for_comparison("address(arg0)"), "arg0");
        assert_eq!(normalize_for_comparison("uint256(arg0)"), "arg0");
        assert_eq!(normalize_for_comparison("(address(arg0))"), "arg0");

        // Nested type casts
        assert_eq!(normalize_for_comparison("address(uint256(arg0))"), "arg0");

        // Bitmask patterns (equivalent to type casts)
        assert_eq!(
            normalize_for_comparison("(arg1) & (0xffffffffffffffffffffffffffffffffffffffff)"),
            "arg1"
        );
        assert_eq!(
            normalize_for_comparison("((arg1) & (0xffffffffffffffffffffffffffffffffffffffff))"),
            "arg1"
        );
        assert_eq!(normalize_for_comparison("arg0 & 0xff"), "arg0");
        assert_eq!(normalize_for_comparison("(arg0) & (0xffff)"), "arg0");
    }

    #[test]
    fn test_is_bitmask() {
        // Valid bitmasks
        assert!(is_bitmask("0xff"));
        assert!(is_bitmask("0xffff"));
        assert!(is_bitmask("0xffffffffffffffffffffffffffffffffffffffff"));
        assert!(is_bitmask("(0xff)"));
        assert!(is_bitmask("((0xffff))"));
        assert!(is_bitmask("0xFFFF")); // uppercase

        // Invalid bitmasks
        assert!(!is_bitmask("0x01"));
        assert!(!is_bitmask("0xfe"));
        assert!(!is_bitmask("arg0"));
        assert!(!is_bitmask("0x"));
        assert!(!is_bitmask(""));
    }

    #[test]
    fn test_is_tautologically_true_with_bitmask() {
        // Bitmask patterns that are tautologically true
        assert!(is_tautologically_true_condition(
            "arg1 == ((arg1) & (0xffffffffffffffffffffffffffffffffffffffff))"
        ));
        assert!(is_tautologically_true_condition("arg0 == (arg0 & 0xff)"));
        assert!(is_tautologically_true_condition(
            "((arg1) & (0xffffffffffffffffffffffffffffffffffffffff)) == arg1"
        ));
    }

    #[test]
    fn test_is_balanced_parens() {
        assert!(is_balanced_parens(""));
        assert!(is_balanced_parens("arg0"));
        assert!(is_balanced_parens("(arg0)"));
        assert!(is_balanced_parens("((arg0))"));
        assert!(is_balanced_parens("a + (b + c)"));
        assert!(!is_balanced_parens("(arg0"));
        assert!(!is_balanced_parens("arg0)"));
        assert!(!is_balanced_parens("((arg0)"));
    }
}
