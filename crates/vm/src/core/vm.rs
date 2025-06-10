use hashbrown::HashSet;
use std::{
    ops::{Div, Rem, Shl, Shr},
    time::{SystemTime, UNIX_EPOCH},
};

use alloy::primitives::{keccak256, Address, I256, U256};
use eyre::{OptionExt, Result};
use heimdall_common::utils::strings::sign_uint;

#[cfg(feature = "step-tracing")]
use std::time::Instant;
#[cfg(feature = "step-tracing")]
use tracing::trace;

use crate::core::opcodes::OpCodeInfo;

use super::{
    constants::{COINBASE_ADDRESS, CREATE2_ADDRESS, CREATE_ADDRESS},
    log::Log,
    memory::Memory,
    opcodes::{WrappedInput, WrappedOpcode},
    stack::{Stack, StackFrame},
    storage::Storage,
};

/// The [`VM`] struct represents an EVM instance. \
/// It contains the EVM's [`Stack`], [`Memory`], [`Storage`], and other state variables needed to
/// emulate EVM execution.
#[derive(Clone, Debug)]
pub struct VM {
    /// The EVM stack that holds values during execution.
    pub stack: Stack,

    /// The EVM memory space that can be read from and written to.
    pub memory: Memory,

    /// The contract's persistent storage.
    pub storage: Storage,

    /// The current instruction pointer (program counter).
    pub instruction: u128,

    /// The compiled bytecode being executed.
    pub bytecode: Vec<u8>,

    /// The input data provided to the contract call.
    pub calldata: Vec<u8>,

    /// The address of the executing contract.
    pub address: Address,

    /// The address that originated the transaction.
    pub origin: Address,

    /// The address that directly called this contract.
    pub caller: Address,

    /// The amount of ether sent with the call (in wei).
    pub value: u128,

    /// The amount of gas remaining for execution.
    pub gas_remaining: u128,

    /// The amount of gas used so far during execution.
    pub gas_used: u128,

    /// The events (logs) emitted during execution.
    pub events: Vec<Log>,

    /// The data returned by the execution.
    pub returndata: Vec<u8>,

    /// The exit code of the execution (0 for success, non-zero for errors).
    pub exitcode: u128,

    /// A set of addresses that have been accessed during execution (used for gas calculation).
    pub address_access_set: HashSet<U256>,

    /// Counter for operations executed (only available with step-tracing feature).
    #[cfg(feature = "step-tracing")]
    pub operation_count: u128,

    /// The time when execution started (only available with step-tracing feature).
    #[cfg(feature = "step-tracing")]
    pub start_time: Instant,
}

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

impl VM {
    /// Creates a new [`VM`] instance with the given bytecode, calldata, address, origin, caller,
    /// value, and gas limit.
    ///
    /// ```
    /// use heimdall_vm::core::vm::VM;
    /// use alloy::primitives::Address;
    ///
    /// let vm = VM::new(
    ///     &vec![0x00],
    ///     &vec![],
    ///     "0x0000000000000000000000000000000000000000".parse::<Address>().expect("failed to parse Address"),
    ///     "0x0000000000000000000000000000000000000001".parse::<Address>().expect("failed to parse Address"),
    ///     "0x0000000000000000000000000000000000000002".parse::<Address>().expect("failed to parse Address"),
    ///     0,
    ///     1000000000000000000,
    /// );
    /// ```
    pub fn new(
        bytecode: &[u8],
        calldata: &[u8],
        address: Address,
        origin: Address,
        caller: Address,
        value: u128,
        gas_limit: u128,
    ) -> VM {
        VM {
            stack: Stack::new(),
            memory: Memory::new(),
            storage: Storage::new(),
            instruction: 1,
            bytecode: bytecode.to_vec(),
            calldata: calldata.to_vec(),
            address,
            origin,
            caller,
            value,
            gas_remaining: gas_limit.max(21000) - 21000,
            gas_used: 21000,
            events: Vec::new(),
            returndata: Vec::new(),
            exitcode: 255,
            address_access_set: HashSet::new(),
            #[cfg(feature = "step-tracing")]
            operation_count: 0,
            #[cfg(feature = "step-tracing")]
            start_time: Instant::now(),
        }
    }

    /// Exits current execution with the given code and returndata.
    ///
    /// ```
    /// use heimdall_vm::core::vm::VM;
    /// use alloy::primitives::Address;
    ///
    /// let mut vm = VM::new(
    ///     &vec![0x00],
    ///     &vec![],
    ///     "0x0000000000000000000000000000000000000000".parse::<Address>().expect("failed to parse Address"),
    ///     "0x0000000000000000000000000000000000000001".parse::<Address>().expect("failed to parse Address"),
    ///     "0x0000000000000000000000000000000000000002".parse::<Address>().expect("failed to parse Address"),
    ///     0,
    ///     1000000000000000000,
    /// );
    ///
    /// vm.exit(0xff, Vec::new());
    /// assert_eq!(vm.exitcode, 0xff);
    /// ```
    pub fn exit(&mut self, code: u128, returndata: Vec<u8>) {
        self.exitcode = code;
        self.returndata = returndata;
    }

    /// Consume gas units, halting execution if out of gas
    ///
    /// ```
    /// use heimdall_vm::core::vm::VM;
    /// use alloy::primitives::Address;
    ///
    /// let mut vm = VM::new(
    ///     &vec![0x00],
    ///     &vec![],
    ///     "0x0000000000000000000000000000000000000000".parse::<Address>().expect("failed to parse Address"),
    ///     "0x0000000000000000000000000000000000000001".parse::<Address>().expect("failed to parse Address"),
    ///     "0x0000000000000000000000000000000000000002".parse::<Address>().expect("failed to parse Address"),
    ///     0,
    ///     1000000000000000000,
    /// );
    ///
    /// vm.consume_gas(100);
    /// assert_eq!(vm.gas_remaining, 999999999999978900);
    ///
    /// vm.consume_gas(1000000000000000000);
    /// assert_eq!(vm.gas_remaining, 0);
    /// assert_eq!(vm.exitcode, 9);
    /// ```
    pub fn consume_gas(&mut self, amount: u128) -> bool {
        // REVERT if out of gas
        if amount > self.gas_remaining {
            self.gas_used += self.gas_remaining;
            self.gas_remaining = 0;
            self.exit(9, Vec::new());
            return false;
        }

        self.gas_remaining = self.gas_remaining.saturating_sub(amount);
        self.gas_used = self.gas_used.saturating_add(amount);
        true
    }

    /// Executes the next instruction in the bytecode. Returns information about the instruction
    /// executed.
    ///
    /// ```no_run
    /// use heimdall_vm::core::vm::VM;
    /// use alloy::primitives::Address;
    ///
    /// let mut vm = VM::new(
    ///     &vec![0x00],
    ///     &vec![],
    ///     "0x0000000000000000000000000000000000000000".parse::<Address>().expect("failed to parse Address"),
    ///     "0x0000000000000000000000000000000000000001".parse::<Address>().expect("failed to parse Address"),
    ///     "0x0000000000000000000000000000000000000002".parse::<Address>().expect("failed to parse Address"),
    ///     0,
    ///     1000000000000000000,
    /// );
    ///
    /// // vm._step(); // 0x00 EXIT
    /// // assert_eq!(vm.exitcode, 10);
    /// ```

    fn push_boolean(&mut self, condition: bool, operation: WrappedOpcode) {
        let value = if condition { U256::from(1u8) } else { U256::ZERO };
        self.stack.push(value, operation);
    }

    fn address_to_u256(address: &Address) -> U256 {
        let mut result = [0u8; 32];
        result[12..].copy_from_slice(address.as_ref());
        U256::from_be_bytes(result)
    }

    fn push_with_optimization(
        &mut self,
        result: U256,
        a: &StackFrame,
        b: &StackFrame,
        operation: WrappedOpcode,
    ) {
        let simplified_operation = if (0x5f..=0x7f).contains(&a.operation.opcode) &&
            (0x5f..=0x7f).contains(&b.operation.opcode)
        {
            WrappedOpcode::new(0x7f, vec![WrappedInput::Raw(result)])
        } else {
            operation
        };
        self.stack.push(result, simplified_operation);
    }

    fn push_with_optimization_single(
        &mut self,
        result: U256,
        a: &StackFrame,
        operation: WrappedOpcode,
    ) {
        let simplified_operation = if (0x5f..=0x7f).contains(&a.operation.opcode) {
            WrappedOpcode::new(0x7f, vec![WrappedInput::Raw(result)])
        } else {
            operation
        };
        self.stack.push(result, simplified_operation);
    }

    fn push_with_optimization_signed(
        &mut self,
        result: I256,
        a: &StackFrame,
        b: &StackFrame,
        operation: WrappedOpcode,
    ) {
        let simplified_operation = if (0x5f..=0x7f).contains(&a.operation.opcode) &&
            (0x5f..=0x7f).contains(&b.operation.opcode)
        {
            WrappedOpcode::new(0x7f, vec![WrappedInput::Raw(result.into_raw())])
        } else {
            operation
        };
        self.stack.push(result.into_raw(), simplified_operation);
    }

    fn safe_copy_data(source: &[u8], offset: usize, size: usize) -> Vec<u8> {
        let end_offset = offset.saturating_add(size).min(source.len());
        let mut value = source.get(offset..end_offset).unwrap_or(&[]).to_owned();
        if value.len() < size {
            value.resize(size, 0u8);
        }
        value
    }

