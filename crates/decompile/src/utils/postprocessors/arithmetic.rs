use eyre::{eyre, OptionExt};
use heimdall_common::{
    ether::{
        evm::core::types::{byte_size_to_type, find_cast},
        lexers::cleanup::simplify_parentheses,
    },
    utils::strings::{find_balanced_encapsulator, find_balanced_encapsulator_backwards},
};

use crate::{
    core::postprocess::PostprocessorState,
    utils::constants::{
        AND_BITMASK_REGEX, AND_BITMASK_REGEX_2, DIV_BY_ONE_REGEX, NON_ZERO_BYTE_REGEX,
    },
    Error,
};

/// Handles simplifying arithmetic operations. For example:
/// - `x + 0` would become `x`
/// - `x * 1` would become `x`
/// - `x - 0` would become `x`
/// - `x / 1` would become `x`
/// - `!!x` would become `x`
///
/// Additionally, this postprocessor will simplify parentheses within the
/// arithmetic operations.
pub fn arithmetic_postprocessor(
    line: &mut String,
    state: &mut PostprocessorState,
) -> Result<(), Error> {
    // 1. Simplify parentheses

    // 2. Simplify arithmetic operations
    while let Some(negation) = line.find("!!") {
        line.replace_range(negation..negation + 2, "");
    }

    *line = simplify_parentheses(&line, 0).unwrap_or(line.clone());

    Ok(())
}

#[cfg(test)]
mod tests {}
