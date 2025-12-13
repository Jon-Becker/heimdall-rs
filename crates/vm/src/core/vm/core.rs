use hashbrown::HashSet;
use std::sync::Arc;

use alloy::primitives::{Address, I256, U256};
use eyre::{OptionExt, Result};

#[cfg(feature = "step-tracing")]
use std::time::Instant;
#[cfg(feature = "step-tracing")]
use tracing::trace;

use crate::core::opcodes::{self, OpCodeInfo, WrappedInput, WrappedOpcode};

use super::super::{
    log::Log,
    memory::Memory,
    stack::{Stack, StackFrame},
    storage::Storage,
};

use super::{
    execution::{ExecutionResult, Instruction, State},
    handlers,
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

    /// Push a boolean value onto the stack
    pub(crate) fn push_boolean(&mut self, condition: bool, operation: WrappedOpcode) {
        let value = if condition { U256::from(1u8) } else { U256::ZERO };
        self.stack.push(value, operation);
    }

    /// Convert an address to U256
    pub(crate) fn address_to_u256(address: &Address) -> U256 {
        let mut result = [0u8; 32];
        result[12..].copy_from_slice(address.as_ref());
        U256::from_be_bytes(result)
    }

    /// Push with optimization for two operands
    pub(crate) fn push_with_optimization(
        &mut self,
        result: U256,
        a: &StackFrame,
        b: &StackFrame,
        operation: WrappedOpcode,
    ) {
        let simplified_operation = if (opcodes::PUSH0..=opcodes::PUSH32)
            .contains(&a.operation.opcode) &&
            (opcodes::PUSH0..=opcodes::PUSH32).contains(&b.operation.opcode)
        {
            WrappedOpcode::new(opcodes::PUSH32, vec![WrappedInput::Raw(result)])
        } else {
            operation
        };
        self.stack.push(result, simplified_operation);
    }

    /// Push with optimization for single operand
    pub(crate) fn push_with_optimization_single(
        &mut self,
        result: U256,
        a: &StackFrame,
        operation: WrappedOpcode,
    ) {
        let simplified_operation =
            if (opcodes::PUSH0..=opcodes::PUSH32).contains(&a.operation.opcode) {
                WrappedOpcode::new(opcodes::PUSH32, vec![WrappedInput::Raw(result)])
            } else {
                operation
            };
        self.stack.push(result, simplified_operation);
    }

    /// Push with optimization for signed operations
    pub(crate) fn push_with_optimization_signed(
        &mut self,
        result: I256,
        a: &StackFrame,
        b: &StackFrame,
        operation: WrappedOpcode,
    ) {
        let simplified_operation = if (opcodes::PUSH0..=opcodes::PUSH32)
            .contains(&a.operation.opcode) &&
            (opcodes::PUSH0..=opcodes::PUSH32).contains(&b.operation.opcode)
        {
            WrappedOpcode::new(opcodes::PUSH32, vec![WrappedInput::Raw(result.into_raw())])
        } else {
            operation
        };
        self.stack.push(result.into_raw(), simplified_operation);
    }

    /// Safely copy data from source with bounds checking
    pub(crate) fn safe_copy_data(source: &[u8], offset: usize, size: usize) -> Vec<u8> {
        let end_offset = offset.saturating_add(size).min(source.len());
        let mut value = source.get(offset..end_offset).unwrap_or(&[]).to_owned();
        if value.len() < size {
            value.resize(size, 0u8);
        }
        value
    }

    /// Executes the next instruction in the bytecode. Returns information about the instruction
    /// executed.
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
            .map(|x| WrappedInput::Opcode(Arc::new(x.to_owned())))
            .collect::<Vec<WrappedInput>>();
        let operation = WrappedOpcode::new(opcode, wrapped_inputs);

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
            opcodes::STOP => {
                return Ok(handlers::control::stop(
                    self,
                    last_instruction,
                    &inputs,
                    &input_operations,
                ));
            }

            opcodes::ADD => handlers::arithmetic::add(self, operation)?,
            opcodes::MUL => handlers::arithmetic::mul(self, operation)?,
            opcodes::SUB => handlers::arithmetic::sub(self, operation)?,
            opcodes::DIV => handlers::arithmetic::div(self, operation)?,
            opcodes::SDIV => handlers::arithmetic::sdiv(self, operation)?,
            opcodes::MOD => handlers::arithmetic::modulo(self, operation)?,
            opcodes::SMOD => handlers::arithmetic::smod(self, operation)?,
            opcodes::ADDMOD => handlers::arithmetic::addmod(self, operation)?,
            opcodes::MULMOD => handlers::arithmetic::mulmod(self, operation)?,
            opcodes::EXP => handlers::arithmetic::exp(self, operation)?,
            opcodes::SIGNEXTEND => handlers::arithmetic::signextend(self, operation)?,

            opcodes::LT => handlers::comparison::lt(self, operation)?,
            opcodes::GT => handlers::comparison::gt(self, operation)?,
            opcodes::SLT => handlers::comparison::slt(self, operation)?,
            opcodes::SGT => handlers::comparison::sgt(self, operation)?,
            opcodes::EQ => handlers::comparison::eq(self, operation)?,
            opcodes::ISZERO => handlers::comparison::iszero(self, operation)?,

            opcodes::AND => handlers::bitwise::and(self, operation)?,
            opcodes::OR => handlers::bitwise::or(self, operation)?,
            opcodes::XOR => handlers::bitwise::xor(self, operation)?,
            opcodes::NOT => handlers::bitwise::not(self, operation)?,
            opcodes::BYTE => handlers::bitwise::byte(self, operation)?,
            opcodes::SHL => handlers::bitwise::shl(self, operation)?,
            opcodes::SHR => handlers::bitwise::shr(self, operation)?,
            opcodes::SAR => handlers::bitwise::sar(self, operation)?,

            opcodes::SHA3 => handlers::crypto::sha3(self, operation)?,

            opcodes::ADDRESS => handlers::environment::address(self, operation)?,
            opcodes::BALANCE => handlers::environment::balance(self, operation)?,
            opcodes::ORIGIN => handlers::environment::origin(self, operation)?,
            opcodes::CALLER => handlers::environment::caller(self, operation)?,
            opcodes::CALLVALUE => handlers::environment::callvalue(self, operation)?,
            opcodes::CALLDATALOAD => handlers::environment::calldataload(self, operation)?,
            opcodes::CALLDATASIZE => handlers::environment::calldatasize(self, operation)?,
            opcodes::CALLDATACOPY => handlers::environment::calldatacopy(
                self,
                #[cfg(feature = "experimental")]
                operation,
            )?,
            opcodes::CODESIZE => handlers::environment::codesize(self, operation)?,
            opcodes::CODECOPY => handlers::environment::codecopy(
                self,
                #[cfg(feature = "experimental")]
                operation,
            )?,
            opcodes::GASPRICE => handlers::environment::gasprice(self, operation)?,
            opcodes::EXTCODESIZE => handlers::environment::extcodesize(self, operation)?,
            opcodes::EXTCODECOPY => handlers::environment::extcodecopy(
                self,
                #[cfg(feature = "experimental")]
                operation,
            )?,
            opcodes::RETURNDATASIZE => handlers::environment::returndatasize(self, operation)?,
            opcodes::RETURNDATACOPY => handlers::environment::returndatacopy(
                self,
                #[cfg(feature = "experimental")]
                operation,
            )?,
            opcodes::EXTCODEHASH => handlers::environment::extcodehash(self, operation)?,
            opcodes::BLOCKHASH => handlers::environment::blockhash(self, operation)?,

            opcodes::COINBASE => handlers::block::coinbase(self, operation)?,
            opcodes::TIMESTAMP => handlers::block::timestamp(self, operation)?,
            (opcodes::NUMBER..=opcodes::BLOBBASEFEE) => {
                handlers::block::block_info_stub(self, operation)?
            }

            opcodes::POP => handlers::stack::pop(self)?,
            opcodes::MLOAD => handlers::memory::mload(self, operation)?,
            opcodes::MSTORE => handlers::memory::mstore(
                self,
                #[cfg(feature = "experimental")]
                operation,
            )?,
            opcodes::MSTORE8 => handlers::memory::mstore8(
                self,
                #[cfg(feature = "experimental")]
                operation,
            )?,
            opcodes::SLOAD => handlers::storage::sload(self, operation)?,
            opcodes::SSTORE => handlers::storage::sstore(self)?,

            opcodes::JUMP => {
                if let Some(instruction) =
                    handlers::control::jump(self, last_instruction, &inputs, &input_operations)
                {
                    return Ok(instruction);
                }
            }
            opcodes::JUMPI => {
                if let Some(instruction) =
                    handlers::control::jumpi(self, last_instruction, &inputs, &input_operations)
                {
                    return Ok(instruction);
                }
            }
            opcodes::JUMPDEST => handlers::control::jumpdest()?,
            opcodes::TLOAD => handlers::storage::tload(self, operation)?,
            opcodes::TSTORE => handlers::storage::tstore(self)?,
            opcodes::MCOPY => handlers::memory::mcopy(
                self,
                #[cfg(feature = "experimental")]
                operation,
            )?,
            opcodes::PC => handlers::control::pc(self, operation)?,
            opcodes::MSIZE => handlers::memory::msize(self, operation)?,
            opcodes::GAS => handlers::control::gas(self, operation)?,

            opcodes::PUSH0 => handlers::stack::push0(self, operation)?,
            (opcodes::PUSH1..=opcodes::PUSH32) => handlers::stack::push_n(self, opcode, operation)?,
            (opcodes::DUP1..=opcodes::DUP16) => handlers::stack::dup_n(self, opcode)?,
            (opcodes::SWAP1..=opcodes::SWAP16) => handlers::stack::swap_n(self, opcode)?,

            (opcodes::LOG0..=opcodes::LOG4) => {
                let topic_count = opcode - 160;
                handlers::logging::log_n(self, topic_count)?;
            }

            opcodes::CREATE => handlers::system::create(self, operation)?,
            opcodes::CALL => handlers::system::call(self, operation)?,
            opcodes::CALLCODE => handlers::system::callcode(self, operation)?,
            opcodes::RETURN => handlers::system::op_return(self)?,
            opcodes::DELEGATECALL => handlers::system::delegatecall(self, operation)?,
            opcodes::CREATE2 => handlers::system::create2(self, operation)?,
            opcodes::STATICCALL => handlers::system::staticcall(self, operation)?,
            opcodes::REVERT => handlers::system::revert(self)?,

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
