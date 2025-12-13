use std::ops::{Div, Rem};

use alloy::primitives::{I256, U256};
use eyre::Result;
use heimdall_common::utils::strings::sign_uint;

use crate::core::opcodes::WrappedOpcode;

use super::super::core::VM;

/// ADD - Addition operation
pub fn add(vm: &mut VM, operation: WrappedOpcode) -> Result<()> {
    let a = vm.stack.pop()?;
    let b = vm.stack.pop()?;
    let result = a.value.overflowing_add(b.value).0;
    vm.push_with_optimization(result, &a, &b, operation);
    Ok(())
}

/// MUL - Multiplication operation
pub fn mul(vm: &mut VM, operation: WrappedOpcode) -> Result<()> {
    let a = vm.stack.pop()?;
    let b = vm.stack.pop()?;
    let result = a.value.overflowing_mul(b.value).0;
    vm.push_with_optimization(result, &a, &b, operation);
    Ok(())
}

/// SUB - Subtraction operation
pub fn sub(vm: &mut VM, operation: WrappedOpcode) -> Result<()> {
    let a = vm.stack.pop()?;
    let b = vm.stack.pop()?;
    let result = a.value.overflowing_sub(b.value).0;
    vm.push_with_optimization(result, &a, &b, operation);
    Ok(())
}

/// DIV - Integer division operation
pub fn div(vm: &mut VM, operation: WrappedOpcode) -> Result<()> {
    let numerator = vm.stack.pop()?;
    let denominator = vm.stack.pop()?;
    let result =
        if !denominator.value.is_zero() { numerator.value.div(denominator.value) } else { U256::ZERO };
    vm.push_with_optimization(result, &numerator, &denominator, operation);
    Ok(())
}

/// SDIV - Signed integer division operation
pub fn sdiv(vm: &mut VM, operation: WrappedOpcode) -> Result<()> {
    let numerator = vm.stack.pop()?;
    let denominator = vm.stack.pop()?;
    let result = if !denominator.value.is_zero() {
        sign_uint(numerator.value).div(sign_uint(denominator.value))
    } else {
        I256::ZERO
    };
    vm.push_with_optimization_signed(result, &numerator, &denominator, operation);
    Ok(())
}

/// MOD - Modulo operation
pub fn modulo(vm: &mut VM, operation: WrappedOpcode) -> Result<()> {
    let a = vm.stack.pop()?;
    let modulus = vm.stack.pop()?;
    let result = if !modulus.value.is_zero() { a.value.rem(modulus.value) } else { U256::ZERO };
    vm.push_with_optimization(result, &a, &modulus, operation);
    Ok(())
}

/// SMOD - Signed modulo operation
pub fn smod(vm: &mut VM, operation: WrappedOpcode) -> Result<()> {
    let a = vm.stack.pop()?;
    let modulus = vm.stack.pop()?;
    let result = if !modulus.value.is_zero() {
        sign_uint(a.value).rem(sign_uint(modulus.value))
    } else {
        I256::ZERO
    };
    vm.push_with_optimization_signed(result, &a, &modulus, operation);
    Ok(())
}

/// ADDMOD - Addition modulo operation
pub fn addmod(vm: &mut VM, operation: WrappedOpcode) -> Result<()> {
    let a = vm.stack.pop()?;
    let b = vm.stack.pop()?;
    let modulus = vm.stack.pop()?;
    let result =
        if !modulus.value.is_zero() { a.value.add_mod(b.value, modulus.value) } else { U256::ZERO };
    vm.push_with_optimization(result, &a, &b, operation);
    Ok(())
}

/// MULMOD - Multiplication modulo operation
pub fn mulmod(vm: &mut VM, operation: WrappedOpcode) -> Result<()> {
    let a = vm.stack.pop()?;
    let b = vm.stack.pop()?;
    let modulus = vm.stack.pop()?;
    let result =
        if !modulus.value.is_zero() { a.value.mul_mod(b.value, modulus.value) } else { U256::ZERO };
    vm.push_with_optimization(result, &a, &b, operation);
    Ok(())
}

/// EXP - Exponential operation
pub fn exp(vm: &mut VM, operation: WrappedOpcode) -> Result<()> {
    let a = vm.stack.pop()?;
    let exponent = vm.stack.pop()?;
    let result = a.value.overflowing_pow(exponent.value).0;

    // consume dynamic gas
    let exponent_byte_size = exponent.value.bit_len() / 8;
    let gas_cost = 50 * exponent_byte_size;
    vm.consume_gas(gas_cost as u128);

    vm.push_with_optimization(result, &a, &exponent, operation);
    Ok(())
}

/// SIGNEXTEND - Extend length of two's complement signed integer
pub fn signextend(vm: &mut VM, operation: WrappedOpcode) -> Result<()> {
    let x = vm.stack.pop()?.value;
    let b = vm.stack.pop()?.value;

    let t = x * U256::from(8u32) + U256::from(7u32);
    let sign_bit = U256::from(1u32) << t;

    // (b & sign_bit - 1) - (b & sign_bit)
    let result =
        (b & (sign_bit.overflowing_sub(U256::from(1u32)).0)).overflowing_sub(b & sign_bit).0;

    vm.stack.push(result, operation);
    Ok(())
}
