use std::ops::{Shl, Shr};

use alloy::primitives::{I256, U256};
use eyre::Result;
use heimdall_common::utils::strings::sign_uint;

use crate::core::opcodes::WrappedOpcode;

use super::super::core::VM;

/// AND - Bitwise AND operation
pub fn and(vm: &mut VM, operation: WrappedOpcode) -> Result<()> {
    let a = vm.stack.pop()?;
    let b = vm.stack.pop()?;
    let result = a.value & b.value;
    vm.push_with_optimization(result, &a, &b, operation);
    Ok(())
}

/// OR - Bitwise OR operation
pub fn or(vm: &mut VM, operation: WrappedOpcode) -> Result<()> {
    let a = vm.stack.pop()?;
    let b = vm.stack.pop()?;
    let result = a.value | b.value;
    vm.push_with_optimization(result, &a, &b, operation);
    Ok(())
}

/// XOR - Bitwise XOR operation
pub fn xor(vm: &mut VM, operation: WrappedOpcode) -> Result<()> {
    let a = vm.stack.pop()?;
    let b = vm.stack.pop()?;
    let result = a.value ^ b.value;
    vm.push_with_optimization(result, &a, &b, operation);
    Ok(())
}

/// NOT - Bitwise NOT operation
pub fn not(vm: &mut VM, operation: WrappedOpcode) -> Result<()> {
    let a = vm.stack.pop()?;
    let result = !a.value;
    vm.push_with_optimization_single(result, &a, operation);
    Ok(())
}

/// BYTE - Retrieve single byte from word
pub fn byte(vm: &mut VM, operation: WrappedOpcode) -> Result<()> {
    let b = vm.stack.pop()?.value;
    let a = vm.stack.pop()?.value;
    let result = if b >= U256::from(32u32) {
        U256::ZERO
    } else {
        a / (U256::from(256u32).pow(U256::from(31u32) - b)) % U256::from(256u32)
    };
    vm.stack.push(result, operation);
    Ok(())
}

/// SHL - Shift left operation
pub fn shl(vm: &mut VM, operation: WrappedOpcode) -> Result<()> {
    let a = vm.stack.pop()?;
    let b = vm.stack.pop()?;
    let result = if a.value > U256::from(255u8) { U256::ZERO } else { b.value.shl(a.value) };
    vm.push_with_optimization(result, &a, &b, operation);
    Ok(())
}

/// SHR - Shift right operation
pub fn shr(vm: &mut VM, operation: WrappedOpcode) -> Result<()> {
    let a = vm.stack.pop()?;
    let b = vm.stack.pop()?;
    let result = if a.value > U256::from(255u8) { U256::ZERO } else { b.value.shr(a.value) };
    vm.push_with_optimization(result, &a, &b, operation);
    Ok(())
}

/// SAR - Arithmetic shift right operation
pub fn sar(vm: &mut VM, operation: WrappedOpcode) -> Result<()> {
    let a = vm.stack.pop()?;
    let b = vm.stack.pop()?;
    let usize_a: usize = a.value.try_into().unwrap_or(usize::MAX);
    let result = if !b.value.is_zero() { sign_uint(b.value).shr(usize_a) } else { I256::ZERO };
    vm.push_with_optimization_signed(result, &a, &b, operation);
    Ok(())
}
