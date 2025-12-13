use alloy::primitives::U256;
use eyre::Result;

use crate::core::opcodes::WrappedOpcode;

use super::super::core::VM;

/// ADDRESS - Get address of currently executing account
pub fn address(vm: &mut VM, operation: WrappedOpcode) -> Result<()> {
    vm.stack.push(VM::address_to_u256(&vm.address), operation);
    Ok(())
}

/// BALANCE - Get balance of the given account
pub fn balance(vm: &mut VM, operation: WrappedOpcode) -> Result<()> {
    let address = vm.stack.pop()?.value;

    // consume dynamic gas
    if !vm.address_access_set.contains(&address) {
        vm.consume_gas(2600);
        vm.address_access_set.insert(address);
    } else {
        vm.consume_gas(100);
    }

    // balance is set to 1 wei because we won't run into div by 0 errors
    vm.stack.push(U256::from(1), operation);
    Ok(())
}

/// ORIGIN - Get execution origination address
pub fn origin(vm: &mut VM, operation: WrappedOpcode) -> Result<()> {
    vm.stack.push(VM::address_to_u256(&vm.origin), operation);
    Ok(())
}

/// CALLER - Get caller address
pub fn caller(vm: &mut VM, operation: WrappedOpcode) -> Result<()> {
    vm.stack.push(VM::address_to_u256(&vm.caller), operation);
    Ok(())
}

/// CALLVALUE - Get deposited value by the instruction/transaction responsible for this execution
pub fn callvalue(vm: &mut VM, operation: WrappedOpcode) -> Result<()> {
    vm.stack.push(U256::from(vm.value), operation);
    Ok(())
}

/// CALLDATALOAD - Get input data of current environment
pub fn calldataload(vm: &mut VM, operation: WrappedOpcode) -> Result<()> {
    let i = vm.stack.pop()?.value;

    // Safely convert U256 to usize
    let i: usize = i.try_into().unwrap_or(usize::MAX);

    let result = if i.saturating_add(32) > vm.calldata.len() {
        let mut value = [0u8; 32];

        if i <= vm.calldata.len() {
            value[..vm.calldata.len() - i].copy_from_slice(&vm.calldata[i..]);
        }

        U256::from_be_bytes(value)
    } else {
        U256::from_be_slice(&vm.calldata[i..i + 32])
    };

    vm.stack.push(result, operation);
    Ok(())
}

/// CALLDATASIZE - Get size of input data in current environment
pub fn calldatasize(vm: &mut VM, operation: WrappedOpcode) -> Result<()> {
    let result = U256::from(vm.calldata.len());
    vm.stack.push(result, operation);
    Ok(())
}

