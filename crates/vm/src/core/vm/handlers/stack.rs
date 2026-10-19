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
    let num_bytes = usize::from(opcode - 95);
    let start = (vm.instruction - 1) as usize;
    let available = vm.bytecode.len().saturating_sub(start).min(num_bytes);
    let mut bytes = [0u8; 32];
    bytes[..available].copy_from_slice(&vm.bytecode[start..start + available]);
    let value = U256::from_be_slice(&bytes[..num_bytes]);
    vm.instruction += num_bytes as u128;
    operation.inputs = vec![WrappedInput::Raw(value)];
    vm.stack.push(value, operation);
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

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::Address;

    #[test]
    fn push_immediates_zero_pad_missing_trailing_bytes() {
        for width in 1..=32usize {
            for available in 0..=width {
                let mut bytecode = vec![95 + width as u8];
                bytecode.extend(vec![0xab; available]);
                let mut vm = VM::new(
                    &bytecode,
                    &[],
                    Address::ZERO,
                    Address::ZERO,
                    Address::ZERO,
                    0,
                    u128::MAX,
                );
                vm.step().unwrap();
                let mut expected = vec![0xab; available];
                expected.resize(width, 0);
                let value = U256::from_be_slice(&expected);
                assert_eq!(vm.stack.peek(0).value, value);
                assert_eq!(vm.stack.peek(0).operation.inputs, vec![WrappedInput::Raw(value)]);
                assert_eq!(vm.instruction, (width + 2) as u128);
                vm.step().unwrap();
                assert_eq!(vm.exitcode, 2);
            }
        }
    }
}
