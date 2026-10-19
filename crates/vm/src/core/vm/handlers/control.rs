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

    if vm.bytecode.get(usize::try_from(pc).unwrap_or(usize::MAX)) != Some(&opcodes::JUMPDEST) {
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
        if vm.bytecode.get(usize::try_from(pc).unwrap_or(usize::MAX)) != Some(&opcodes::JUMPDEST) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::Address;
    use heimdall_common::utils::strings::decode_hex;

    fn exitcode(bytecode: &str) -> u128 {
        let bytecode = decode_hex(bytecode).unwrap();
        let mut vm =
            VM::new(&bytecode, &[], Address::ZERO, Address::ZERO, Address::ZERO, 0, u128::MAX);
        for _ in 0..8 {
            vm.step().unwrap();
            if vm.exitcode != 255 {
                return vm.exitcode;
            }
        }
        panic!("fixture did not terminate");
    }

    #[test]
    fn rejects_invalid_jump_destinations_without_overflow_or_index_panics() {
        for bytecode in
            ["600356".to_string(), "600056".to_string(), format!("7f{}56", "ff".repeat(32))]
        {
            assert_eq!(exitcode(&bytecode), 790);
        }
        assert_eq!(exitcode(&format!("60017f{}57", "ff".repeat(32))), 790);
        assert_eq!(exitcode(&format!("60007f{}5700", "ff".repeat(32))), 10);
    }
}