/// CALLDATACOPY - Copy input data in current environment to memory
pub fn calldatacopy(vm: &mut VM, operation: WrappedOpcode) -> Result<()> {
    let dest_offset = vm.stack.pop()?.value;
    let offset = vm.stack.pop()?.value;
    let size = vm.stack.pop()?.value;

    let dest_offset: usize = dest_offset.try_into().unwrap_or(usize::MAX);
    let offset: usize = offset.try_into().unwrap_or(usize::MAX);
    let size: usize = size.try_into().unwrap_or(vm.calldata.len()).min(vm.calldata.len());

    let value = VM::safe_copy_data(&vm.calldata, offset, size);

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

/// CODESIZE - Get size of code running in current environment
pub fn codesize(vm: &mut VM, operation: WrappedOpcode) -> Result<()> {
    let result = U256::from(vm.bytecode.len() as u128);
    vm.stack.push(result, operation);
    Ok(())
}

/// CODECOPY - Copy code running in current environment to memory
pub fn codecopy(vm: &mut VM, operation: WrappedOpcode) -> Result<()> {
    let dest_offset = vm.stack.pop()?.value;
    let offset = vm.stack.pop()?.value;
    let size = vm.stack.pop()?.value;

    let dest_offset: usize = dest_offset.try_into().unwrap_or(usize::MAX);
    let offset: usize = offset.try_into().unwrap_or(usize::MAX);
    let size: usize = size.try_into().unwrap_or(vm.bytecode.len()).min(vm.bytecode.len());

    let value = VM::safe_copy_data(&vm.bytecode, offset, size);

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

/// GASPRICE - Get price of gas in current environment
pub fn gasprice(vm: &mut VM, operation: WrappedOpcode) -> Result<()> {
    vm.stack.push(U256::from(1), operation);
    Ok(())
}

/// EXTCODESIZE - Get size of an account's code
pub fn extcodesize(vm: &mut VM, operation: WrappedOpcode) -> Result<()> {
    let address = vm.stack.pop()?.value;

    // consume dynamic gas
    if !vm.address_access_set.contains(&address) {
        vm.consume_gas(2600);
        vm.address_access_set.insert(address);
    } else {
        vm.consume_gas(100);
    }

    vm.stack.push(U256::from(1), operation);
    Ok(())
}

/// EXTCODECOPY - Copy an account's code to memory
pub fn extcodecopy(vm: &mut VM, operation: WrappedOpcode) -> Result<()> {
    let address = vm.stack.pop()?.value;
    let dest_offset = vm.stack.pop()?.value;
    vm.stack.pop()?;
    let size = vm.stack.pop()?.value;

    // Safely convert U256 to usize
    let dest_offset: usize = dest_offset.try_into().unwrap_or(0);
    let mut size: usize = size.try_into().unwrap_or(256);
    size = size.max(256);

    let mut value = Vec::with_capacity(size);
    value.fill(0xff);

    // consume dynamic gas
    let minimum_word_size = size.div_ceil(32) as u128;
    let gas_cost = 3 * minimum_word_size + vm.memory.expansion_cost(dest_offset, size);
    vm.consume_gas(gas_cost);
    if !vm.address_access_set.contains(&address) {
        vm.consume_gas(2600);
        vm.address_access_set.insert(address);
    } else {
        vm.consume_gas(100);
    }

    vm.memory.store_with_opcode(
        dest_offset,
        size,
        &value,
        #[cfg(feature = "experimental")]
        operation,
    );
    Ok(())
}

/// RETURNDATASIZE - Get size of output data from the previous call
pub fn returndatasize(vm: &mut VM, operation: WrappedOpcode) -> Result<()> {
    vm.stack.push(U256::from(1u8), operation);
    Ok(())
}

/// RETURNDATACOPY - Copy output data from the previous call to memory
pub fn returndatacopy(vm: &mut VM, operation: WrappedOpcode) -> Result<()> {
    let dest_offset = vm.stack.pop()?.value;
    vm.stack.pop()?;
    let size = vm.stack.pop()?.value;

    // Safely convert U256 to usize
    let dest_offset: usize = dest_offset.try_into().unwrap_or(0);
    let size: usize = size.try_into().unwrap_or(256);

    let mut value = Vec::with_capacity(size);
    value.fill(0xff);

    // consume dynamic gas
    let minimum_word_size = size.div_ceil(32) as u128;
    let gas_cost = 3 * minimum_word_size + vm.memory.expansion_cost(dest_offset, size);
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

/// EXTCODEHASH - Get hash of an account's code
pub fn extcodehash(vm: &mut VM, operation: WrappedOpcode) -> Result<()> {
    let address = vm.stack.pop()?.value;

    // consume dynamic gas
    if !vm.address_access_set.contains(&address) {
        vm.consume_gas(2600);
        vm.address_access_set.insert(address);
    } else {
        vm.consume_gas(100);
    }

    vm.stack.push(U256::ZERO, operation);
    Ok(())
}

/// BLOCKHASH - Get the hash of one of the 256 most recent complete blocks
pub fn blockhash(vm: &mut VM, operation: WrappedOpcode) -> Result<()> {
    vm.stack.pop()?;
    vm.stack.push(U256::ZERO, operation);
    Ok(())
}
