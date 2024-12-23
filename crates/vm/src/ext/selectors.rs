use hashbrown::HashMap;

use crate::core::vm::Vm;

/// find all function selectors in the given EVM bytecode.
pub fn find_function_selectors(evm: &Vm) -> HashMap<String, u128> {
    todo!();
}

#[cfg(test)]
mod tests {

    #[test]
    fn test_find_function_selectors() {}

    #[test]
    fn test_find_function_selectors_vyper() {}
}
