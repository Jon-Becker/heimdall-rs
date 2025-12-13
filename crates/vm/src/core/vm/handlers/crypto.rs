use alloy::primitives::{keccak256, U256};
use eyre::Result;

use crate::core::opcodes::WrappedOpcode;

use super::super::core::VM;

/// SHA3 - Compute Keccak-256 hash
pub fn sha3(vm: &mut VM, operation: WrappedOpcode) -> Result<()> {
    let offset = vm.stack.pop()?.value;
    let size = vm.stack.pop()?.value;

    // Safely convert U256 to usize
    let offset: usize = offset.try_into().unwrap_or(usize::MAX);
    let size: usize = size.try_into().unwrap_or(usize::MAX);

    let data = vm.memory.read(offset, size);
    let result = keccak256(data);

    // consume dynamic gas
    let minimum_word_size = size.div_ceil(32) as u128;
    let gas_cost = 6 * minimum_word_size + vm.memory.expansion_cost(offset, size);
    vm.consume_gas(gas_cost);

    vm.stack.push(U256::from_be_bytes(result.0), operation);
    Ok(())
}