    fn _step(&mut self) -> Result<Instruction> {
        // sanity check
        if self.bytecode.len() < self.instruction as usize {
            self.exit(2, Vec::new());
            return Ok(Instruction {
                instruction: self.instruction,
                opcode: 0xff,
                inputs: Vec::new(),
                outputs: Vec::new(),
                input_operations: Vec::new(),
                output_operations: Vec::new(),
            });
        }

        // get the opcode at the current instruction
        let opcode = self
            .bytecode
            .get((self.instruction - 1) as usize)
            .ok_or_eyre(format!("invalid jumpdest: {}", self.instruction - 1))?
            .to_owned();
        let last_instruction = self.instruction;
        self.instruction += 1;
        #[cfg(feature = "step-tracing")]
        {
            self.operation_count += 1;
        }
        #[cfg(feature = "step-tracing")]
        let start_time = Instant::now();

        // add the opcode to the trace
        let opcode_info = OpCodeInfo::from(opcode);
        let input_frames = self.stack.peek_n(opcode_info.inputs() as usize);
        let input_operations =
            input_frames.iter().map(|x| x.operation.clone()).collect::<Vec<WrappedOpcode>>();
        let inputs = input_frames.iter().map(|x| x.value).collect::<Vec<U256>>();

        // Consume the minimum gas for the opcode
        let gas_cost = opcode_info.min_gas();
        self.consume_gas(gas_cost.into());

        // convert inputs to WrappedInputs
        let wrapped_inputs = input_operations
            .iter()
            .map(|x| WrappedInput::Opcode(x.to_owned()))
            .collect::<Vec<WrappedInput>>();
        let mut operation = WrappedOpcode::new(opcode, wrapped_inputs);

        // if step-tracing feature is enabled, print the current operation
        #[cfg(feature = "step-tracing")]
        trace!(
            pc = self.instruction - 1,
            opcode = opcode_info.name(),
            inputs = ?inputs
                .iter()
                .map(|x| format!("{x:#x}"))
                .collect::<Vec<String>>(),
            "executing opcode"
        );

        // execute the operation
        match opcode {
            // STOP
            0x00 => {
                self.exit(10, Vec::new());
                return Ok(Instruction {
                    instruction: last_instruction,
                    opcode,
                    inputs,
                    outputs: Vec::new(),
                    input_operations,
                    output_operations: Vec::new(),
                });
            }

            // ADD
            0x01 => {
                let a = self.stack.pop()?;
                let b = self.stack.pop()?;
                let result = a.value.overflowing_add(b.value).0;
                self.push_with_optimization(result, &a, &b, operation);
            }

            // MUL
            0x02 => {
                let a = self.stack.pop()?;
                let b = self.stack.pop()?;
                let result = a.value.overflowing_mul(b.value).0;
                self.push_with_optimization(result, &a, &b, operation);
            }

            // SUB
            0x03 => {
                let a = self.stack.pop()?;
                let b = self.stack.pop()?;
                let result = a.value.overflowing_sub(b.value).0;
                self.push_with_optimization(result, &a, &b, operation);
            }

            // DIV
            0x04 => {
                let numerator = self.stack.pop()?;
                let denominator = self.stack.pop()?;
                let result = if !denominator.value.is_zero() {
                    numerator.value.div(denominator.value)
                } else {
                    U256::ZERO
                };
                self.push_with_optimization(result, &numerator, &denominator, operation);
            }

            // SDIV
            0x05 => {
                let numerator = self.stack.pop()?;
                let denominator = self.stack.pop()?;
                let result = if !denominator.value.is_zero() {
                    sign_uint(numerator.value).div(sign_uint(denominator.value))
                } else {
                    I256::ZERO
                };
                self.push_with_optimization_signed(result, &numerator, &denominator, operation);
            }

            // MOD
            0x06 => {
                let a = self.stack.pop()?;
                let modulus = self.stack.pop()?;
                let result =
                    if !modulus.value.is_zero() { a.value.rem(modulus.value) } else { U256::ZERO };
                self.push_with_optimization(result, &a, &modulus, operation);
            }

            // SMOD
            0x07 => {
                let a = self.stack.pop()?;
                let modulus = self.stack.pop()?;
                let result = if !modulus.value.is_zero() {
                    sign_uint(a.value).rem(sign_uint(modulus.value))
                } else {
                    I256::ZERO
                };
                self.push_with_optimization_signed(result, &a, &modulus, operation);
            }

            // ADDMOD
            0x08 => {
                let a = self.stack.pop()?;
                let b = self.stack.pop()?;
                let modulus = self.stack.pop()?;
                let result = if !modulus.value.is_zero() {
                    a.value.overflowing_add(b.value).0.rem(modulus.value)
                } else {
                    U256::ZERO
                };
                self.push_with_optimization(result, &a, &b, operation);
            }

            // MULMOD
            0x09 => {
                let a = self.stack.pop()?;
                let b = self.stack.pop()?;
                let modulus = self.stack.pop()?;
                let result = if !modulus.value.is_zero() {
                    a.value.overflowing_mul(b.value).0.rem(modulus.value)
                } else {
                    U256::ZERO
                };
                self.push_with_optimization(result, &a, &b, operation);
            }

            // EXP
            0x0A => {
                let a = self.stack.pop()?;
                let exponent = self.stack.pop()?;
                let result = a.value.overflowing_pow(exponent.value).0;

                // consume dynamic gas
                let exponent_byte_size = exponent.value.bit_len() / 8;
                let gas_cost = 50 * exponent_byte_size;
                self.consume_gas(gas_cost as u128);

                self.push_with_optimization(result, &a, &exponent, operation);
            }

            // SIGNEXTEND
            0x0B => {
                let x = self.stack.pop()?.value;
                let b = self.stack.pop()?.value;

                let t = x * U256::from(8u32) + U256::from(7u32);
                let sign_bit = U256::from(1u32) << t;

                // (b & sign_bit - 1) - (b & sign_bit)
                let result = (b & (sign_bit.overflowing_sub(U256::from(1u32)).0))
                    .overflowing_sub(b & sign_bit)
                    .0;

                self.stack.push(result, operation)
            }

            // LT
            0x10 => {
                let a = self.stack.pop()?.value;
                let b = self.stack.pop()?.value;
                self.push_boolean(a.lt(&b), operation);
            }

            // GT
            0x11 => {
                let a = self.stack.pop()?.value;
                let b = self.stack.pop()?.value;
                self.push_boolean(a.gt(&b), operation);
            }

            // SLT
            0x12 => {
                let a = self.stack.pop()?.value;
                let b = self.stack.pop()?.value;
                self.push_boolean(sign_uint(a).lt(&sign_uint(b)), operation);
            }

            // SGT
            0x13 => {
                let a = self.stack.pop()?.value;
                let b = self.stack.pop()?.value;
                self.push_boolean(sign_uint(a).gt(&sign_uint(b)), operation);
            }

            // EQ
            0x14 => {
                let a = self.stack.pop()?.value;
                let b = self.stack.pop()?.value;
                self.push_boolean(a.eq(&b), operation);
            }

            // ISZERO
            0x15 => {
                let a = self.stack.pop()?.value;
                self.push_boolean(a.is_zero(), operation);
            }

            // AND
            0x16 => {
                let a = self.stack.pop()?;
                let b = self.stack.pop()?;
                let result = a.value & b.value;
                self.push_with_optimization(result, &a, &b, operation);
            }

            // OR
            0x17 => {
                let a = self.stack.pop()?;
                let b = self.stack.pop()?;
                let result = a.value | b.value;
                self.push_with_optimization(result, &a, &b, operation);
            }

            // XOR
            0x18 => {
                let a = self.stack.pop()?;
                let b = self.stack.pop()?;
                let result = a.value ^ b.value;
                self.push_with_optimization(result, &a, &b, operation);
            }

            // NOT
            0x19 => {
                let a = self.stack.pop()?;
                let result = !a.value;
                self.push_with_optimization_single(result, &a, operation);
            }

            // BYTE
            0x1A => {
                let b = self.stack.pop()?.value;
                let a = self.stack.pop()?.value;
                let result = if b >= U256::from(32u32) {
                    U256::ZERO
                } else {
                    a / (U256::from(256u32).pow(U256::from(31u32) - b)) % U256::from(256u32)
                };
                self.stack.push(result, operation);
            }

            // SHL
            0x1B => {
                let a = self.stack.pop()?;
                let b = self.stack.pop()?;
                let result =
                    if a.value > U256::from(255u8) { U256::ZERO } else { b.value.shl(a.value) };
                self.push_with_optimization(result, &a, &b, operation);
            }

            // SHR
            0x1C => {
                let a = self.stack.pop()?;
                let b = self.stack.pop()?;
                let result =
                    if a.value > U256::from(255u8) { U256::ZERO } else { b.value.shr(a.value) };
                self.push_with_optimization(result, &a, &b, operation);
            }

            // SAR
            0x1D => {
                let a = self.stack.pop()?;
                let b = self.stack.pop()?;
                let usize_a: usize = a.value.try_into().unwrap_or(usize::MAX);
                let result =
                    if !b.value.is_zero() { sign_uint(b.value).shr(usize_a) } else { I256::ZERO };
                self.push_with_optimization_signed(result, &a, &b, operation);
            }

            // SHA3
            0x20 => {
                let offset = self.stack.pop()?.value;
                let size = self.stack.pop()?.value;

                // Safely convert U256 to usize
                let offset: usize = offset.try_into().unwrap_or(usize::MAX);
                let size: usize = size.try_into().unwrap_or(usize::MAX);

                let data = self.memory.read(offset, size);
                let result = keccak256(data);

                // consume dynamic gas
                let minimum_word_size = size.div_ceil(32) as u128;
                let gas_cost = 6 * minimum_word_size + self.memory.expansion_cost(offset, size);
                self.consume_gas(gas_cost);

                self.stack.push(U256::from_be_bytes(result.0), operation);
            }

            // ADDRESS
            0x30 => {
                self.stack.push(Self::address_to_u256(&self.address), operation);
            }

            // BALANCE
            0x31 => {
                let address = self.stack.pop()?.value;

                // consume dynamic gas
                if !self.address_access_set.contains(&address) {
                    self.consume_gas(2600);
                    self.address_access_set.insert(address);
                } else {
                    self.consume_gas(100);
                }

                // balance is set to 1 wei because we won't run into div by 0 errors
                self.stack.push(U256::from(1), operation);
            }

            // ORIGIN
            0x32 => {
                self.stack.push(Self::address_to_u256(&self.origin), operation);
            }

            // CALLER
            0x33 => {
                self.stack.push(Self::address_to_u256(&self.caller), operation);
            }

            // CALLVALUE
            0x34 => {
                self.stack.push(U256::from(self.value), operation);
            }

            // CALLDATALOAD
            0x35 => {
                let i = self.stack.pop()?.value;

                // Safely convert U256 to usize
                let i: usize = i.try_into().unwrap_or(usize::MAX);

                let result = if i.saturating_add(32) > self.calldata.len() {
                    let mut value = [0u8; 32];

                    if i <= self.calldata.len() {
                        value[..self.calldata.len() - i].copy_from_slice(&self.calldata[i..]);
                    }

                    U256::from_be_bytes(value)
                } else {
                    U256::from_be_slice(&self.calldata[i..i + 32])
                };

                self.stack.push(result, operation);
            }

            // CALLDATASIZE
            0x36 => {
                let result = U256::from(self.calldata.len());

                self.stack.push(result, operation);
            }

            // CALLDATACOPY
            0x37 => {
                let dest_offset = self.stack.pop()?.value;
                let offset = self.stack.pop()?.value;
                let size = self.stack.pop()?.value;

                let dest_offset: usize = dest_offset.try_into().unwrap_or(usize::MAX);
                let offset: usize = offset.try_into().unwrap_or(usize::MAX);
                let size: usize =
                    size.try_into().unwrap_or(self.calldata.len()).min(self.calldata.len());

                let value = Self::safe_copy_data(&self.calldata, offset, size);

                // consume dynamic gas
                let minimum_word_size = size.div_ceil(32) as u128;
                let gas_cost = 3 * minimum_word_size + self.memory.expansion_cost(offset, size);
                self.consume_gas(gas_cost);

                self.memory.store_with_opcode(
                    dest_offset,
                    size,
                    &value,
                    #[cfg(feature = "experimental")]
                    operation,
                );
            }

            // CODESIZE
            0x38 => {
                let result = U256::from(self.bytecode.len() as u128);

                self.stack.push(result, operation);
            }

            // CODECOPY
            0x39 => {
                let dest_offset = self.stack.pop()?.value;
                let offset = self.stack.pop()?.value;
                let size = self.stack.pop()?.value;

                let dest_offset: usize = dest_offset.try_into().unwrap_or(usize::MAX);
                let offset: usize = offset.try_into().unwrap_or(usize::MAX);
                let size: usize =
                    size.try_into().unwrap_or(self.bytecode.len()).min(self.bytecode.len());

                let value = Self::safe_copy_data(&self.bytecode, offset, size);

                // consume dynamic gas
                let minimum_word_size = size.div_ceil(32) as u128;
                let gas_cost = 3 * minimum_word_size + self.memory.expansion_cost(offset, size);
                self.consume_gas(gas_cost);

                self.memory.store_with_opcode(
                    dest_offset,
                    size,
                    &value,
                    #[cfg(feature = "experimental")]
                    operation,
                );
            }

            // GASPRICE
            0x3A => {
                self.stack.push(U256::from(1), operation);
            }

            // EXTCODESIZE
            0x3B => {
                let address = self.stack.pop()?.value;

                // consume dynamic gas
                if !self.address_access_set.contains(&address) {
                    self.consume_gas(2600);
                    self.address_access_set.insert(address);
                } else {
                    self.consume_gas(100);
                }

                self.stack.push(U256::from(1), operation);
            }

            // EXTCODECOPY
            0x3C => {
                let address = self.stack.pop()?.value;
                let dest_offset = self.stack.pop()?.value;
                self.stack.pop()?;
                let size = self.stack.pop()?.value;

                // Safely convert U256 to usize
                let dest_offset: usize = dest_offset.try_into().unwrap_or(0);
                let mut size: usize = size.try_into().unwrap_or(256);
                size = size.max(256);

                let mut value = Vec::with_capacity(size);
                value.fill(0xff);

                // consume dynamic gas
                let minimum_word_size = size.div_ceil(32) as u128;
                let gas_cost =
                    3 * minimum_word_size + self.memory.expansion_cost(dest_offset, size);
                self.consume_gas(gas_cost);
                if !self.address_access_set.contains(&address) {
                    self.consume_gas(2600);
                    self.address_access_set.insert(address);
                } else {
                    self.consume_gas(100);
                }

                self.memory.store_with_opcode(
                    dest_offset,
                    size,
                    &value,
                    #[cfg(feature = "experimental")]
                    operation,
                );
            }

            // RETURNDATASIZE
            0x3D => {
                self.stack.push(U256::from(1u8), operation);
            }

            // RETURNDATACOPY
            0x3E => {
                let dest_offset = self.stack.pop()?.value;
                self.stack.pop()?;
                let size = self.stack.pop()?.value;

                // Safely convert U256 to usize
                let dest_offset: usize = dest_offset.try_into().unwrap_or(0);
                let size: usize = size.try_into().unwrap_or(256);

                let mut value = Vec::with_capacity(size);
                value.fill(0xff);

                // consume dynamic gas
                let minimum_word_size = size.div_ceil(32) as u128;
                let gas_cost =
                    3 * minimum_word_size + self.memory.expansion_cost(dest_offset, size);
                self.consume_gas(gas_cost);

                self.memory.store_with_opcode(
                    dest_offset,
                    size,
                    &value,
                    #[cfg(feature = "experimental")]
                    operation,
                );
            }

            // EXTCODEHASH and BLOCKHASH
            0x3F | 0x40 => {
                let address = self.stack.pop()?.value;

                // consume dynamic gas
                if opcode == 0x3f && !self.address_access_set.contains(&address) {
                    self.consume_gas(2600);
                    self.address_access_set.insert(address);
                } else if opcode == 0x3f {
                    self.consume_gas(100);
                }

                self.stack.push(U256::ZERO, operation);
            }

            // COINBASE
            0x41 => {
                self.stack.push(*COINBASE_ADDRESS, operation);
            }

            // TIMESTAMP
            0x42 => {
                let timestamp =
                    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();

                self.stack.push(U256::from(timestamp), operation);
            }

            // NUMBER -> BLOBBASEFEE
            (0x43..=0x4a) => {
                self.stack.push(U256::from(1u8), operation);
            }

            // POP
            0x50 => {
                self.stack.pop()?;
            }

            // MLOAD
            0x51 => {
                let i = self.stack.pop()?.value;
                let i: usize = i.try_into().unwrap_or(usize::MAX);

                let result = U256::from_be_slice(self.memory.read(i, 32).as_slice());

                // consume dynamic gas
                let gas_cost = self.memory.expansion_cost(i, 32);
                self.consume_gas(gas_cost);

                self.stack.push(result, operation);
            }

            // MSTORE
            0x52 => {
                let offset = self.stack.pop()?.value;
                let value = self.stack.pop()?.value;

                // Safely convert U256 to usize
                let offset: usize = offset.try_into().unwrap_or(usize::MAX);

                // consume dynamic gas
                let gas_cost = self.memory.expansion_cost(offset, 32);
                self.consume_gas(gas_cost);

                self.memory.store_with_opcode(
                    offset,
                    32,
                    &value.to_be_bytes_vec(),
                    #[cfg(feature = "experimental")]
                    operation,
                );
            }

            // MSTORE8
            0x53 => {
                let offset = self.stack.pop()?.value;
                let value = self.stack.pop()?.value;

                // Safely convert U256 to usize
                let offset: usize = offset.try_into().unwrap_or(usize::MAX);

                // consume dynamic gas
                let gas_cost = self.memory.expansion_cost(offset, 1);
                self.consume_gas(gas_cost);

                self.memory.store_with_opcode(
                    offset,
                    1,
                    &[value.to_be_bytes_vec()[31]],
                    #[cfg(feature = "experimental")]
                    operation,
                );
            }

            // SLOAD
            0x54 => {
                let key = self.stack.pop()?.value;

                // consume dynamic gas
                let gas_cost = self.storage.access_cost(key);
                self.consume_gas(gas_cost);

                self.stack.push(U256::from(self.storage.load(key)), operation)
            }

            // SSTORE
            0x55 => {
                let key = self.stack.pop()?.value;
                let value = self.stack.pop()?.value;

                // consume dynamic gas
                let gas_cost = self.storage.storage_cost(key, value);
                self.consume_gas(gas_cost);

                self.storage.store(key, value);
            }

            // JUMP
            0x56 => {
                let pc = self.stack.pop()?.value;

                // Safely convert U256 to u128
                let pc: u128 = pc.try_into().unwrap_or(u128::MAX);

                // Check if JUMPDEST is valid and throw with 790 if not (invalid jump destination)
                if (pc <=
                    self.bytecode
                        .len()
                        .try_into()
                        .expect("impossible case: bytecode is larger than u128::MAX")) &&
                    (self.bytecode[pc as usize] != 0x5b)
                {
                    self.exit(790, Vec::new());
                    return Ok(Instruction {
                        instruction: last_instruction,
                        opcode,
                        inputs,
                        outputs: Vec::new(),
                        input_operations,
                        output_operations: Vec::new(),
                    });
                } else {
                    self.instruction = pc + 1;
                }
            }

            // JUMPI
            0x57 => {
                let pc = self.stack.pop()?.value;
                let condition = self.stack.pop()?.value;

                // Safely convert U256 to u128
                let pc: u128 = pc.try_into().unwrap_or(u128::MAX);

                if !condition.is_zero() {
                    // Check if JUMPDEST is valid and throw with 790 if not (invalid jump
                    // destination)
                    if (pc <
                        self.bytecode
                            .len()
                            .try_into()
                            .expect("impossible case: bytecode is larger than u128::MAX")) &&
                        (self.bytecode[pc as usize] != 0x5b)
                    {
                        self.exit(790, Vec::new());
                        return Ok(Instruction {
                            instruction: last_instruction,
                            opcode,
                            inputs,
                            outputs: Vec::new(),
                            input_operations,
                            output_operations: Vec::new(),
                        });
                    } else {
                        self.instruction = pc + 1;
                    }
                }
            }

            // JUMPDEST
            0x5B => {}

            // TLOAD
            0x5C => {
                let key = self.stack.pop()?.value;
                self.stack.push(U256::from(self.storage.tload(key)), operation)
            }

            // TSTORE
            0x5D => {
                let key = self.stack.pop()?.value;
                let value = self.stack.pop()?.value;
                self.storage.tstore(key, value);
            }

            // MCOPY
            0x5E => {
                let dest_offset = self.stack.pop()?.value;
                let offset = self.stack.pop()?.value;
                let size = self.stack.pop()?.value;

                let dest_offset: usize = dest_offset.try_into().unwrap_or(u128::MAX as usize);
                let offset: usize = offset.try_into().unwrap_or(u128::MAX as usize);
                let memory_size: usize =
                    self.memory.size().try_into().expect("failed to convert u128 to usize");
                let size: usize = size.try_into().unwrap_or(memory_size).min(memory_size);

                let value = Self::safe_copy_data(&self.memory.memory, offset, size);

                // consume dynamic gas
                let minimum_word_size = size.div_ceil(32) as u128;
                let gas_cost = 3 * minimum_word_size + self.memory.expansion_cost(offset, size);
                self.consume_gas(gas_cost);

                self.memory.store_with_opcode(
                    dest_offset,
                    size,
                    &value,
                    #[cfg(feature = "experimental")]
                    operation,
                );
            }

            // PC
            0x58 => {
                self.stack.push(U256::from(self.instruction), operation);
            }

            // MSIZE
            0x59 => {
                self.stack.push(U256::from(self.memory.size()), operation);
            }

            // GAS
            0x5a => {
                self.stack.push(U256::from(self.gas_remaining), operation);
            }

            // PUSH0
            0x5f => {
                self.stack.push(U256::ZERO, operation);
            }

            // PUSH1 -> PUSH32
            (0x60..=0x7F) => {
                // Get the number of bytes to push
                let num_bytes = (opcode - 95) as u128;

                // Get the bytes to push from bytecode
                let bytes = &self.bytecode
                    [(self.instruction - 1) as usize..(self.instruction - 1 + num_bytes) as usize];
                self.instruction += num_bytes;

                // update the operation's inputs
                let new_operation_inputs = vec![WrappedInput::Raw(U256::from_be_slice(bytes))];

                operation.inputs = new_operation_inputs;

                // Push the bytes to the stack
                self.stack.push(U256::from_be_slice(bytes), operation);
            }

            // DUP1 -> DUP16
            (0x80..=0x8F) => {
                // Get the number of items to swap
                let index = opcode - 127;

                // Perform the swap
                self.stack.dup(index as usize);
            }

            // SWAP1 -> SWAP16
            (0x90..=0x9F) => {
                // Get the number of items to swap
                let index = opcode - 143;

                // Perform the swap
                self.stack.swap(index as usize);
            }

            // LOG0 -> LOG4
            (0xA0..=0xA4) => {
                let topic_count = opcode - 160;
                let offset = self.stack.pop()?.value;
                let size = self.stack.pop()?.value;
                let topics =
                    self.stack.pop_n(topic_count as usize).iter().map(|x| x.value).collect();

                // Safely convert U256 to usize
                let offset: usize = offset.try_into().unwrap_or(usize::MAX);
                let size: usize = size.try_into().unwrap_or(usize::MAX);

                let data = self.memory.read(offset, size);

                // consume dynamic gas
                let gas_cost = (375 * (topic_count as u128)) +
                    8 * (size as u128) +
                    self.memory.expansion_cost(offset, size);
                self.consume_gas(gas_cost);

                // no need for a panic check because the length of events should never be larger
                // than a u128
                self.events.push(Log::new(
                    self.events
                        .len()
                        .try_into()
                        .expect("impossible case: log_index is larger than u128::MAX"),
                    topics,
                    &data,
                ))
            }

            // CREATE
            0xF0 => {
                self.stack.pop_n(3);

                self.stack.push(*CREATE_ADDRESS, operation);
            }

            // CALL, CALLCODE
            0xF1 | 0xF2 => {
                let address = self.stack.pop()?.value;
                self.stack.pop_n(6);

                // consume dynamic gas
                if !self.address_access_set.contains(&address) {
                    self.consume_gas(2600);
                    self.address_access_set.insert(address);
                } else {
                    self.consume_gas(100);
                }

                self.stack.push(U256::from(1u8), operation);
            }

            // RETURN
            0xF3 => {
                let offset = self.stack.pop()?.value;
                let size = self.stack.pop()?.value;

                // Safely convert U256 to usize
                let offset: usize = offset.try_into().unwrap_or(usize::MAX);
                let size: usize = size.try_into().unwrap_or(usize::MAX);

                // consume dynamic gas
                let gas_cost = self.memory.expansion_cost(offset, size);
                self.consume_gas(gas_cost);

                self.exit(0, self.memory.read(offset, size));
            }

            // DELEGATECALL, STATICCALL
            0xF4 | 0xFA => {
                let address = self.stack.pop()?.value;
                self.stack.pop_n(5);

                // consume dynamic gas
                if !self.address_access_set.contains(&address) {
                    self.consume_gas(2600);
                    self.address_access_set.insert(address);
                } else {
                    self.consume_gas(100);
                }

                self.stack.push(U256::from(1u8), operation);
            }

            // CREATE2
            0xF5 => {
                self.stack.pop_n(4);

                self.stack.push(*CREATE2_ADDRESS, operation);
            }

            // REVERT
            0xFD => {
                let offset = self.stack.pop()?.value;
                let size = self.stack.pop()?.value;

                // Safely convert U256 to usize
                let offset: usize = offset.try_into().unwrap_or(usize::MAX);
                let size: usize = size.try_into().unwrap_or(usize::MAX);

                self.exit(1, self.memory.read(offset, size));
            }

            // INVALID & SELFDESTRUCT
            _ => {
                self.exit(1, Vec::new());
            }
        }

        // get outputs
        let output_frames = self.stack.peek_n(opcode_info.outputs() as usize);
        let output_operations =
            output_frames.iter().map(|x| x.operation.clone()).collect::<Vec<WrappedOpcode>>();
        let outputs = output_frames.iter().map(|x| x.value).collect::<Vec<U256>>();

        // if step-tracing feature is enabled, print the current operation
        #[cfg(feature = "step-tracing")]
        trace!(
            pc = self.instruction - 1,
            opcode = opcode_info.name(),
            outputs = ?outputs
                .iter()
                .map(|x| format!("{x:#x}"))
                .collect::<Vec<String>>(),
            elapsed = ?Instant::now().duration_since(start_time),
            "done executing opcode"
        );

        #[cfg(feature = "step-tracing")]
        {
            trace!(
                ops_per_sec =
                    (self.operation_count as f64 / self.start_time.elapsed().as_secs_f64()),
                mem_size = self.memory.size(),
                stack_size = self.stack.size(),
                "_step.end"
            );
        }

        Ok(Instruction {
            instruction: last_instruction,
            opcode,
            inputs,
            outputs,
            input_operations,
            output_operations,
        })
    }

