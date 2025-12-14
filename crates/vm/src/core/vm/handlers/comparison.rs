use eyre::Result;
use heimdall_common::utils::strings::sign_uint;

use crate::core::opcodes::WrappedOpcode;

use super::super::core::VM;

/// LT - Less than comparison
pub fn lt(vm: &mut VM, operation: WrappedOpcode) -> Result<()> {
    let a = vm.stack.pop()?.value;
    let b = vm.stack.pop()?.value;
    vm.push_boolean(a.lt(&b), operation);
    Ok(())
}

/// GT - Greater than comparison
pub fn gt(vm: &mut VM, operation: WrappedOpcode) -> Result<()> {
    let a = vm.stack.pop()?.value;
    let b = vm.stack.pop()?.value;
    vm.push_boolean(a.gt(&b), operation);
    Ok(())
}

/// SLT - Signed less than comparison
pub fn slt(vm: &mut VM, operation: WrappedOpcode) -> Result<()> {
    let a = vm.stack.pop()?.value;
    let b = vm.stack.pop()?.value;
    vm.push_boolean(sign_uint(a).lt(&sign_uint(b)), operation);
    Ok(())
}

/// SGT - Signed greater than comparison
pub fn sgt(vm: &mut VM, operation: WrappedOpcode) -> Result<()> {
    let a = vm.stack.pop()?.value;
    let b = vm.stack.pop()?.value;
    vm.push_boolean(sign_uint(a).gt(&sign_uint(b)), operation);
    Ok(())
}

/// EQ - Equality comparison
pub fn eq(vm: &mut VM, operation: WrappedOpcode) -> Result<()> {
    let a = vm.stack.pop()?.value;
    let b = vm.stack.pop()?.value;
    vm.push_boolean(a.eq(&b), operation);
    Ok(())
}

/// ISZERO - Check if zero
pub fn iszero(vm: &mut VM, operation: WrappedOpcode) -> Result<()> {
    let a = vm.stack.pop()?.value;
    vm.push_boolean(a.is_zero(), operation);
    Ok(())
}
