use alloy::primitives::U256;
use eyre::Result;

use crate::core::{
    constants::{CREATE2_ADDRESS, CREATE_ADDRESS},
    opcodes::WrappedOpcode,
};

use super::super::core::VM;

/// CREATE - Create a new account with associated code
pub fn create(vm: &mut VM, operation: WrappedOpcode) -> Result<()> {
    vm.stack.pop_n(3)?;
    vm.stack.push(*CREATE_ADDRESS, operation);
    Ok(())
}

/// CALL - Message-call into an account
pub fn call(vm: &mut VM, operation: WrappedOpcode) -> Result<()> {
    let address = vm.stack.pop()?.value;
    vm.stack.pop_n(6)?;

    // consume dynamic gas
    if !vm.address_access_set.contains(&address) {
        vm.consume_gas(2600);
        vm.address_access_set.insert(address);
    } else {
        vm.consume_gas(100);
    }

    vm.stack.push(U256::from(1u8), operation);
    Ok(())
}

/// CALLCODE - Message-call into this account with alternative account's code
pub fn callcode(vm: &mut VM, operation: WrappedOpcode) -> Result<()> {
    let address = vm.stack.pop()?.value;
    vm.stack.pop_n(6)?;

    // consume dynamic gas
    if !vm.address_access_set.contains(&address) {
        vm.consume_gas(2600);
        vm.address_access_set.insert(address);
    } else {
        vm.consume_gas(100);
    }

    vm.stack.push(U256::from(1u8), operation);
    Ok(())
}

/// RETURN - Halt execution returning output data
pub fn op_return(vm: &mut VM) -> Result<()> {
    let offset = vm.stack.pop()?.value;
    let size = vm.stack.pop()?.value;

    // Safely convert U256 to usize
    let offset: usize = offset.try_into().unwrap_or(usize::MAX);
    let size: usize = size.try_into().unwrap_or(usize::MAX);

    // consume dynamic gas
    let gas_cost = vm.memory.expansion_cost(offset, size);
    vm.consume_gas(gas_cost);

    vm.exit(0, vm.memory.read(offset, size));
    Ok(())
}

/// DELEGATECALL - Message-call into this account with an alternative account's code
pub fn delegatecall(vm: &mut VM, operation: WrappedOpcode) -> Result<()> {
    let address = vm.stack.pop()?.value;
    vm.stack.pop_n(5)?;

    // consume dynamic gas
    if !vm.address_access_set.contains(&address) {
        vm.consume_gas(2600);
        vm.address_access_set.insert(address);
    } else {
        vm.consume_gas(100);
    }

    vm.stack.push(U256::from(1u8), operation);
    Ok(())
}

/// STATICCALL - Static message-call into an account
pub fn staticcall(vm: &mut VM, operation: WrappedOpcode) -> Result<()> {
    let address = vm.stack.pop()?.value;
    vm.stack.pop_n(5)?;

    // consume dynamic gas
    if !vm.address_access_set.contains(&address) {
        vm.consume_gas(2600);
        vm.address_access_set.insert(address);
    } else {
        vm.consume_gas(100);
    }

    vm.stack.push(U256::from(1u8), operation);
    Ok(())
}

/// CREATE2 - Create a new account with associated code at a predictable address
pub fn create2(vm: &mut VM, operation: WrappedOpcode) -> Result<()> {
    vm.stack.pop_n(4)?;
    vm.stack.push(*CREATE2_ADDRESS, operation);
    Ok(())
}

/// REVERT - Halt execution reverting state changes
pub fn revert(vm: &mut VM) -> Result<()> {
    let offset = vm.stack.pop()?.value;
    let size = vm.stack.pop()?.value;

    // Safely convert U256 to usize
    let offset: usize = offset.try_into().unwrap_or(usize::MAX);
    let size: usize = size.try_into().unwrap_or(usize::MAX);

    vm.exit(1, vm.memory.read(offset, size));
    Ok(())
}