    /// Executes the next instruction in the VM and returns a snapshot of the VM state after
    /// executing the instruction
    ///
    /// ```
    /// use heimdall_vm::core::vm::VM;
    /// use alloy::primitives::Address;
    ///
    /// let mut vm = VM::new(
    ///     &vec![0x00],
    ///     &vec![],
    ///     "0x0000000000000000000000000000000000000000".parse::<Address>().expect("failed to parse Address"),
    ///     "0x0000000000000000000000000000000000000001".parse::<Address>().expect("failed to parse Address"),
    ///     "0x0000000000000000000000000000000000000002".parse::<Address>().expect("failed to parse Address"),
    ///     0,
    ///     1000000000000000000,
    /// );
    ///
    /// vm.step(); // 0x00 EXIT
    /// assert_eq!(vm.exitcode, 10);
    /// ```
    pub fn step(&mut self) -> Result<State> {
        let instruction = self._step()?;

        Ok(State {
            last_instruction: instruction,
            gas_used: self.gas_used,
            gas_remaining: self.gas_remaining,
            stack: self.stack.clone(),
            memory: self.memory.clone(),
            storage: self.storage.clone(),
            events: self.events.clone(),
        })
    }

    /// View the next n instructions without executing them
    ///
    /// ```
    /// use heimdall_vm::core::vm::VM;
    /// use alloy::primitives::Address;
    ///
    /// let mut vm = VM::new(
    ///     &vec![0x00],
    ///     &vec![],
    ///     "0x0000000000000000000000000000000000000000".parse::<Address>().expect("failed to parse Address"),
    ///     "0x0000000000000000000000000000000000000001".parse::<Address>().expect("failed to parse Address"),
    ///     "0x0000000000000000000000000000000000000002".parse::<Address>().expect("failed to parse Address"),
    ///     0,
    ///     1000000000000000000,
    /// );
    ///
    /// vm.peek(1); // 0x00 EXIT (not executed)
    /// assert_eq!(vm.exitcode, 255);
    /// ```
    pub fn peek(&mut self, n: usize) -> Result<Vec<State>> {
        let mut states = Vec::new();
        let mut vm_clone = self.clone();

        for _ in 0..n {
            if vm_clone.bytecode.len() < vm_clone.instruction as usize ||
                vm_clone.exitcode != 255 ||
                !vm_clone.returndata.is_empty()
            {
                break;
            }
            states.push(vm_clone.step()?);
        }

        Ok(states)
    }

