use alloy::primitives::U256;
use eyre::Result;

use crate::core::{
    opcodes::{self, WrappedOpcode},
    program::Program,
};

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
    let destination = vm.stack.pop().ok()?.value;
    jump_to(vm, destination, last_instruction, opcodes::JUMP, inputs, input_operations)
}

/// JUMPI - Conditionally alter the program counter
pub fn jumpi(
    vm: &mut VM,
    last_instruction: u128,
    inputs: &[U256],
    input_operations: &[WrappedOpcode],
) -> Option<Instruction> {
    let destination = vm.stack.pop().ok()?.value;
    let condition = vm.stack.pop().ok()?.value;
    if !condition.is_zero() {
        return jump_to(vm, destination, last_instruction, opcodes::JUMPI, inputs, input_operations);
    }
    None
}

fn jump_to(
    vm: &mut VM,
    destination: U256,
    last_instruction: u128,
    opcode: u8,
    inputs: &[U256],
    input_operations: &[WrappedOpcode],
) -> Option<Instruction> {
    if let Ok(pc) = usize::try_from(destination) {
        if Program::is_valid_jumpdest_in(&vm.bytecode, pc) {
            vm.instruction = pc as u128 + 1;
            return None;
        }
    }
    vm.exit(790, Vec::new());
    Some(Instruction {
        instruction: last_instruction,
        opcode,
        inputs: inputs.to_vec(),
        outputs: Vec::new(),
        input_operations: input_operations.to_vec(),
        output_operations: Vec::new(),
    })
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
