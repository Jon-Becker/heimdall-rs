use alloy::primitives::U256;
use eyre::Result;

use crate::core::opcodes::WrappedOpcode;

use super::super::core::VM;

/// MLOAD - Load word from memory
pub fn mload(vm: &mut VM, operation: WrappedOpcode) -> Result<()> {
    let i = vm.stack.pop()?.value;
    let i: usize = i.try_into().unwrap_or(usize::MAX);

    let result = U256::from_be_slice(vm.memory.read(i, 32).as_slice());

    // consume dynamic gas
    let gas_cost = vm.memory.expansion_cost(i, 32);
    vm.consume_gas(gas_cost);

    vm.stack.push(result, operation);
    Ok(())
}

/// MSTORE - Save word to memory
pub fn mstore(vm: &mut VM, operation: WrappedOpcode) -> Result<()> {
    let offset = vm.stack.pop()?.value;
    let value = vm.stack.pop()?.value;

    // Safely convert U256 to usize
    let offset: usize = offset.try_into().unwrap_or(usize::MAX);

    // consume dynamic gas
    let gas_cost = vm.memory.expansion_cost(offset, 32);
    vm.consume_gas(gas_cost);

    vm.memory.store_with_opcode(
        offset,
        32,
        &value.to_be_bytes_vec(),
        #[cfg(feature = "experimental")]
        operation,
    );
    Ok(())
}

/// MSTORE8 - Save byte to memory
pub fn mstore8(vm: &mut VM, operation: WrappedOpcode) -> Result<()> {
    let offset = vm.stack.pop()?.value;
    let value = vm.stack.pop()?.value;

    // Safely convert U256 to usize
    let offset: usize = offset.try_into().unwrap_or(usize::MAX);

    // consume dynamic gas
    let gas_cost = vm.memory.expansion_cost(offset, 1);
    vm.consume_gas(gas_cost);

    vm.memory.store_with_opcode(
        offset,
        1,
        &[value.to_be_bytes_vec()[31]],
        #[cfg(feature = "experimental")]
        operation,
    );
    Ok(())
}

/// MSIZE - Get the size of active memory in bytes
pub fn msize(vm: &mut VM, operation: WrappedOpcode) -> Result<()> {
    vm.stack.push(U256::from(vm.memory.size()), operation);
    Ok(())
}

/// MCOPY - Copy memory areas
pub fn mcopy(vm: &mut VM, operation: WrappedOpcode) -> Result<()> {
    let dest_offset = vm.stack.pop()?.value;
    let offset = vm.stack.pop()?.value;
    let size = vm.stack.pop()?.value;

    let dest_offset: usize = dest_offset.try_into().unwrap_or(u128::MAX as usize);
    let offset: usize = offset.try_into().unwrap_or(u128::MAX as usize);
    let memory_size: usize = vm.memory.size().try_into().expect("failed to convert u128 to usize");
    let size: usize = size.try_into().unwrap_or(memory_size).min(memory_size);

    let value = VM::safe_copy_data(&vm.memory.memory, offset, size);

    // consume dynamic gas
    let minimum_word_size = size.div_ceil(32) as u128;
    let gas_cost = 3 * minimum_word_size + vm.memory.expansion_cost(offset, size);
    vm.consume_gas(gas_cost);

    vm.memory.store_with_opcode(
        dest_offset,
        size,
        &value,
        #[cfg(feature = "experimental")]
        operation,
    );
    Ok(())
}
