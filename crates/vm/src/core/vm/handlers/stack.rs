use alloy::primitives::U256;
use eyre::Result;

use crate::core::opcodes::{WrappedInput, WrappedOpcode};

use super::super::core::VM;

/// POP - Remove item from stack
pub fn pop(vm: &mut VM) -> Result<()> {
    vm.stack.pop()?;
    Ok(())
}

/// PUSH0 - Push 0 onto stack
pub fn push0(vm: &mut VM, operation: WrappedOpcode) -> Result<()> {
    vm.stack.push(U256::ZERO, operation);
    Ok(())
}

/// PUSH1-PUSH32 - Push N bytes onto stack
pub fn push_n(vm: &mut VM, opcode: u8, mut operation: WrappedOpcode) -> Result<()> {
    // Get the number of bytes to push
    let num_bytes = (opcode - 95) as u128;

    // Get the bytes to push from bytecode
    let bytes =
        &vm.bytecode[(vm.instruction - 1) as usize..(vm.instruction - 1 + num_bytes) as usize];
    vm.instruction += num_bytes;

    // update the operation's inputs
    let new_operation_inputs = vec![WrappedInput::Raw(U256::from_be_slice(bytes))];

    operation.inputs = new_operation_inputs;

    // Push the bytes to the stack
    vm.stack.push(U256::from_be_slice(bytes), operation);
    Ok(())
}

/// DUP1-DUP16 - Duplicate Nth stack item
pub fn dup_n(vm: &mut VM, opcode: u8) -> Result<()> {
    // Get the number of items to swap
    let index = opcode - 127;
    // Perform the dup
    vm.stack.dup(index as usize);
    Ok(())
}

/// SWAP1-SWAP16 - Exchange 1st and Nth stack items
pub fn swap_n(vm: &mut VM, opcode: u8) -> Result<()> {
    // Get the number of items to swap
    let index = opcode - 143;
    // Perform the swap
    vm.stack.swap(index as usize);
    Ok(())
}