    /// Resets the VM state for a new execution
    ///
    /// ```
    /// use heimdall_vm::core::vm::VM;
    /// use alloy::primitives::Address;
    ///
    /// let mut vm = VM::new(
    ///     &vec![0x00],
    ///     &vec![],
    ///     "0x0000000000000000000000000000000000000000".parse::<Address>().expect("failed to parse Address"),
    ///     "0x0000000000000000000000000000000000000001".parse::<Address>().expect("failed to parse Address"),
    ///     "0x0000000000000000000000000000000000000002".parse::<Address>().expect("failed to parse Address"),
    ///     0,
    ///     1000000000000000000,
    /// );
    ///
    /// vm.step(); // 0x00 EXIT (not executed)
    /// assert_eq!(vm.exitcode, 10);
    ///
    /// vm.reset();
    /// assert_eq!(vm.exitcode, 255);
    /// ```
    pub fn reset(&mut self) {
        self.stack = Stack::new();
        self.memory = Memory::new();
        self.instruction = 1;
        self.gas_remaining = (self.gas_used + self.gas_remaining).max(21000) - 21000;
        self.gas_used = 21000;
        self.events = Vec::new();
        self.returndata = Vec::new();
        self.exitcode = 255;
    }

    /// Executes the code until finished
    ///
    /// ```
    /// use heimdall_vm::core::vm::VM;
    /// use alloy::primitives::Address;
    ///
    /// let mut vm = VM::new(
    ///     &vec![0x00],
    ///     &vec![],
    ///     "0x0000000000000000000000000000000000000000".parse::<Address>().expect("failed to parse Address"),
    ///     "0x0000000000000000000000000000000000000001".parse::<Address>().expect("failed to parse Address"),
    ///     "0x0000000000000000000000000000000000000002".parse::<Address>().expect("failed to parse Address"),
    ///     0,
    ///     1000000000000000000,
    /// );
    ///
    /// vm.execute().expect("execution failed!"); // 0x00 EXIT (not executed)
    /// assert_eq!(vm.exitcode, 10);
    /// ```
    pub fn execute(&mut self) -> Result<ExecutionResult> {
        while self.bytecode.len() >= self.instruction as usize {
            self.step()?;

            if self.exitcode != 255 || !self.returndata.is_empty() {
                break;
            }
        }

        Ok(ExecutionResult {
            gas_used: self.gas_used,
            gas_remaining: self.gas_remaining,
            returndata: self.returndata.to_owned(),
            exitcode: self.exitcode,
            events: self.events.clone(),
            instruction: self.instruction,
        })
    }

    /// Executes provided calldata until finished
    ///
    /// ```
    /// use heimdall_vm::core::vm::VM;
    /// use alloy::primitives::Address;
    ///
    /// let mut vm = VM::new(
    ///     &vec![0x00],
    ///     &vec![],
    ///     "0x0000000000000000000000000000000000000000".parse::<Address>().expect("failed to parse Address"),
    ///     "0x0000000000000000000000000000000000000001".parse::<Address>().expect("failed to parse Address"),
    ///     "0x0000000000000000000000000000000000000002".parse::<Address>().expect("failed to parse Address"),
    ///     0,
    ///     1000000000000000000,
    /// );
    ///
    /// vm.call(&vec![], 0);
    /// assert_eq!(vm.exitcode, 10);
    /// ```
    pub fn call(&mut self, calldata: &[u8], value: u128) -> Result<ExecutionResult> {
        // reset the VM temp state
        self.reset();
        calldata.clone_into(&mut self.calldata);
        self.value = value;

        self.execute()
    }
}

#[cfg(test)]
mod tests {

    use std::str::FromStr;

    use heimdall_common::utils::strings::decode_hex;

    use super::*;

    // creates a new test VM with calldata.
    fn new_test_vm(bytecode: &str) -> VM {
        VM::new(
            &decode_hex(bytecode).expect("failed to decode bytecode"),
            &decode_hex("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF")
                .expect("failed to decode calldata"),
            "0x6865696d64616c6c000000000061646472657373"
                .parse::<Address>()
                .expect("failed to parse Address"),
            "0x6865696d64616c6c0000000000006f726967696e"
                .parse::<Address>()
                .expect("failed to parse Address"),
            "0x6865696d64616c6c00000000000063616c6c6572"
                .parse::<Address>()
                .expect("failed to parse Address"),
            0,
            9999999999,
        )
    }

    #[test]
    fn test_stop_vm() {
        let mut vm = new_test_vm("0x00");
        vm.execute().expect("execution failed!");

        assert!(vm.returndata.is_empty());
        assert_eq!(vm.exitcode, 10);
    }

    #[test]
    fn test_pc_out_of_range() {
        let mut vm = new_test_vm("0x");
        vm.execute().expect("execution failed!");

        assert!(vm.returndata.is_empty());
        assert_eq!(vm.exitcode, 255);
    }

    #[test]
    fn test_add() {
        let mut vm = new_test_vm(
            "0x600a600a017fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff600101",
        );
        vm.execute().expect("execution failed!");

        assert_eq!(vm.stack.peek(1).value, U256::from_str("0x14").expect("failed to parse hex"));
        assert_eq!(vm.stack.peek(0).value, U256::from_str("0x00").expect("failed to parse hex"));
    }

    #[test]
    fn test_mul() {
        let mut vm = new_test_vm(
            "0x600a600a027fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff600202",
        );
        vm.execute().expect("execution failed!");

        assert_eq!(vm.stack.peek(1).value, U256::from_str("0x64").expect("failed to parse hex"));
        assert_eq!(
            vm.stack.peek(0).value,
            U256::from_str("0xfffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe")
                .expect("failed to parse hex")
        );
    }

    #[test]
    fn test_sub() {
        let mut vm = new_test_vm("0x600a600a036001600003");
        vm.execute().expect("execution failed!");

        assert_eq!(vm.stack.peek(1).value, U256::from_str("0x00").expect("failed to parse hex"));
        assert_eq!(
            vm.stack.peek(0).value,
            U256::from_str("0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff")
                .expect("failed to parse hex")
        );
    }

    #[test]
    fn test_div() {
        let mut vm = new_test_vm("0x600a600a046002600104");
        vm.execute().expect("execution failed!");

        assert_eq!(vm.stack.peek(1).value, U256::from_str("0x01").expect("failed to parse hex"));
        assert_eq!(vm.stack.peek(0).value, U256::from_str("0x00").expect("failed to parse hex"));
    }

