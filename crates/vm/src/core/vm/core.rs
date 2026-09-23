use hashbrown::HashSet;
use std::sync::Arc;

use alloy::primitives::{Address, I256, U256};
use eyre::{OptionExt, Result};

#[cfg(feature = "step-tracing")]
use std::time::Instant;
#[cfg(feature = "step-tracing")]
use tracing::trace;

use crate::core::{
    hardfork::HardFork,
    opcodes::{self, OpCodeInfo, WrappedInput, WrappedOpcode},
};

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

    /// The hard fork to use for opcode activation.
    pub hardfork: HardFork,

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
            hardfork: HardFork::default(),
            #[cfg(feature = "step-tracing")]
            operation_count: 0,
            #[cfg(feature = "step-tracing")]
            start_time: Instant::now(),
        }
    }

    /// Sets the hard fork for opcode activation.
    pub fn with_hardfork(mut self, hardfork: HardFork) -> Self {
        self.hardfork = hardfork;
        self
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
        let opcode_info = match OpCodeInfo::for_fork(opcode, self.hardfork) {
            Some(info) => info,
            None => {
                // Opcode not active at this hardfork - treat as invalid
                self.exit(1, Vec::new());
                return Ok(Instruction {
                    instruction: last_instruction,
                    opcode,
                    inputs: Vec::new(),
                    outputs: Vec::new(),
                    input_operations: Vec::new(),
                    output_operations: Vec::new(),
                });
            }
        };
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
            opcodes::CLZ => handlers::bitwise::clz(self, operation)?,

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

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use alloy::primitives::{Address, U256};
    use heimdall_common::utils::strings::decode_hex;

    use super::VM;
    use crate::core::hardfork::HardFork;

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
        let mut vm = new_test_vm(include_str!("../../../../core/tests/testdata/vm/sdiv.hex"));
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
        let mut vm = new_test_vm(include_str!("../../../../core/tests/testdata/vm/smod.hex"));
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
        let mut vm = new_test_vm(include_str!("../../../../core/tests/testdata/vm/mulmod.hex"));
        vm.execute().expect("execution failed!");

        assert_eq!(vm.stack.peek(1).value, U256::from_str("0x04").expect("failed to parse hex"));
        assert_eq!(vm.stack.peek(0).value, U256::from_str("0x09").expect("failed to parse hex"));
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
        let mut vm = new_test_vm(
            "0x7f000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f6020526020602060005e",
        );
        vm.execute().expect("execution failed!");
        assert_eq!(
            vm.memory.read(0, 64),
            decode_hex(include_str!("../../../../core/tests/testdata/vm/mcopy.hex"))
                .expect("failed to parse hex")
        );
    }

    #[test]
    fn test_mcopy_clamping_source_beyond_memory() {
        // Test copying from offset beyond current memory size
        // Store 32 bytes at memory[0x20], then try to copy from offset 0x40 (beyond memory)
        let mut vm = new_test_vm(
            "0x7f000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f6020526020604060005e",
        );
        vm.execute().expect("execution failed!");

        // Should copy zeros since source is beyond memory
        let result = vm.memory.read(0, 32);
        assert_eq!(result, vec![0u8; 32]);
    }

    #[test]
    fn test_mcopy_clamping_partial_source_beyond_memory() {
        // Test copying where source starts in memory but extends beyond it
        // Store 32 bytes at memory[0x20], then copy 64 bytes from offset 0x30
        let mut vm = new_test_vm(
            "0x7f000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f6020526040603060005e",
        );
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
        let mut vm = new_test_vm(
            "0x7f000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f6020526000602060005e",
        );
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
        let mut vm = new_test_vm(
            "0x7f000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f6020526040602060005e",
        );
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
        let mut vm = new_test_vm(
            "0x7f000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f6000526010601060005e",
        );
        vm.execute().expect("execution failed!");

        // Original data at 0x00: 000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f
        // Copy 16 bytes from 0x10 to 0x00: should copy 101112131415161718191a1b1c1d1e1f
        let result = vm.memory.read(0, 32);
        let expected = [
            &decode_hex("101112131415161718191a1b1c1d1e1f").expect("failed to parse hex")[..], // 0x00-0x0F: copied data
            &decode_hex("101112131415161718191a1b1c1d1e1f").expect("failed to parse hex")[..], // 0x10-0x1F: original data
        ]
        .concat();
        assert_eq!(result, expected);
    }

    #[test]
    fn test_mcopy_clamping_large_offsets() {
        // Test with very large U256 offsets that get clamped to usize::MAX
        // This tests the try_into().unwrap_or() clamping behavior
        let mut vm = new_test_vm(
            "0x7f000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f6020526020608060005e",
        );
        vm.execute().expect("execution failed!");

        // Should handle large offset gracefully and copy zeros (since source is beyond memory)
        let result = vm.memory.read(0, 32);
        assert_eq!(result, vec![0u8; 32]);
    }

    #[test]
    fn test_mcopy_clamping_exact_memory_boundary() {
        // Test copying exactly at memory boundary
        // Store 32 bytes, then copy from the last valid offset
        let mut vm = new_test_vm(
            "0x7f000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f6020526001603f60005e",
        );
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
        let mut vm = new_test_vm(
            "0x7f00000000000000000000000000000000000000000000000000000000000000FF600052600051600151",
        );
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
        let mut vm = new_test_vm(include_str!("../../../../core/tests/testdata/vm/usdt_sim.hex"));
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

    // Helper to create a test VM with a specific hardfork
    fn new_test_vm_with_fork(bytecode: &str, fork: HardFork) -> VM {
        new_test_vm(bytecode).with_hardfork(fork)
    }

    #[test]
    fn test_vm_push0_active_in_shanghai() {
        // PUSH0 followed by STOP: 0x5f 0x00
        let mut vm = new_test_vm_with_fork("0x5f00", HardFork::Shanghai);
        vm.execute().expect("execution failed!");

        // Should have pushed 0 onto the stack
        assert_eq!(vm.stack.peek(0).value, U256::ZERO);
        assert_eq!(vm.exitcode, 10); // STOP exit code
    }

    #[test]
    fn test_vm_push0_unknown_before_shanghai() {
        // PUSH0 (0x5f) should be treated as unknown before Shanghai
        let mut vm = new_test_vm_with_fork("0x5f00", HardFork::London);
        vm.execute().expect("execution failed!");

        // Should exit with code 1 (invalid opcode)
        assert_eq!(vm.exitcode, 1);
    }

    #[test]
    fn test_vm_shl_active_in_constantinople() {
        // PUSH1 0x02, PUSH1 0x01, SHL, STOP: shift 1 left by 2 bits = 4
        // 0x6002 6001 1b 00
        let mut vm = new_test_vm_with_fork("0x600260011b00", HardFork::Constantinople);
        vm.execute().expect("execution failed!");

        assert_eq!(vm.stack.peek(0).value, U256::from(4));
        assert_eq!(vm.exitcode, 10);
    }

    #[test]
    fn test_vm_shl_unknown_before_constantinople() {
        // SHL (0x1b) should be unknown before Constantinople
        let mut vm = new_test_vm_with_fork("0x600260011b00", HardFork::Byzantium);
        vm.execute().expect("execution failed!");

        // Should exit with code 1 (invalid opcode)
        assert_eq!(vm.exitcode, 1);
    }

    #[test]
    fn test_vm_tload_active_in_cancun() {
        // PUSH1 0x00, TLOAD, STOP: load from transient storage slot 0
        // 0x6000 5c 00
        let mut vm = new_test_vm_with_fork("0x60005c00", HardFork::Cancun);
        vm.execute().expect("execution failed!");

        // Should have loaded 0 from empty transient storage
        assert_eq!(vm.stack.peek(0).value, U256::ZERO);
        assert_eq!(vm.exitcode, 10);
    }

    #[test]
    fn test_vm_tload_unknown_before_cancun() {
        // TLOAD (0x5c) should be unknown before Cancun
        let mut vm = new_test_vm_with_fork("0x60005c00", HardFork::Shanghai);
        vm.execute().expect("execution failed!");

        // Should exit with code 1 (invalid opcode)
        assert_eq!(vm.exitcode, 1);
    }

    #[test]
    fn test_vm_clz_active_in_fusaka() {
        // PUSH1 0x01, CLZ, STOP: count leading zeros of 1 = 255
        // 0x6001 1e 00
        let mut vm = new_test_vm_with_fork("0x60011e00", HardFork::Fusaka);
        vm.execute().expect("execution failed!");

        // CLZ(1) = 255 (255 leading zeros in a 256-bit integer)
        assert_eq!(vm.stack.peek(0).value, U256::from(255));
        assert_eq!(vm.exitcode, 10);
    }

    #[test]
    fn test_vm_clz_zero_input() {
        // PUSH1 0x00, CLZ, STOP: count leading zeros of 0 = 256
        // 0x6000 1e 00
        let mut vm = new_test_vm_with_fork("0x60001e00", HardFork::Fusaka);
        vm.execute().expect("execution failed!");

        // CLZ(0) = 256 (special case per EIP-7939)
        assert_eq!(vm.stack.peek(0).value, U256::from(256));
        assert_eq!(vm.exitcode, 10);
    }

    #[test]
    fn test_vm_clz_high_bit_set() {
        // PUSH32 with high bit set (0x80...00), CLZ, STOP
        // CLZ of a value with the highest bit set = 0
        // 0x7f 8000...00 (32 bytes) 1e 00
        let mut vm = new_test_vm_with_fork(
            include_str!("../../../../core/tests/testdata/vm/clz_high_bit_set.hex"),
            HardFork::Fusaka,
        );
        vm.execute().expect("execution failed!");

        // CLZ(0x80...00) = 0 (no leading zeros)
        assert_eq!(vm.stack.peek(0).value, U256::ZERO);
        assert_eq!(vm.exitcode, 10);
    }

    #[test]
    fn test_vm_clz_unknown_before_fusaka() {
        // CLZ (0x1e) should be unknown before Fusaka
        let mut vm = new_test_vm_with_fork("0x60011e00", HardFork::Pectra);
        vm.execute().expect("execution failed!");

        // Should exit with code 1 (invalid opcode)
        assert_eq!(vm.exitcode, 1);
    }

    #[test]
    fn test_vm_default_hardfork_is_latest() {
        // Default VM should use Latest hardfork and support all opcodes
        let mut vm = new_test_vm("0x5f00"); // PUSH0, STOP
        vm.execute().expect("execution failed!");

        // PUSH0 should work with default (Latest) hardfork
        assert_eq!(vm.stack.peek(0).value, U256::ZERO);
        assert_eq!(vm.exitcode, 10);
    }

    #[test]
    fn test_vm_frontier_opcodes_always_work() {
        // ADD should work at any hardfork
        // PUSH1 0x01, PUSH1 0x02, ADD, STOP
        let mut vm = new_test_vm_with_fork("0x6001600201", HardFork::Frontier);
        vm.execute().expect("execution failed!");

        assert_eq!(vm.stack.peek(0).value, U256::from(3));
    }
}
