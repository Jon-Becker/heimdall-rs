use alloy::primitives::U256;
use eyre::Result;

use crate::core::{
    opcodes::{WrappedInput, WrappedOpcode},
    program::decode_push_operand,
};

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
    let num_bytes = usize::from(opcode - 95);
    let start = (vm.instruction - 1) as usize;
    let end = start.saturating_add(num_bytes).min(vm.bytecode.len());
    let value = decode_push_operand(opcode, &vm.bytecode[start..end])
        .expect("PUSH handler received a non-PUSH opcode");
    vm.instruction += num_bytes as u128;
    operation.inputs = vec![WrappedInput::Raw(value)];
    vm.stack.push(value, operation);
    Ok(())
}

/// DUP1-DUP16 - Duplicate Nth stack item
pub fn dup_n(vm: &mut VM, opcode: u8) -> Result<()> {
    let index = opcode - 127;
    vm.stack.dup(index as usize);
    Ok(())
}

/// SWAP1-SWAP16 - Exchange 1st and Nth stack items
pub fn swap_n(vm: &mut VM, opcode: u8) -> Result<()> {
    let index = opcode - 143;
    vm.stack.swap(index as usize);
    Ok(())
}
