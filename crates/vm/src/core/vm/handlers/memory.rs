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
pub fn mstore(
    vm: &mut VM,
    #[cfg(feature = "experimental")] operation: WrappedOpcode,
) -> Result<()> {
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
pub fn mstore8(
    vm: &mut VM,
    #[cfg(feature = "experimental")] operation: WrappedOpcode,
) -> Result<()> {
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

/// MCOPY - Copy memory areas (EIP-5656; stack top: length, source offset, dest offset)
pub fn mcopy(vm: &mut VM, #[cfg(feature = "experimental")] operation: WrappedOpcode) -> Result<()> {
    let size_word = vm.stack.pop()?.value;
    let src_offset = vm.stack.pop()?.value;
    let dest_offset = vm.stack.pop()?.value;

    let dest_offset: usize = dest_offset.try_into().unwrap_or(usize::MAX);
    let offset: usize = src_offset.try_into().unwrap_or(usize::MAX);
    let size: usize = size_word.try_into().unwrap_or(usize::MAX);

    let value = VM::safe_copy_data(&vm.memory.memory, offset, size);

    // consume dynamic gas — memory expansion covers source and destination ranges (EIP-5656)
    let minimum_word_size = size.div_ceil(32) as u128;
    let expand_src = vm.memory.expansion_cost(offset, size);
    let expand_dest = vm.memory.expansion_cost(dest_offset, size);
    let gas_cost = 3 * minimum_word_size + expand_src.max(expand_dest);
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