    #[test]
    fn test_div_by_zero() {
        let mut vm = new_test_vm("0x6002600004");
        vm.execute().expect("execution failed!");

        assert_eq!(vm.stack.peek(0).value, U256::from_str("0x00").expect("failed to parse hex"));
    }

    #[test]
    fn test_sdiv() {
        let mut vm = new_test_vm("0x600a600a057fFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF7fFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFE05");
        vm.execute().expect("execution failed!");

        assert_eq!(vm.stack.peek(1).value, U256::from_str("0x01").expect("failed to parse hex"));
        assert_eq!(vm.stack.peek(0).value, U256::from_str("0x02").expect("failed to parse hex"));
    }

    #[test]
    fn test_sdiv_by_zero() {
        let mut vm = new_test_vm("0x6002600005");
        vm.execute().expect("execution failed!");

        assert_eq!(vm.stack.peek(0).value, U256::from_str("0x00").expect("failed to parse hex"));
    }

    #[test]
    fn test_mod() {
        let mut vm = new_test_vm("0x6003600a066005601106");
        vm.execute().expect("execution failed!");

        assert_eq!(vm.stack.peek(1).value, U256::from_str("0x01").expect("failed to parse hex"));
        assert_eq!(vm.stack.peek(0).value, U256::from_str("0x02").expect("failed to parse hex"));
    }

    #[test]
    fn test_mod_by_zero() {
        let mut vm = new_test_vm("0x6002600006");
        vm.execute().expect("execution failed!");

        assert_eq!(vm.stack.peek(0).value, U256::from_str("0x00").expect("failed to parse hex"));
    }

    #[test]
    fn test_smod() {
        let mut vm = new_test_vm("0x6003600a077ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffd7ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff807");
        vm.execute().expect("execution failed!");

        assert_eq!(vm.stack.peek(1).value, U256::from_str("0x01").expect("failed to parse hex"));
        assert_eq!(
            vm.stack.peek(0).value,
            U256::from_str("0xfffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe")
                .expect("failed to parse hex")
        );
    }

    #[test]
    fn test_smod_by_zero() {
        let mut vm = new_test_vm("0x6002600007");
        vm.execute().expect("execution failed!");

        assert_eq!(vm.stack.peek(0).value, U256::from_str("0x00").expect("failed to parse hex"));
    }

    #[test]
    fn test_addmod() {
        let mut vm = new_test_vm("0x6008600a600a08600260027fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff08");
        vm.execute().expect("execution failed!");

        assert_eq!(vm.stack.peek(1).value, U256::from_str("0x04").expect("failed to parse hex"));
        assert_eq!(vm.stack.peek(0).value, U256::from_str("0x01").expect("failed to parse hex"));
    }

    #[test]
    fn test_addmod_by_zero() {
        let mut vm = new_test_vm("0x60026000600008");
        vm.execute().expect("execution failed!");

        assert_eq!(vm.stack.peek(0).value, U256::from_str("0x00").expect("failed to parse hex"));
    }

    #[test]
    fn test_mulmod() {
        let mut vm = new_test_vm("0x6008600a600a09600c7fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff7fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff09");
        vm.execute().expect("execution failed!");

        assert_eq!(vm.stack.peek(1).value, U256::from_str("0x04").expect("failed to parse hex"));
        assert_eq!(vm.stack.peek(0).value, U256::from_str("0x01").expect("failed to parse hex"));
    }

    #[test]
    fn test_mulmod_by_zero() {
        let mut vm = new_test_vm("0x60026000600009");
        vm.execute().expect("execution failed!");

        assert_eq!(vm.stack.peek(0).value, U256::from_str("0x00").expect("failed to parse hex"));
    }

    #[test]
    fn test_exp() {
        let mut vm = new_test_vm("0x6002600a0a600260020a");
        vm.execute().expect("execution failed!");

        assert_eq!(vm.stack.peek(1).value, U256::from_str("0x64").expect("failed to parse hex"));
        assert_eq!(vm.stack.peek(0).value, U256::from_str("0x04").expect("failed to parse hex"));
    }

    #[test]
    fn test_signextend() {
        let mut vm = new_test_vm("0x60ff60000b607f60000b");
        vm.execute().expect("execution failed!");

        assert_eq!(
            vm.stack.peek(1).value,
            U256::from_str("0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff")
                .expect("failed to parse hex")
        );
        assert_eq!(vm.stack.peek(0).value, U256::from_str("0x7f").expect("failed to parse hex"));
    }

    #[test]
    fn test_lt() {
        let mut vm = new_test_vm("0x600a600910600a600a10");
        vm.execute().expect("execution failed!");

        assert_eq!(vm.stack.peek(1).value, U256::from_str("0x01").expect("failed to parse hex"));
        assert_eq!(vm.stack.peek(0).value, U256::from_str("0x00").expect("failed to parse hex"));
    }

    #[test]
    fn test_gt() {
        let mut vm = new_test_vm("0x6009600a11600a600a10");
        vm.execute().expect("execution failed!");

        assert_eq!(vm.stack.peek(1).value, U256::from_str("0x01").expect("failed to parse hex"));
        assert_eq!(vm.stack.peek(0).value, U256::from_str("0x00").expect("failed to parse hex"));
    }

    #[test]
    fn test_slt() {
        let mut vm = new_test_vm(
            "0x60097fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff12600a600a12",
        );
        vm.execute().expect("execution failed!");

        assert_eq!(vm.stack.peek(1).value, U256::from_str("0x01").expect("failed to parse hex"));
        assert_eq!(vm.stack.peek(0).value, U256::from_str("0x00").expect("failed to parse hex"));
    }

    #[test]
    fn test_sgt() {
        let mut vm = new_test_vm(
            "0x7fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff600913600a600a13",
        );
        vm.execute().expect("execution failed!");

        assert_eq!(vm.stack.peek(1).value, U256::from_str("0x01").expect("failed to parse hex"));
        assert_eq!(vm.stack.peek(0).value, U256::from_str("0x00").expect("failed to parse hex"));
    }

    #[test]
    fn test_eq() {
        let mut vm = new_test_vm("0x600a600a14600a600514");
        vm.execute().expect("execution failed!");

        assert_eq!(vm.stack.peek(1).value, U256::from_str("0x01").expect("failed to parse hex"));
        assert_eq!(vm.stack.peek(0).value, U256::from_str("0x00").expect("failed to parse hex"));
    }

    #[test]
    fn test_iszero() {
        let mut vm = new_test_vm("0x600015600a15");
        vm.execute().expect("execution failed!");

        assert_eq!(vm.stack.peek(1).value, U256::from_str("0x01").expect("failed to parse hex"));
        assert_eq!(vm.stack.peek(0).value, U256::from_str("0x00").expect("failed to parse hex"));
    }

    #[test]
    fn test_and() {
        let mut vm = new_test_vm("0x600f600f16600060ff1600");
        vm.execute().expect("execution failed!");

        assert_eq!(vm.stack.peek(1).value, U256::from_str("0x0F").expect("failed to parse hex"));
        assert_eq!(vm.stack.peek(0).value, U256::from_str("0x00").expect("failed to parse hex"));
    }

    #[test]
    fn test_or() {
        let mut vm = new_test_vm("0x600f60f01760ff60ff17");
        vm.execute().expect("execution failed!");

        assert_eq!(vm.stack.peek(1).value, U256::from_str("0xff").expect("failed to parse hex"));
        assert_eq!(vm.stack.peek(0).value, U256::from_str("0xff").expect("failed to parse hex"));
    }

    #[test]
    fn test_xor() {
        let mut vm = new_test_vm("0x600f60f01860ff60ff18");
        vm.execute().expect("execution failed!");

        assert_eq!(vm.stack.peek(1).value, U256::from_str("0xff").expect("failed to parse hex"));
        assert_eq!(vm.stack.peek(0).value, U256::from_str("0x00").expect("failed to parse hex"));
    }

    #[test]
    fn test_not() {
        let mut vm = new_test_vm("0x600019");
        vm.execute().expect("execution failed!");

        assert_eq!(
            vm.stack.peek(0).value,
            U256::from_str("0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff")
                .expect("failed to parse hex")
        );
    }

    #[test]
    fn test_byte() {
        let mut vm = new_test_vm("0x60ff601f1a61ff00601e1a");
        vm.execute().expect("execution failed!");

        assert_eq!(vm.stack.peek(1).value, U256::from_str("0xff").expect("failed to parse hex"));
        assert_eq!(vm.stack.peek(0).value, U256::from_str("0xff").expect("failed to parse hex"));
    }

    #[test]
    fn test_shl() {
        let mut vm = new_test_vm(
            "600160011b7fFF0000000000000000000000000000000000000000000000000000000000000060041b",
        );
        vm.execute().expect("execution failed!");

        assert_eq!(vm.stack.peek(1).value, U256::from_str("0x02").expect("failed to parse hex"));
        assert_eq!(
            vm.stack.peek(0).value,
            U256::from_str("0xF000000000000000000000000000000000000000000000000000000000000000")
                .expect("failed to parse hex")
        );
    }

    #[test]
    fn test_shl_gt_255() {
        let mut vm = new_test_vm(
            "600161ffff1b7fFF0000000000000000000000000000000000000000000000000000000000000060041b",
        );
        vm.execute().expect("execution failed!");

        assert_eq!(vm.stack.peek(1).value, U256::from_str("0x00").expect("failed to parse hex"));
        assert_eq!(
            vm.stack.peek(0).value,
            U256::from_str("0xF000000000000000000000000000000000000000000000000000000000000000")
                .expect("failed to parse hex")
        );
    }

    #[test]
    fn test_shr() {
        let mut vm = new_test_vm("600260011c60ff60041c");
        vm.execute().expect("execution failed!");

        assert_eq!(vm.stack.peek(1).value, U256::from_str("0x01").expect("failed to parse hex"));
        assert_eq!(vm.stack.peek(0).value, U256::from_str("0x0f").expect("failed to parse hex"));
    }

    #[test]
    fn test_shr_gt_256() {
        let mut vm = new_test_vm("600261ffff1c61ffff60041c");
        vm.execute().expect("execution failed!");

        assert_eq!(vm.stack.peek(1).value, U256::from_str("0x00").expect("failed to parse hex"));
        assert_eq!(vm.stack.peek(0).value, U256::from_str("0x0fff").expect("failed to parse hex"));
    }

    #[test]
    fn test_shr_zero() {
        let mut vm = new_test_vm("0x600060011c");
        vm.execute().expect("execution failed!");

        assert_eq!(vm.stack.peek(0).value, U256::from_str("0x00").expect("failed to parse hex"));
    }

    #[test]
    fn test_sar() {
        let mut vm = new_test_vm("600260011d");
        vm.execute().expect("execution failed!");

        assert_eq!(vm.stack.peek(0).value, U256::from_str("0x01").expect("failed to parse hex"));
    }

    #[test]
    fn test_sar_zero() {
        let mut vm = new_test_vm("0x600060011d");
        vm.execute().expect("execution failed!");

        assert_eq!(vm.stack.peek(0).value, U256::from_str("0x00").expect("failed to parse hex"));
    }

    #[test]
    fn test_sha3() {
        let mut vm = new_test_vm(
            "0x7fffffffff000000000000000000000000000000000000000000000000000000006000526004600020",
        );
        vm.execute().expect("execution failed!");

        assert_eq!(
            vm.stack.peek(0).value,
            U256::from_str("0x29045A592007D0C246EF02C2223570DA9522D0CF0F73282C79A1BC8F0BB2C238")
                .expect("failed to parse hex")
        );
    }

