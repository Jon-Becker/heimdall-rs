use alloy::primitives::U256;
use eyre::Result;

use crate::core::opcodes::WrappedOpcode;

use super::super::core::VM;

/// SLOAD - Load word from storage
pub fn sload(vm: &mut VM, operation: WrappedOpcode) -> Result<()> {
    let key = vm.stack.pop()?.value;

    // consume dynamic gas
    let gas_cost = vm.storage.access_cost(key);
    vm.consume_gas(gas_cost);

    vm.stack.push(U256::from(vm.storage.load(key)), operation);
    Ok(())
}

/// SSTORE - Save word to storage
pub fn sstore(vm: &mut VM) -> Result<()> {
    let key = vm.stack.pop()?.value;
    let value = vm.stack.pop()?.value;

    // consume dynamic gas
    let gas_cost = vm.storage.storage_cost(key, value);
    vm.consume_gas(gas_cost);

    vm.storage.store(key, value);
    Ok(())
}

/// TLOAD - Load word from transient storage
pub fn tload(vm: &mut VM, operation: WrappedOpcode) -> Result<()> {
    let key = vm.stack.pop()?.value;
    vm.stack.push(U256::from(vm.storage.tload(key)), operation);
    Ok(())
}

/// TSTORE - Save word to transient storage
pub fn tstore(vm: &mut VM) -> Result<()> {
    let key = vm.stack.pop()?.value;
    let value = vm.stack.pop()?.value;
    vm.storage.tstore(key, value);
    Ok(())
}
