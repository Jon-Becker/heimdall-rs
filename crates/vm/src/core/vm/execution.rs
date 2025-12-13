use alloy::primitives::U256;

use super::super::{
    log::Log, memory::Memory, opcodes::WrappedOpcode, stack::Stack, storage::Storage,
};

/// [`ExecutionResult`] is the result of a single contract execution.
#[derive(Clone, Debug)]
pub struct ExecutionResult {
    /// The amount of gas consumed during the execution.
    pub gas_used: u128,

    /// The amount of gas left after execution completes.
    pub gas_remaining: u128,

    /// The data returned by the execution.
    pub returndata: Vec<u8>,

    /// The exit code of the execution (0 for success, non-zero for errors).
    pub exitcode: u128,

    /// The events (logs) emitted during execution.
    pub events: Vec<Log>,

    /// The final instruction pointer value after execution.
    pub instruction: u128,
}

/// [`State`] is the state of the EVM after executing a single instruction. It is returned by the
/// [`VM::step`] function, and is used by heimdall for tracing contract execution.
#[derive(Clone, Debug)]
pub struct State {
    /// The instruction that was just executed.
    pub last_instruction: Instruction,

    /// The total amount of gas used so far during execution.
    pub gas_used: u128,

    /// The amount of gas remaining for execution.
    pub gas_remaining: u128,

    /// The current state of the EVM stack.
    pub stack: Stack,

    /// The current state of the EVM memory.
    pub memory: Memory,

    /// The current state of the contract storage.
    pub storage: Storage,

    /// The events (logs) emitted so far during execution.
    pub events: Vec<Log>,
}

/// [`Instruction`] is a single EVM instruction. It is returned by the [`VM::step`] function, and
/// contains necessary tracing information, such as the opcode executed, it's inputs and outputs, as
/// well as their parent operations.
#[derive(Clone, Debug)]
pub struct Instruction {
    /// The position of this instruction in the bytecode.
    pub instruction: u128,

    /// The opcode value of the instruction.
    pub opcode: u8,

    /// The raw values of the inputs to this instruction.
    pub inputs: Vec<U256>,

    /// The raw values of the outputs produced by this instruction.
    pub outputs: Vec<U256>,

    /// The wrapped operations that produced the inputs to this instruction.
    /// This allows for tracking data flow and operation dependencies.
    pub input_operations: Vec<WrappedOpcode>,

    /// The wrapped operations that will consume the outputs of this instruction.
    /// This allows for forward tracking of data flow.
    pub output_operations: Vec<WrappedOpcode>,
}