    #[test]
    fn test_address() {
        let mut vm = new_test_vm("0x30");
        vm.execute().expect("execution failed!");

        assert_eq!(
            vm.stack.peek(0).value,
            U256::from_str("0x6865696d64616c6c000000000061646472657373")
                .expect("failed to parse hex")
        );
    }

    #[test]
    fn test_calldataload() {
        let mut vm = new_test_vm("600035601f35");
        vm.execute().expect("execution failed!");

        assert_eq!(
            vm.stack.peek(1).value,
            U256::from_str("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF")
                .expect("failed to parse hex")
        );
        assert_eq!(
            vm.stack.peek(0).value,
            U256::from_str("0xFF00000000000000000000000000000000000000000000000000000000000000")
                .expect("failed to parse hex")
        );
    }

    #[test]
    fn test_calldatasize() {
        let mut vm = new_test_vm("0x36");
        vm.execute().expect("execution failed!");

        assert_eq!(vm.stack.peek(0).value, U256::from_str("0x20").expect("failed to parse hex"));
    }

    #[test]
    fn test_xdatacopy() {
        // returndatacopy, calldatacopy, etc share same code.
        let mut vm = new_test_vm("0x60ff6000600037");
        vm.execute().expect("execution failed!");
        assert_eq!(
            vm.memory.read(0, 32),
            decode_hex("FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF")
                .expect("failed to parse hex")
        );
    }

    #[test]
    fn test_mcopy() {
        let mut vm = new_test_vm("0x7f000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f6020526020602060005e");
        vm.execute().expect("execution failed!");
        assert_eq!(
            vm.memory.read(0, 64),
            decode_hex("000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f")
                .expect("failed to parse hex")
        );
    }

    #[test]
    fn test_mcopy_clamping_source_beyond_memory() {
        // Test copying from offset beyond current memory size
        // Store 32 bytes at memory[0x20], then try to copy from offset 0x40 (beyond memory)
        let mut vm = new_test_vm("0x7f000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f6020526020604060005e");
        vm.execute().expect("execution failed!");

        // Should copy zeros since source is beyond memory
        let result = vm.memory.read(0, 32);
        assert_eq!(result, vec![0u8; 32]);
    }

    #[test]
    fn test_mcopy_clamping_partial_source_beyond_memory() {
        // Test copying where source starts in memory but extends beyond it
        // Store 32 bytes at memory[0x20], then copy 64 bytes from offset 0x30
        let mut vm = new_test_vm("0x7f000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f6020526040603060005e");
        vm.execute().expect("execution failed!");

        // Should copy the available 16 bytes from memory[0x30-0x3F] then pad with zeros
        let result = vm.memory.read(0, 64);
        let expected = [
            &decode_hex("101112131415161718191a1b1c1d1e1f").expect("failed to parse hex")[..],
            &vec![0u8; 48][..],
        ]
        .concat();
        assert_eq!(result, expected);
    }

    #[test]
    fn test_mcopy_clamping_zero_size() {
        // Test copying zero bytes
        let mut vm = new_test_vm("0x7f000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f6020526000602060005e");
        vm.execute().expect("execution failed!");

        // Memory should only contain the original store at 0x20, destination at 0x00 should be
        // unchanged
        let result = vm.memory.read(0, 64);
        let expected = [
            &vec![0u8; 32][..],
            &decode_hex("000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f")
                .expect("failed to parse hex")[..],
        ]
        .concat();
        assert_eq!(result, expected);
    }

    #[test]
    fn test_mcopy_clamping_size_exceeds_memory() {
        // Test copying more bytes than available memory
        // Store 32 bytes at 0x20, then copy 64 bytes from 0x20 to 0x00
        // The copy overlaps, so source data at 0x20+ gets overwritten by destination
        let mut vm = new_test_vm("0x7f000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f6020526040602060005e");
        vm.execute().expect("execution failed!");

        // Since we copy 64 bytes from 0x20 to 0x00, and only 32 bytes exist at source:
        // - Bytes 0x00-0x1F get the original data from 0x20-0x3F
        // - Bytes 0x20-0x3F get overwritten with zeros (padding from beyond source)
        let result = vm.memory.read(0, 64);
        let expected = [
            &decode_hex("000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f")
                .expect("failed to parse hex")[..], // First 32 bytes: copied data
            &vec![0u8; 32][..], // Next 32 bytes: zeros (padding that overwrote the source)
        ]
        .concat();
        assert_eq!(result, expected);
    }

    #[test]
    fn test_mcopy_clamping_simple_overlap() {
        // Test a simpler overlapping copy case
        // Store data, then copy within the same memory region
        let mut vm = new_test_vm("0x7f000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f6000526010601060005e");
        vm.execute().expect("execution failed!");

        // Original data at 0x00: 000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f
        // Copy 16 bytes from 0x10 to 0x00: should copy 101112131415161718191a1b1c1d1e1f
        let result = vm.memory.read(0, 32);
        let expected = [
            &decode_hex("101112131415161718191a1b1c1d1e1f").expect("failed to parse hex")[..], // 0x00-0x0F: copied data
            &decode_hex("101112131415161718191a1b1c1d1e1f").expect("failed to parse hex")[..] // 0x10-0x1F: original data
        ].concat();
        assert_eq!(result, expected);
    }

    #[test]
    fn test_mcopy_clamping_large_offsets() {
        // Test with very large U256 offsets that get clamped to usize::MAX
        // This tests the try_into().unwrap_or() clamping behavior
        let mut vm = new_test_vm("0x7f000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f6020526020608060005e");
        vm.execute().expect("execution failed!");

        // Should handle large offset gracefully and copy zeros (since source is beyond memory)
        let result = vm.memory.read(0, 32);
        assert_eq!(result, vec![0u8; 32]);
    }

    #[test]
    fn test_mcopy_clamping_exact_memory_boundary() {
        // Test copying exactly at memory boundary
        // Store 32 bytes, then copy from the last valid offset
        let mut vm = new_test_vm("0x7f000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f6020526001603f60005e");
        vm.execute().expect("execution failed!");

        // Should copy the last byte of memory and pad with zero
        let result = vm.memory.read(0, 32);
        let expected = [
            &[0x1f][..],        // Last byte of the stored data
            &vec![0u8; 31][..], // Padding
        ]
        .concat();
        assert_eq!(result, expected);
    }

    #[test]
    fn test_mcopy_clamping_empty_memory() {
        // Test copying from empty memory (no prior stores)
        let mut vm = new_test_vm("0x6020602060005e");
        vm.execute().expect("execution failed!");

        // Should copy all zeros
        let result = vm.memory.read(0, 64);
        assert_eq!(result, vec![0u8; 64]);
    }

    #[test]
    fn test_codesize() {
        let mut vm = new_test_vm("0x60ff60ff60ff60ff60ff38");
        vm.execute().expect("execution failed!");

        assert_eq!(vm.stack.peek(0).value, U256::from_str("0x0B").expect("failed to parse hex"));
    }

    #[test]
    fn test_mload_mstore() {
        let mut vm = new_test_vm("0x7f00000000000000000000000000000000000000000000000000000000000000FF600052600051600151");
        vm.execute().expect("execution failed!");

        assert_eq!(vm.stack.peek(1).value, U256::from_str("0xff").expect("failed to parse hex"));
        assert_eq!(vm.stack.peek(0).value, U256::from_str("0xff00").expect("failed to parse hex"));
    }

    #[test]
    fn test_mstore8() {
        let mut vm = new_test_vm("0x60ff600053");
        vm.execute().expect("execution failed!");

        assert_eq!(
            vm.memory.read(0, 32),
            decode_hex("ff00000000000000000000000000000000000000000000000000000000000000")
                .expect("failed to parse hex")
        )
    }

    #[test]
    fn test_msize() {
        let mut vm = new_test_vm("0x60ff60005359");
        vm.execute().expect("execution failed!");

        assert_eq!(vm.stack.peek(0).value, U256::from_str("0x20").expect("failed to parse hex"));
    }

    #[test]
    fn test_sload_sstore() {
        let mut vm = new_test_vm("0x602e600055600054600154");
        vm.execute().expect("execution failed!");

        assert_eq!(vm.stack.peek(1).value, U256::from_str("0x2e").expect("failed to parse hex"));
        assert_eq!(vm.stack.peek(0).value, U256::from_str("0x00").expect("failed to parse hex"));
    }

    #[test]
    fn test_tload_tstore() {
        let mut vm = new_test_vm("0x602e60005d60005c60015c");
        vm.execute().expect("execution failed!");

        assert_eq!(vm.stack.peek(1).value, U256::from_str("0x2e").expect("failed to parse hex"));
        assert_eq!(vm.stack.peek(0).value, U256::from_str("0x00").expect("failed to parse hex"));
    }

    #[test]
    fn test_sstore_tstore_independence() {
        let mut vm = new_test_vm("0x60ff60015560fe60015d60015460015c");
        vm.execute().expect("execution failed!");

        assert_eq!(vm.stack.peek(1).value, U256::from_str("0xff").expect("failed to parse hex"));
        assert_eq!(vm.stack.peek(0).value, U256::from_str("0xfe").expect("failed to parse hex"));
    }

    #[test]
    fn test_jump() {
        let mut vm = new_test_vm("0x60fe56");
        vm.execute().expect("execution failed!");

        assert_eq!(
            U256::from(vm.instruction),
            U256::from_str("0xff").expect("failed to parse hex")
        );
    }

    #[test]
    fn test_jumpi() {
        let mut vm = new_test_vm("0x600160fe57");
        vm.execute().expect("execution failed!");

        assert_eq!(
            U256::from(vm.instruction),
            U256::from_str("0xff").expect("failed to parse hex")
        );

        let mut vm = new_test_vm("0x600060fe5758");
        vm.execute().expect("execution failed!");

        assert_eq!(
            U256::from(vm.instruction),
            U256::from_str("0x07").expect("failed to parse hex")
        );

        // PC test
        assert_eq!(vm.stack.peek(0).value, U256::from_str("0x07").expect("failed to parse hex"));
    }

