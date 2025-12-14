use alloy::primitives::U256;
use eyre::Result;

use crate::core::opcodes::{self, WrappedOpcode};

use super::super::{core::VM, execution::Instruction};

/// STOP - Halts execution
pub fn stop(
    vm: &mut VM,
    last_instruction: u128,
    inputs: &[U256],
    input_operations: &[WrappedOpcode],
) -> Instruction {
    vm.exit(10, Vec::new());
    Instruction {
        instruction: last_instruction,
        opcode: opcodes::STOP,
        inputs: inputs.to_vec(),
        outputs: Vec::new(),
        input_operations: input_operations.to_vec(),
        output_operations: Vec::new(),
    }
}

/// JUMP - Alter the program counter
pub fn jump(
    vm: &mut VM,
    last_instruction: u128,
    inputs: &[U256],
    input_operations: &[WrappedOpcode],
) -> Option<Instruction> {
    let pc = vm.stack.pop().ok()?.value;

    // Safely convert U256 to u128
    let pc: u128 = pc.try_into().unwrap_or(u128::MAX);

    // Check if JUMPDEST is valid and throw with 790 if not (invalid jump destination)
    if (pc <=
        vm.bytecode
            .len()
            .try_into()
            .expect("impossible case: bytecode is larger than u128::MAX")) &&
        (vm.bytecode[pc as usize] != opcodes::JUMPDEST)
    {
        vm.exit(790, Vec::new());
        return Some(Instruction {
            instruction: last_instruction,
            opcode: opcodes::JUMP,
            inputs: inputs.to_vec(),
            outputs: Vec::new(),
            input_operations: input_operations.to_vec(),
            output_operations: Vec::new(),
        });
    } else {
        vm.instruction = pc + 1;
    }
    None
}

/// JUMPI - Conditionally alter the program counter
pub fn jumpi(
    vm: &mut VM,
    last_instruction: u128,
    inputs: &[U256],
    input_operations: &[WrappedOpcode],
) -> Option<Instruction> {
    let pc = vm.stack.pop().ok()?.value;
    let condition = vm.stack.pop().ok()?.value;

    // Safely convert U256 to u128
    let pc: u128 = pc.try_into().unwrap_or(u128::MAX);

    if !condition.is_zero() {
        // Check if JUMPDEST is valid and throw with 790 if not (invalid jump
        // destination)
        if (pc <
            vm.bytecode
                .len()
                .try_into()
                .expect("impossible case: bytecode is larger than u128::MAX")) &&
            (vm.bytecode[pc as usize] != opcodes::JUMPDEST)
        {
            vm.exit(790, Vec::new());
            return Some(Instruction {
                instruction: last_instruction,
                opcode: opcodes::JUMPI,
                inputs: inputs.to_vec(),
                outputs: Vec::new(),
                input_operations: input_operations.to_vec(),
                output_operations: Vec::new(),
            });
        } else {
            vm.instruction = pc + 1;
        }
    }
    None
}

/// JUMPDEST - Mark a valid destination for jumps (no-op)
pub fn jumpdest() -> Result<()> {
    Ok(())
}

/// PC - Get the value of the program counter prior to the increment
pub fn pc(vm: &mut VM, operation: WrappedOpcode) -> Result<()> {
    vm.stack.push(U256::from(vm.instruction), operation);
    Ok(())
}

/// GAS - Get the amount of available gas
pub fn gas(vm: &mut VM, operation: WrappedOpcode) -> Result<()> {
    vm.stack.push(U256::from(vm.gas_remaining), operation);
    Ok(())
}
