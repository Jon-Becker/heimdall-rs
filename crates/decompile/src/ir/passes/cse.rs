use crate::{ir::types::Function, Error};

/// Common subexpression elimination - deduplicate repeated expressions
pub fn run(func: Function) -> Result<Function, Error> {
    // TODO: Implement CSE
    // 1. Find common subexpressions within blocks
    // 2. Replace duplicates with references to first occurrence
    // 3. Be careful about side effects
    Ok(func)
}