    #[test]
    fn test_usdt_sim() {
        // this execution should return the name of the USDT contract
        let mut vm = new_test_vm("608060405234801561001057600080fd5b50600436106101b95760003560e01c80636a627842116100f9578063ba9a7a5611610097578063d21220a711610071578063d21220a7146105da578063d505accf146105e2578063dd62ed3e14610640578063fff6cae91461067b576101b9565b8063ba9a7a5614610597578063bc25cf771461059f578063c45a0155146105d2576101b9565b80637ecebe00116100d35780637ecebe00146104d757806389afcb441461050a57806395d89b4114610556578063a9059cbb1461055e576101b9565b80636a6278421461046957806370a082311461049c5780637464fc3d146104cf576101b9565b806323b872dd116101665780633644e515116101405780633644e51514610416578063485cc9551461041e5780635909c0d5146104595780635a3d549314610461576101b9565b806323b872dd146103ad57806330adf81f146103f0578063313ce567146103f8576101b9565b8063095ea7b311610197578063095ea7b3146103155780630dfe16811461036257806318160ddd14610393576101b9565b8063022c0d9f146101be57806306fdde03146102595780630902f1ac146102d6575b600080fd5b610257600480360360808110156101d457600080fd5b81359160208101359173ffffffffffffffffffffffffffffffffffffffff604083013516919081019060808101606082013564010000000081111561021857600080fd5b82018360208201111561022a57600080fd5b8035906020019184600183028401116401000000008311171561024c57600080fd5b509092509050610683565b005b610261610d57565b6040805160208082528351818301528351919283929083019185019080838360005b8381101561029b578181015183820152602001610283565b50505050905090810190601f1680156102c85780820380516001836020036101000a031916815260200191505b509250505060405180910390f35b6102de610d90565b604080516dffffffffffffffffffffffffffff948516815292909316602083015263ffffffff168183015290519081900360600190f35b61034e6004803603604081101561032b57600080fd5b5073ffffffffffffffffffffffffffffffffffffffff8135169060200135610de5565b604080519115158252519081900360200190f35b61036a610dfc565b6040805173ffffffffffffffffffffffffffffffffffffffff9092168252519081900360200190f35b61039b610e18565b60408051918252519081900360200190f35b61034e600480360360608110156103c357600080fd5b5073ffffffffffffffffffffffffffffffffffffffff813581169160208101359091169060400135610e1e565b61039b610efd565b610400610f21565b6040805160ff9092168252519081900360200190f35b61039b610f26565b6102576004803603604081101561043457600080fd5b5073ffffffffffffffffffffffffffffffffffffffff81358116916020013516610f2c565b61039b611005565b61039b61100b565b61039b6004803603602081101561047f57600080fd5b503573ffffffffffffffffffffffffffffffffffffffff16611011565b61039b600480360360208110156104b257600080fd5b503573ffffffffffffffffffffffffffffffffffffffff166113cb565b61039b6113dd565b61039b600480360360208110156104ed57600080fd5b503573ffffffffffffffffffffffffffffffffffffffff166113e3565b61053d6004803603602081101561052057600080fd5b503573ffffffffffffffffffffffffffffffffffffffff166113f5565b6040805192835260208301919091528051918290030190f35b610261611892565b61034e6004803603604081101561057457600080fd5b5073ffffffffffffffffffffffffffffffffffffffff81351690602001356118cb565b61039b6118d8565b610257600480360360208110156105b557600080fd5b503573ffffffffffffffffffffffffffffffffffffffff166118de565b61036a611ad4565b61036a611af0565b610257600480360360e08110156105f857600080fd5b5073ffffffffffffffffffffffffffffffffffffffff813581169160208101359091169060408101359060608101359060ff6080820135169060a08101359060c00135611b0c565b61039b6004803603604081101561065657600080fd5b5073ffffffffffffffffffffffffffffffffffffffff81358116916020013516611dd8565b610257611df5565b600c546001146106f457604080517f08c379a000000000000000000000000000000000000000000000000000000000815260206004820152601160248201527f556e697377617056323a204c4f434b4544000000000000000000000000000000604482015290519081900360640190fd5b6000600c55841515806107075750600084115b61075c576040517f08c379a0000000000000000000000000000000000000000000000000000000008152600401808060200182810382526025815260200180612b2f6025913960400191505060405180910390fd5b600080610767610d90565b5091509150816dffffffffffffffffffffffffffff168710801561079a5750806dffffffffffffffffffffffffffff1686105b6107ef576040517f08c379a0000000000000000000000000000000000000000000000000000000008152600401808060200182810382526021815260200180612b786021913960400191505060405180910390fd5b600654600754600091829173ffffffffffffffffffffffffffffffffffffffff91821691908116908916821480159061085457508073ffffffffffffffffffffffffffffffffffffffff168973ffffffffffffffffffffffffffffffffffffffff1614155b6108bf57604080517f08c379a000000000000000000000000000000000000000000000000000000000815260206004820152601560248201527f556e697377617056323a20494e56414c49445f544f0000000000000000000000604482015290519081900360640190fd5b8a156108d0576108d0828a8d611fdb565b89156108e1576108e1818a8c611fdb565b86156109c3578873ffffffffffffffffffffffffffffffffffffffff166310d1e85c338d8d8c8c6040518663ffffffff1660e01b8152600401808673ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff168152602001858152602001848152602001806020018281038252848482818152602001925080828437600081840152601f19601f8201169050808301925050509650505050505050600060405180830381600087803b1580156109aa57600080fd5b505af11580156109be573d6000803e3d6000fd5b505050505b604080517f70a08231000000000000000000000000000000000000000000000000000000008152306004820152905173ffffffffffffffffffffffffffffffffffffffff8416916370a08231916024808301926020929190829003018186803b158015610a2f57600080fd5b505afa158015610a43573d6000803e3d6000fd5b505050506040513d6020811015610a5957600080fd5b5051604080517f70a08231000000000000000000000000000000000000000000000000000000008152306004820152905191955073ffffffffffffffffffffffffffffffffffffffff8316916370a0823191602480820192602092909190829003018186803b158015610acb57600080fd5b505afa158015610adf573d6000803e3d6000fd5b505050506040513d6020811015610af557600080fd5b5051925060009150506dffffffffffffffffffffffffffff85168a90038311610b1f576000610b35565b89856dffffffffffffffffffffffffffff160383035b9050600089856dffffffffffffffffffffffffffff16038311610b59576000610b6f565b89856dffffffffffffffffffffffffffff160383035b90506000821180610b805750600081115b610bd5576040517f08c379a0000000000000000000000000000000000000000000000000000000008152600401808060200182810382526024815260200180612b546024913960400191505060405180910390fd5b6000610c09610beb84600363ffffffff6121e816565b610bfd876103e863ffffffff6121e816565b9063ffffffff61226e16565b90506000610c21610beb84600363ffffffff6121e816565b9050610c59620f4240610c4d6dffffffffffffffffffffffffffff8b8116908b1663ffffffff6121e816565b9063ffffffff6121e816565b610c69838363ffffffff6121e816565b1015610cd657604080517f08c379a000000000000000000000000000000000000000000000000000000000815260206004820152600c60248201527f556e697377617056323a204b0000000000000000000000000000000000000000604482015290519081900360640190fd5b5050610ce4848488886122e0565b60408051838152602081018390528082018d9052606081018c9052905173ffffffffffffffffffffffffffffffffffffffff8b169133917fd78ad95fa46c994b6551d0da85fc275fe613ce37657fb8d5e3d130840159d8229181900360800190a350506001600c55505050505050505050565b6040518060400160405280600a81526020017f556e69737761702056320000000000000000000000000000000000000000000081525081565b6008546dffffffffffffffffffffffffffff808216926e0100000000000000000000000000008304909116917c0100000000000000000000000000000000000000000000000000000000900463ffffffff1690565b6000610df233848461259c565b5060015b92915050565b60065473ffffffffffffffffffffffffffffffffffffffff1681565b60005481565b73ffffffffffffffffffffffffffffffffffffffff831660009081526002602090815260408083203384529091528120547fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff14610ee85773ffffffffffffffffffffffffffffffffffffffff84166000908152600260209081526040808320338452909152902054610eb6908363ffffffff61226e16565b73ffffffffffffffffffffffffffffffffffffffff851660009081526002602090815260408083203384529091529020555b610ef384848461260b565b5060019392505050565b7f6e71edae12b1b97f4d1f60370fef10105fa2faae0126114a169c64845d6126c981565b601281565b60035481565b60055473ffffffffffffffffffffffffffffffffffffffff163314610fb257604080517f08c379a000000000000000000000000000000000000000000000000000000000815260206004820152601460248201527f556e697377617056323a20464f5242494444454e000000000000000000000000604482015290519081900360640190fd5b6006805473ffffffffffffffffffffffffffffffffffffffff9384167fffffffffffffffffffffffff00000000000000000000000000000000000000009182161790915560078054929093169116179055565b60095481565b600a5481565b6000600c5460011461108457604080517f08c379a000000000000000000000000000000000000000000000000000000000815260206004820152601160248201527f556e697377617056323a204c4f434b4544000000000000000000000000000000604482015290519081900360640190fd5b6000600c81905580611094610d90565b50600654604080517f70a08231000000000000000000000000000000000000000000000000000000008152306004820152905193955091935060009273ffffffffffffffffffffffffffffffffffffffff909116916370a08231916024808301926020929190829003018186803b15801561110e57600080fd5b505afa158015611122573d6000803e3d6000fd5b505050506040513d602081101561113857600080fd5b5051600754604080517f70a08231000000000000000000000000000000000000000000000000000000008152306004820152905192935060009273ffffffffffffffffffffffffffffffffffffffff909216916370a0823191602480820192602092909190829003018186803b1580156111b157600080fd5b505afa1580156111c5573d6000803e3d6000fd5b505050506040513d60208110156111db57600080fd5b505190506000611201836dffffffffffffffffffffffffffff871663ffffffff61226e16565b90506000611225836dffffffffffffffffffffffffffff871663ffffffff61226e16565b9050600061123387876126ec565b600054909150806112705761125c6103e8610bfd611257878763ffffffff6121e816565b612878565b985061126b60006103e86128ca565b6112cd565b6112ca6dffffffffffffffffffffffffffff8916611294868463ffffffff6121e816565b8161129b57fe5b046dffffffffffffffffffffffffffff89166112bd868563ffffffff6121e816565b816112c457fe5b0461297a565b98505b60008911611326576040517f08c379a0000000000000000000000000000000000000000000000000000000008152600401808060200182810382526028815260200180612bc16028913960400191505060405180910390fd5b6113308a8a6128ca565b61133c86868a8a6122e0565b811561137e5760085461137a906dffffffffffffffffffffffffffff808216916e01000000000000000000000000000090041663ffffffff6121e816565b600b555b6040805185815260208101859052815133927f4c209b5fc8ad50758f13e2e1088ba56a560dff690a1c6fef26394f4c03821c4f928290030190a250506001600c5550949695505050505050565b60016020526000908152604090205481565b600b5481565b60046020526000908152604090205481565b600080600c5460011461146957604080517f08c379a000000000000000000000000000000000000000000000000000000000815260206004820152601160248201527f556e697377617056323a204c4f434b4544000000000000000000000000000000604482015290519081900360640190fd5b6000600c81905580611479610d90565b50600654600754604080517f70a08231000000000000000000000000000000000000000000000000000000008152306004820152905194965092945073ffffffffffffffffffffffffffffffffffffffff9182169391169160009184916370a08231916024808301926020929190829003018186803b1580156114fb57600080fd5b505afa15801561150f573d6000803e3d6000fd5b505050506040513d602081101561152557600080fd5b5051604080517f70a08231000000000000000000000000000000000000000000000000000000008152306004820152905191925060009173ffffffffffffffffffffffffffffffffffffffff8516916370a08231916024808301926020929190829003018186803b15801561159957600080fd5b505afa1580156115ad573d6000803e3d6000fd5b505050506040513d60208110156115c357600080fd5b5051306000908152600160205260408120549192506115e288886126ec565b600054909150806115f9848763ffffffff6121e816565b8161160057fe5b049a5080611614848663ffffffff6121e816565b8161161b57fe5b04995060008b11801561162e575060008a115b611683576040517f08c379a0000000000000000000000000000000000000000000000000000000008152600401808060200182810382526028815260200180612b996028913960400191505060405180910390fd5b61168d3084612992565b611698878d8d611fdb565b6116a3868d8c611fdb565b604080517f70a08231000000000000000000000000000000000000000000000000000000008152306004820152905173ffffffffffffffffffffffffffffffffffffffff8916916370a08231916024808301926020929190829003018186803b15801561170f57600080fd5b505afa158015611723573d6000803e3d6000fd5b505050506040513d602081101561173957600080fd5b5051604080517f70a08231000000000000000000000000000000000000000000000000000000008152306004820152905191965073ffffffffffffffffffffffffffffffffffffffff8816916370a0823191602480820192602092909190829003018186803b1580156117ab57600080fd5b505afa1580156117bf573d6000803e3d6000fd5b505050506040513d60208110156117d557600080fd5b505193506117e585858b8b6122e0565b811561182757600854611823906dffffffffffffffffffffffffffff808216916e01000000000000000000000000000090041663ffffffff6121e816565b600b555b604080518c8152602081018c9052815173ffffffffffffffffffffffffffffffffffffffff8f169233927fdccd412f0b1252819cb1fd330b93224ca42612892bb3f4f789976e6d81936496929081900390910190a35050505050505050506001600c81905550915091565b6040518060400160405280600681526020017f554e492d5632000000000000000000000000000000000000000000000000000081525081565b6000610df233848461260b565b6103e881565b600c5460011461194f57604080517f08c379a000000000000000000000000000000000000000000000000000000000815260206004820152601160248201527f556e697377617056323a204c4f434b4544000000000000000000000000000000604482015290519081900360640190fd5b6000600c55600654600754600854604080517f70a08231000000000000000000000000000000000000000000000000000000008152306004820152905173ffffffffffffffffffffffffffffffffffffffff9485169490931692611a2b9285928792611a26926dffffffffffffffffffffffffffff169185916370a0823191602480820192602092909190829003018186803b1580156119ee57600080fd5b505afa158015611a02573d6000803e3d6000fd5b505050506040513d6020811015611a1857600080fd5b50519063ffffffff61226e16565b611fdb565b600854604080517f70a082310000000000000000000000000000000000000000000000000000000081523060048201529051611aca9284928792611a26926e01000000000000000000000000000090046dffffffffffffffffffffffffffff169173ffffffffffffffffffffffffffffffffffffffff8616916370a0823191602480820192602092909190829003018186803b1580156119ee57600080fd5b50506001600c5550565b60055473ffffffffffffffffffffffffffffffffffffffff1681565b60075473ffffffffffffffffffffffffffffffffffffffff1681565b42841015611b7b57604080517f08c379a000000000000000000000000000000000000000000000000000000000815260206004820152601260248201527f556e697377617056323a20455850495245440000000000000000000000000000604482015290519081900360640190fd5b60035473ffffffffffffffffffffffffffffffffffffffff80891660008181526004602090815260408083208054600180820190925582517f6e71edae12b1b97f4d1f60370fef10105fa2faae0126114a169c64845d6126c98186015280840196909652958d166060860152608085018c905260a085019590955260c08085018b90528151808603909101815260e0850182528051908301207f19010000000000000000000000000000000000000000000000000000000000006101008601526101028501969096526101228085019690965280518085039096018652610142840180825286519683019690962095839052610162840180825286905260ff89166101828501526101a284018890526101c28401879052519193926101e2808201937fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe081019281900390910190855afa158015611cdc573d6000803e3d6000fd5b50506040517fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe0015191505073ffffffffffffffffffffffffffffffffffffffff811615801590611d5757508873ffffffffffffffffffffffffffffffffffffffff168173ffffffffffffffffffffffffffffffffffffffff16145b611dc257604080517f08c379a000000000000000000000000000000000000000000000000000000000815260206004820152601c60248201527f556e697377617056323a20494e56414c49445f5349474e415455524500000000604482015290519081900360640190fd5b611dcd89898961259c565b505050505050505050565b600260209081526000928352604080842090915290825290205481565b600c54600114611e6657604080517f08c379a000000000000000000000000000000000000000000000000000000000815260206004820152601160248201527f556e697377617056323a204c4f434b4544000000000000000000000000000000604482015290519081900360640190fd5b6000600c55600654604080517f70a082310000000000000000000000000000000000000000000000000000000081523060048201529051611fd49273ffffffffffffffffffffffffffffffffffffffff16916370a08231916024808301926020929190829003018186803b158015611edd57600080fd5b505afa158015611ef1573d6000803e3d6000fd5b505050506040513d6020811015611f0757600080fd5b5051600754604080517f70a08231000000000000000000000000000000000000000000000000000000008152306004820152905173ffffffffffffffffffffffffffffffffffffffff909216916370a0823191602480820192602092909190829003018186803b158015611f7a57600080fd5b505afa158015611f8e573d6000803e3d6000fd5b505050506040513d6020811015611fa457600080fd5b50516008546dffffffffffffffffffffffffffff808216916e0100000000000000000000000000009004166122e0565b6001600c55565b604080518082018252601981527f7472616e7366657228616464726573732c75696e743235362900000000000000602091820152815173ffffffffffffffffffffffffffffffffffffffff85811660248301526044808301869052845180840390910181526064909201845291810180517bffffffffffffffffffffffffffffffffffffffffffffffffffffffff167fa9059cbb000000000000000000000000000000000000000000000000000000001781529251815160009460609489169392918291908083835b602083106120e157805182527fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe090920191602091820191016120a4565b6001836020036101000a0380198251168184511680821785525050505050509050019150506000604051808303816000865af19150503d8060008114612143576040519150601f19603f3d011682016040523d82523d6000602084013e612148565b606091505b5091509150818015612176575080511580612176575080806020019051602081101561217357600080fd5b50515b6121e157604080517f08c379a000000000000000000000000000000000000000000000000000000000815260206004820152601a60248201527f556e697377617056323a205452414e534645525f4641494c4544000000000000604482015290519081900360640190fd5b5050505050565b60008115806122035750508082028282828161220057fe5b04145b610df657604080517f08c379a000000000000000000000000000000000000000000000000000000000815260206004820152601460248201527f64732d6d6174682d6d756c2d6f766572666c6f77000000000000000000000000604482015290519081900360640190fd5b80820382811115610df657604080517f08c379a000000000000000000000000000000000000000000000000000000000815260206004820152601560248201527f64732d6d6174682d7375622d756e646572666c6f770000000000000000000000604482015290519081900360640190fd5b6dffffffffffffffffffffffffffff841180159061230c57506dffffffffffffffffffffffffffff8311155b61237757604080517f08c379a000000000000000000000000000000000000000000000000000000000815260206004820152601360248201527f556e697377617056323a204f564552464c4f5700000000000000000000000000604482015290519081900360640190fd5b60085463ffffffff428116917c0100000000000000000000000000000000000000000000000000000000900481168203908116158015906123c757506dffffffffffffffffffffffffffff841615155b80156123e257506dffffffffffffffffffffffffffff831615155b15612492578063ffffffff16612425856123fb86612a57565b7bffffffffffffffffffffffffffffffffffffffffffffffffffffffff169063ffffffff612a7b16565b600980547bffffffffffffffffffffffffffffffffffffffffffffffffffffffff929092169290920201905563ffffffff8116612465846123fb87612a57565b600a80547bffffffffffffffffffffffffffffffffffffffffffffffffffffffff92909216929092020190555b600880547fffffffffffffffffffffffffffffffffffff0000000000000000000000000000166dffffffffffffffffffffffffffff888116919091177fffffffff0000000000000000000000000000ffffffffffffffffffffffffffff166e0100000000000000000000000000008883168102919091177bffffffffffffffffffffffffffffffffffffffffffffffffffffffff167c010000000000000000000000000000000000000000000000000000000063ffffffff871602179283905560408051848416815291909304909116602082015281517f1c411e9a96e071241c2f21f7726b17ae89e3cab4c78be50e062b03a9fffbbad1929181900390910190a1505050505050565b73ffffffffffffffffffffffffffffffffffffffff808416600081815260026020908152604080832094871680845294825291829020859055815185815291517f8c5be1e5ebec7d5bd14f71427d1e84f3dd0314c0f7b2291e5b200ac8c7c3b9259281900390910190a3505050565b73ffffffffffffffffffffffffffffffffffffffff8316600090815260016020526040902054612641908263ffffffff61226e16565b73ffffffffffffffffffffffffffffffffffffffff8085166000908152600160205260408082209390935590841681522054612683908263ffffffff612abc16565b73ffffffffffffffffffffffffffffffffffffffff80841660008181526001602090815260409182902094909455805185815290519193928716927fddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef92918290030190a3505050565b600080600560009054906101000a900473ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff1663017e7e586040518163ffffffff1660e01b815260040160206040518083038186803b15801561275757600080fd5b505afa15801561276b573d6000803e3d6000fd5b505050506040513d602081101561278157600080fd5b5051600b5473ffffffffffffffffffffffffffffffffffffffff821615801594509192509061286457801561285f5760006127d86112576dffffffffffffffffffffffffffff88811690881663ffffffff6121e816565b905060006127e583612878565b90508082111561285c576000612813612804848463ffffffff61226e16565b6000549063ffffffff6121e816565b905060006128388361282c86600563ffffffff6121e816565b9063ffffffff612abc16565b9050600081838161284557fe5b04905080156128585761285887826128ca565b5050505b50505b612870565b8015612870576000600b555b505092915050565b600060038211156128bb575080600160028204015b818110156128b5578091506002818285816128a457fe5b0401816128ad57fe5b04905061288d565b506128c5565b81156128c5575060015b919050565b6000546128dd908263ffffffff612abc16565b600090815573ffffffffffffffffffffffffffffffffffffffff8316815260016020526040902054612915908263ffffffff612abc16565b73ffffffffffffffffffffffffffffffffffffffff831660008181526001602090815260408083209490945583518581529351929391927fddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef9281900390910190a35050565b6000818310612989578161298b565b825b9392505050565b73ffffffffffffffffffffffffffffffffffffffff82166000908152600160205260409020546129c8908263ffffffff61226e16565b73ffffffffffffffffffffffffffffffffffffffff831660009081526001602052604081209190915554612a02908263ffffffff61226e16565b600090815560408051838152905173ffffffffffffffffffffffffffffffffffffffff8516917fddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef919081900360200190a35050565b6dffffffffffffffffffffffffffff166e0100000000000000000000000000000290565b60006dffffffffffffffffffffffffffff82167bffffffffffffffffffffffffffffffffffffffffffffffffffffffff841681612ab457fe5b049392505050565b80820182811015610df657604080517f08c379a000000000000000000000000000000000000000000000000000000000815260206004820152601460248201527f64732d6d6174682d6164642d6f766572666c6f77000000000000000000000000604482015290519081900360640190fdfe556e697377617056323a20494e53554646494349454e545f4f55545055545f414d4f554e54556e697377617056323a20494e53554646494349454e545f494e5055545f414d4f554e54556e697377617056323a20494e53554646494349454e545f4c4951554944495459556e697377617056323a20494e53554646494349454e545f4c49515549444954595f4255524e4544556e697377617056323a20494e53554646494349454e545f4c49515549444954595f4d494e544544a265627a7a723158207dca18479e58487606bf70c79e44d8dee62353c9ee6d01f9a9d70885b8765f2264736f6c63430005100032");
        vm.calldata = [0x06, 0xfd, 0xde, 0x03].to_vec();
        vm.execute().expect("execution failed!");

        assert_eq!(
            vm.returndata,
            vec![
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 32, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0, 10, 85, 110, 105, 115, 119, 97, 112, 32, 86, 50, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0
            ]
        );
    }
}
