use std::{
    ops::{Div, Rem, Shl, Shr},
    str::FromStr,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use ethers::{abi::AbiEncode, prelude::U256, types::I256, utils::keccak256};

use crate::{
    ether::evm::opcodes::{Opcode, WrappedInput, WrappedOpcode},
    utils::strings::{decode_hex, sign_uint},
};

use super::{log::Log, memory::Memory, stack::Stack, storage::Storage};

#[derive(Clone, Debug)]
pub struct VM {
    pub stack: Stack,
    pub memory: Memory,
    pub storage: Storage,
    pub instruction: u128,
    pub bytecode: Vec<u8>,
    pub calldata: Vec<u8>,
    pub address: Vec<u8>,
    pub origin: Vec<u8>,
    pub caller: Vec<u8>,
    pub value: u128,
    pub gas_remaining: u128,
    pub gas_used: u128,
    pub events: Vec<Log>,
    pub returndata: Vec<u8>,
    pub exitcode: u128,
    pub timestamp: Instant,
}

#[derive(Clone, Debug)]
pub struct Result {
    pub gas_used: u128,
    pub gas_remaining: u128,
    pub returndata: Vec<u8>,
    pub exitcode: u128,
    pub events: Vec<Log>,
    pub runtime: f64,
    pub instruction: u128,
}

#[derive(Clone, Debug)]
pub struct State {
    pub last_instruction: Instruction,
    pub gas_used: u128,
    pub gas_remaining: u128,
    pub stack: Stack,
    pub memory: Memory,
    pub storage: Storage,
    pub events: Vec<Log>,
}

#[derive(Clone, Debug)]
pub struct Instruction {
    pub instruction: u128,
    pub opcode: u8,
    pub opcode_details: Option<Opcode>,
    pub inputs: Vec<U256>,
    pub outputs: Vec<U256>,
    pub input_operations: Vec<WrappedOpcode>,
    pub output_operations: Vec<WrappedOpcode>,
}

impl VM {
    // Creates a new VM instance
    pub fn new(
        bytecode: String,
        calldata: String,
        address: String,
        origin: String,
        caller: String,
        value: u128,
        gas_limit: u128,
    ) -> VM {
        VM {
            stack: Stack::new(),
            memory: Memory::new(),
            storage: Storage::new(),
            instruction: 1,
            bytecode: decode_hex(&bytecode.replacen("0x", "", 1)).unwrap(),
            calldata: decode_hex(&calldata.replacen("0x", "", 1)).unwrap(),
            address: decode_hex(&address.replacen("0x", "", 1)).unwrap(),
            origin: decode_hex(&origin.replacen("0x", "", 1)).unwrap(),
            caller: decode_hex(&caller.replacen("0x", "", 1)).unwrap(),
            value: value,
            gas_remaining: gas_limit.max(21000) - 21000,
            gas_used: 21000,
            events: Vec::new(),
            returndata: Vec::new(),
            exitcode: 255,
            timestamp: Instant::now(),
        }
    }

    // exits the current VM state with the given exit code and return data
    pub fn exit(&mut self, code: u128, returndata: Vec<u8>) {
        self.exitcode = code;
        self.returndata = returndata;
    }

    // consumes the given amount of gas, exiting if there is not enough remaining
    pub fn consume_gas(&mut self, amount: u128) -> bool {
        // REVERT if out of gas
        if amount > self.gas_remaining {
            return false
        }

        self.gas_remaining = self.gas_remaining.saturating_sub(amount);
        self.gas_used = self.gas_used.saturating_add(amount);
        true
    }

    // Steps to the next PC and executes the instruction
    fn _step(&mut self) -> Instruction {
        // sanity check
        if self.bytecode.len() < self.instruction as usize {
            self.exit(2, Vec::new());
            return Instruction {
                instruction: self.instruction,
                opcode: 0xff,
                opcode_details: None,
                inputs: Vec::new(),
                outputs: Vec::new(),
                input_operations: Vec::new(),
                output_operations: Vec::new(),
            }
        }

        // get the opcode at the current instruction
        let opcode = self.bytecode[(self.instruction - 1) as usize];
        let last_instruction = self.instruction;
        self.instruction += 1;

        // add the opcode to the trace
        let opcode_details = crate::ether::evm::opcodes::opcode(opcode);
        let input_frames = self.stack.peek_n(opcode_details.inputs as usize);
        let input_operations =
            input_frames.iter().map(|x| x.operation.clone()).collect::<Vec<WrappedOpcode>>();
        let inputs = input_frames.iter().map(|x| x.value).collect::<Vec<U256>>();

        // Consume the minimum gas for the opcode
        let gas_cost = opcode_details.mingas;
        match self.consume_gas(gas_cost.into()) {
            true => {}
            false => {
                self.exit(9, Vec::new());
                return Instruction {
                    instruction: last_instruction,
                    opcode: opcode,
                    opcode_details: Some(opcode_details),
                    inputs: inputs,
                    outputs: Vec::new(),
                    input_operations: input_operations,
                    output_operations: Vec::new(),
                }
            }
        }

        // convert inputs to WrappedInputs
        let wrapped_inputs = input_operations
            .iter()
            .map(|x| WrappedInput::Opcode(x.to_owned()))
            .collect::<Vec<WrappedInput>>();
        let mut operation = WrappedOpcode::new(opcode, wrapped_inputs);

        // execute the operation
        match opcode {
            // STOP
            0x00 => {
                self.exit(10, Vec::new());
                return Instruction {
                    instruction: last_instruction,
                    opcode: opcode,
                    opcode_details: Some(opcode_details),
                    inputs: inputs,
                    outputs: Vec::new(),
                    input_operations: input_operations,
                    output_operations: Vec::new(),
                }
            }

            // ADD
            0x01 => {
                let a = self.stack.pop();
                let b = self.stack.pop();

                let result = a.value.overflowing_add(b.value).0;

                // if both inputs are PUSH instructions, simplify the operation
                let mut simplified_operation = operation;
                if (0x5f..=0x7f).contains(&a.operation.opcode.code) &&
                    (0x5f..=0x7f).contains(&b.operation.opcode.code)
                {
                    simplified_operation = WrappedOpcode::new(0x7f, vec![WrappedInput::Raw(result)])
                }

                self.stack.push(result, simplified_operation);
            }

            // MUL
            0x02 => {
                let a = self.stack.pop();
                let b = self.stack.pop();

                let result = a.value.overflowing_mul(b.value).0;

                // if both inputs are PUSH instructions, simplify the operation
                let mut simplified_operation = operation;
                if (0x5f..=0x7f).contains(&a.operation.opcode.code) &&
                    (0x5f..=0x7f).contains(&b.operation.opcode.code)
                {
                    simplified_operation = WrappedOpcode::new(0x7f, vec![WrappedInput::Raw(result)])
                }

                self.stack.push(result, simplified_operation);
            }

            // SUB
            0x03 => {
                let a = self.stack.pop();
                let b = self.stack.pop();

                let result = a.value.overflowing_sub(b.value).0;

                // if both inputs are PUSH instructions, simplify the operation
                let mut simplified_operation = operation;
                if (0x5f..=0x7f).contains(&a.operation.opcode.code) &&
                    (0x5f..=0x7f).contains(&b.operation.opcode.code)
                {
                    simplified_operation = WrappedOpcode::new(0x7f, vec![WrappedInput::Raw(result)])
                }

                self.stack.push(result, simplified_operation);
            }

            // DIV
            0x04 => {
                let numerator = self.stack.pop();
                let denominator = self.stack.pop();

                let mut result = U256::zero();
                if !denominator.value.is_zero() {
                    result = numerator.value.div(denominator.value);
                }

                // if both inputs are PUSH instructions, simplify the operation
                let mut simplified_operation = operation;
                if (0x5f..=0x7f).contains(&numerator.operation.opcode.code) &&
                    (0x5f..=0x7f).contains(&denominator.operation.opcode.code)
                {
                    simplified_operation = WrappedOpcode::new(0x7f, vec![WrappedInput::Raw(result)])
                }

                self.stack.push(result, simplified_operation);
            }

            // SDIV
            0x05 => {
                let numerator = self.stack.pop();
                let denominator = self.stack.pop();

                let mut result = I256::zero();
                if !denominator.value.is_zero() {
                    result = sign_uint(numerator.value).div(sign_uint(denominator.value));
                }

                // if both inputs are PUSH instructions, simplify the operation
                let mut simplified_operation = operation;
                if (0x5f..=0x7f).contains(&numerator.operation.opcode.code) &&
                    (0x5f..=0x7f).contains(&denominator.operation.opcode.code)
                {
                    simplified_operation =
                        WrappedOpcode::new(0x7f, vec![WrappedInput::Raw(result.into_raw())])
                }

                self.stack.push(result.into_raw(), simplified_operation);
            }

            // MOD
            0x06 => {
                let a = self.stack.pop();
                let modulus = self.stack.pop();

                let mut result = U256::zero();
                if !modulus.value.is_zero() {
                    result = a.value.rem(modulus.value);
                }

                // if both inputs are PUSH instructions, simplify the operation
                let mut simplified_operation = operation;
                if (0x5f..=0x7f).contains(&a.operation.opcode.code) &&
                    (0x5f..=0x7f).contains(&modulus.operation.opcode.code)
                {
                    simplified_operation = WrappedOpcode::new(0x7f, vec![WrappedInput::Raw(result)])
                }

                self.stack.push(result, simplified_operation);
            }

            // SMOD
            0x07 => {
                let a = self.stack.pop();
                let modulus = self.stack.pop();

                let mut result = I256::zero();
                if !modulus.value.is_zero() {
                    result = sign_uint(a.value).rem(sign_uint(modulus.value));
                }

                // if both inputs are PUSH instructions, simplify the operation
                let mut simplified_operation = operation;
                if (0x5f..=0x7f).contains(&a.operation.opcode.code) &&
                    (0x5f..=0x7f).contains(&modulus.operation.opcode.code)
                {
                    simplified_operation =
                        WrappedOpcode::new(0x7f, vec![WrappedInput::Raw(result.into_raw())])
                }

                self.stack.push(result.into_raw(), simplified_operation);
            }

            // ADDMOD
            0x08 => {
                let a = self.stack.pop();
                let b = self.stack.pop();
                let modulus = self.stack.pop();

                let mut result = U256::zero();
                if !modulus.value.is_zero() {
                    result = a.value.overflowing_add(b.value).0.rem(modulus.value);
                }

                // if both inputs are PUSH instructions, simplify the operation
                let mut simplified_operation = operation;
                if (0x5f..=0x7f).contains(&a.operation.opcode.code) &&
                    (0x5f..=0x7f).contains(&b.operation.opcode.code)
                {
                    simplified_operation = WrappedOpcode::new(0x7f, vec![WrappedInput::Raw(result)])
                }

                self.stack.push(result, simplified_operation);
            }

            // MULMOD
            0x09 => {
                let a = self.stack.pop();
                let b = self.stack.pop();
                let modulus = self.stack.pop();

                let mut result = U256::zero();
                if !modulus.value.is_zero() {
                    result = a.value.overflowing_mul(b.value).0.rem(modulus.value);
                }

                // if both inputs are PUSH instructions, simplify the operation
                let mut simplified_operation = operation;
                if (0x5f..=0x7f).contains(&a.operation.opcode.code) &&
                    (0x5f..=0x7f).contains(&b.operation.opcode.code)
                {
                    simplified_operation = WrappedOpcode::new(0x7f, vec![WrappedInput::Raw(result)])
                }

                self.stack.push(result, simplified_operation);
            }

            // EXP
            0x0A => {
                let a = self.stack.pop();
                let exponent = self.stack.pop();

                let result = a.value.overflowing_pow(exponent.value).0;

                // if both inputs are PUSH instructions, simplify the operation
                let mut simplified_operation = operation;
                if (0x5f..=0x7f).contains(&a.operation.opcode.code) &&
                    (0x5f..=0x7f).contains(&exponent.operation.opcode.code)
                {
                    simplified_operation = WrappedOpcode::new(0x7f, vec![WrappedInput::Raw(result)])
                }

                self.stack.push(result, simplified_operation);
            }

            // SIGNEXTEND
            0x0B => {
                let x = self.stack.pop().value;
                let b = self.stack.pop().value;

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
                let a = self.stack.pop().value;
                let b = self.stack.pop().value;

                match a.lt(&b) {
                    true => self.stack.push(U256::from(1u8), operation),
                    false => self.stack.push(U256::zero(), operation),
                }
            }

            // GT
            0x11 => {
                let a = self.stack.pop().value;
                let b = self.stack.pop().value;

                match a.gt(&b) {
                    true => self.stack.push(U256::from(1u8), operation),
                    false => self.stack.push(U256::zero(), operation),
                }
            }

            // SLT
            0x12 => {
                let a = self.stack.pop().value;
                let b = self.stack.pop().value;

                match sign_uint(a).lt(&sign_uint(b)) {
                    true => self.stack.push(U256::from(1u8), operation),
                    false => self.stack.push(U256::zero(), operation),
                }
            }

            // SGT
            0x13 => {
                let a = self.stack.pop().value;
                let b = self.stack.pop().value;

                match sign_uint(a).gt(&sign_uint(b)) {
                    true => self.stack.push(U256::from(1u8), operation),
                    false => self.stack.push(U256::zero(), operation),
                }
            }

            // EQ
            0x14 => {
                let a = self.stack.pop().value;
                let b = self.stack.pop().value;

                match a.eq(&b) {
                    true => self.stack.push(U256::from(1u8), operation),
                    false => self.stack.push(U256::zero(), operation),
                }
            }

            // ISZERO
            0x15 => {
                let a = self.stack.pop().value;

                match a.eq(&U256::from(0u8)) {
                    true => self.stack.push(U256::from(1u8), operation),
                    false => self.stack.push(U256::zero(), operation),
                }
            }

            // AND
            0x16 => {
                let a = self.stack.pop();
                let b = self.stack.pop();

                let result = a.value & b.value;

                // if both inputs are PUSH instructions, simplify the operation
                let mut simplified_operation = operation;
                if (0x5f..=0x7f).contains(&a.operation.opcode.code) &&
                    (0x5f..=0x7f).contains(&b.operation.opcode.code)
                {
                    simplified_operation = WrappedOpcode::new(0x7f, vec![WrappedInput::Raw(result)])
                }

                self.stack.push(result, simplified_operation);
            }

            // OR
            0x17 => {
                let a = self.stack.pop();
                let b = self.stack.pop();

                let result = a.value | b.value;

                // if both inputs are PUSH instructions, simplify the operation
                let mut simplified_operation = operation;
                if (0x5f..=0x7f).contains(&a.operation.opcode.code) &&
                    (0x5f..=0x7f).contains(&b.operation.opcode.code)
                {
                    simplified_operation = WrappedOpcode::new(0x7f, vec![WrappedInput::Raw(result)])
                }

                self.stack.push(result, simplified_operation);
            }

            // XOR
            0x18 => {
                let a = self.stack.pop();
                let b = self.stack.pop();

                let result = a.value ^ b.value;

                // if both inputs are PUSH instructions, simplify the operation
                let mut simplified_operation = operation;
                if (0x5f..=0x7f).contains(&a.operation.opcode.code) &&
                    (0x5f..=0x7f).contains(&b.operation.opcode.code)
                {
                    simplified_operation = WrappedOpcode::new(0x7f, vec![WrappedInput::Raw(result)])
                }

                self.stack.push(result, simplified_operation);
            }

            // NOT
            0x19 => {
                let a = self.stack.pop();

                let result = !a.value;

                // if both inputs are PUSH instructions, simplify the operation
                let mut simplified_operation = operation;
                if (0x5f..=0x7f).contains(&a.operation.opcode.code) {
                    simplified_operation = WrappedOpcode::new(0x7f, vec![WrappedInput::Raw(result)])
                }

                self.stack.push(result, simplified_operation);
            }

            // BYTE
            0x1A => {
                let b = self.stack.pop().value;
                let a = self.stack.pop().value;

                if b >= U256::from(32u32) {
                    self.stack.push(U256::zero(), operation)
                } else {
                    let result =
                        a / (U256::from(256u32).pow(U256::from(31u32) - b)) % U256::from(256u32);

                    self.stack.push(result, operation);
                }
            }

            // SHL
            0x1B => {
                let a = self.stack.pop();
                let b = self.stack.pop();

                let mut result = b.value.shl(a.value);

                // if shift is greater than 255, result is 0
                if a.value > U256::from(255u8) {
                    result = U256::zero();
                }

                // if both inputs are PUSH instructions, simplify the operation
                let mut simplified_operation = operation;
                if (0x5f..=0x7f).contains(&a.operation.opcode.code) &&
                    (0x5f..=0x7f).contains(&b.operation.opcode.code)
                {
                    simplified_operation = WrappedOpcode::new(0x7f, vec![WrappedInput::Raw(result)])
                }

                self.stack.push(result, simplified_operation);
            }

            // SHR
            0x1C => {
                let a = self.stack.pop();
                let b = self.stack.pop();

                let mut result = U256::zero();
                if !b.value.is_zero() {
                    result = b.value.shr(a.value);
                }

                // if shift is greater than 255, result is 0
                if a.value > U256::from(255u8) {
                    result = U256::zero();
                }

                // if both inputs are PUSH instructions, simplify the operation
                let mut simplified_operation = operation;
                if (0x5f..=0x7f).contains(&a.operation.opcode.code) &&
                    (0x5f..=0x7f).contains(&b.operation.opcode.code)
                {
                    simplified_operation = WrappedOpcode::new(0x7f, vec![WrappedInput::Raw(result)])
                }

                self.stack.push(result, simplified_operation);
            }

            // SAR
            0x1D => {
                let a = self.stack.pop();
                let b = self.stack.pop();

                // convert a to usize
                let usize_a: usize = match a.value.try_into() {
                    Ok(x) => x,
                    Err(_) => {
                        self.exit(2, Vec::new());
                        return Instruction {
                            instruction: last_instruction,
                            opcode: opcode,
                            opcode_details: Some(opcode_details),
                            inputs: inputs,
                            outputs: Vec::new(),
                            input_operations: input_operations,
                            output_operations: Vec::new(),
                        }
                    }
                };

                let mut result = I256::zero();
                if !b.value.is_zero() {
                    result = sign_uint(b.value).shr(usize_a);
                }

                // if both inputs are PUSH instructions, simplify the operation
                let mut simplified_operation = operation;
                if (0x5f..=0x7f).contains(&a.operation.opcode.code) &&
                    (0x5f..=0x7f).contains(&b.operation.opcode.code)
                {
                    simplified_operation =
                        WrappedOpcode::new(0x7f, vec![WrappedInput::Raw(result.into_raw())])
                }

                self.stack.push(result.into_raw(), simplified_operation);
            }

            // SHA3
            0x20 => {
                let offset = self.stack.pop().value;
                let size = self.stack.pop().value;

                // Safely convert U256 to usize
                let offset: usize = match offset.try_into() {
                    Ok(x) => x,
                    Err(_) => {
                        self.exit(2, Vec::new());
                        return Instruction {
                            instruction: last_instruction,
                            opcode: opcode,
                            opcode_details: Some(opcode_details),
                            inputs: inputs,
                            outputs: Vec::new(),
                            input_operations: input_operations,
                            output_operations: Vec::new(),
                        }
                    }
                };
                let size: usize = match size.try_into() {
                    Ok(x) => x,
                    Err(_) => {
                        self.exit(2, Vec::new());
                        return Instruction {
                            instruction: last_instruction,
                            opcode: opcode,
                            opcode_details: Some(opcode_details),
                            inputs: inputs,
                            outputs: Vec::new(),
                            input_operations: input_operations,
                            output_operations: Vec::new(),
                        }
                    }
                };

                let data = self.memory.read(offset, size);
                let result = keccak256(data);

                self.stack.push(U256::from(result), operation);
            }

            // ADDRESS
            0x30 => {
                let mut result = [0u8; 32];

                // copy address into result
                result[12..].copy_from_slice(&self.address);

                self.stack.push(U256::from(result), operation);
            }

            // BALANCE
            0x31 => {
                self.stack.pop();

                // balance is set to 1 wei because we won't run into div by 0 errors
                self.stack.push(U256::from(1), operation);
            }

            // ORIGIN
            0x32 => {
                let mut result = [0u8; 32];

                // copy address into result
                result[12..].copy_from_slice(&self.origin);

                self.stack.push(U256::from(result), operation);
            }

            // CALLER
            0x33 => {
                let mut result = [0u8; 32];

                // copy address into result
                result[12..].copy_from_slice(&self.caller);

                self.stack.push(U256::from(result), operation);
            }

            // CALLVALUE
            0x34 => {
                self.stack.push(U256::from(self.value), operation);
            }

            // CALLDATALOAD
            0x35 => {
                let i = self.stack.pop().value;

                // Safely convert U256 to usize
                let i: usize = match i.try_into() {
                    Ok(x) => x,
                    Err(_) => {
                        self.exit(2, Vec::new());
                        return Instruction {
                            instruction: last_instruction,
                            opcode: opcode,
                            opcode_details: Some(opcode_details),
                            inputs: inputs,
                            outputs: Vec::new(),
                            input_operations: input_operations,
                            output_operations: Vec::new(),
                        }
                    }
                };

                let result = if i + 32 > self.calldata.len() {
                    let mut value = [0u8; 32];

                    if i <= self.calldata.len() {
                        value[..self.calldata.len() - i].copy_from_slice(&self.calldata[i..]);
                    }

                    U256::from(value)
                } else {
                    U256::from(&self.calldata[i..i + 32])
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
                let dest_offset = self.stack.pop().value;
                let offset = self.stack.pop().value;
                let size = self.stack.pop().value;

                // Safely convert U256 to usize
                let dest_offset: usize = match dest_offset.try_into() {
                    Ok(x) => x,
                    Err(_) => {
                        self.exit(2, Vec::new());
                        return Instruction {
                            instruction: last_instruction,
                            opcode: opcode,
                            opcode_details: Some(opcode_details),
                            inputs: inputs,
                            outputs: Vec::new(),
                            input_operations: input_operations,
                            output_operations: Vec::new(),
                        }
                    }
                };
                let offset: usize = match offset.try_into() {
                    Ok(x) => x,
                    Err(_) => {
                        self.exit(2, Vec::new());
                        return Instruction {
                            instruction: last_instruction,
                            opcode: opcode,
                            opcode_details: Some(opcode_details),
                            inputs: inputs,
                            outputs: Vec::new(),
                            input_operations: input_operations,
                            output_operations: Vec::new(),
                        }
                    }
                };
                let size: usize = match size.try_into() {
                    Ok(x) => x,
                    Err(_) => {
                        self.exit(2, Vec::new());
                        return Instruction {
                            instruction: last_instruction,
                            opcode: opcode,
                            opcode_details: Some(opcode_details),
                            inputs: inputs,
                            outputs: Vec::new(),
                            input_operations: input_operations,
                            output_operations: Vec::new(),
                        }
                    }
                };

                let value_offset_safe = (offset + size).min(self.calldata.len());
                let mut value =
                    self.calldata.get(offset..value_offset_safe).unwrap_or(&[]).to_owned();

                // pad value with 0x00
                if value.len() < size {
                    value.resize(size, 0u8);
                }

                self.memory.store(dest_offset, size, &value);
            }

            // CODESIZE
            0x38 => {
                let result = U256::from(self.bytecode.len() as u128);

                self.stack.push(result, operation);
            }

            // CODECOPY
            0x39 => {
                let dest_offset = self.stack.pop().value;
                let offset = self.stack.pop().value;
                let size = self.stack.pop().value;

                // Safely convert U256 to usize
                let dest_offset: usize = match dest_offset.try_into() {
                    Ok(x) => x,
                    Err(_) => {
                        self.exit(2, Vec::new());
                        return Instruction {
                            instruction: last_instruction,
                            opcode: opcode,
                            opcode_details: Some(opcode_details),
                            inputs: inputs,
                            outputs: Vec::new(),
                            input_operations: input_operations,
                            output_operations: Vec::new(),
                        }
                    }
                };
                let offset: usize = match offset.try_into() {
                    Ok(x) => x,
                    Err(_) => {
                        self.exit(2, Vec::new());
                        return Instruction {
                            instruction: last_instruction,
                            opcode: opcode,
                            opcode_details: Some(opcode_details),
                            inputs: inputs,
                            outputs: Vec::new(),
                            input_operations: input_operations,
                            output_operations: Vec::new(),
                        }
                    }
                };
                let size: usize = match size.try_into() {
                    Ok(x) => x,
                    Err(_) => {
                        self.exit(2, Vec::new());
                        return Instruction {
                            instruction: last_instruction,
                            opcode: opcode,
                            opcode_details: Some(opcode_details),
                            inputs: inputs,
                            outputs: Vec::new(),
                            input_operations: input_operations,
                            output_operations: Vec::new(),
                        }
                    }
                };

                let value_offset_safe = (offset + size).min(self.bytecode.len());
                let mut value =
                    self.bytecode.get(offset..value_offset_safe).unwrap_or(&[]).to_owned();

                // pad value with 0x00
                if value.len() < size {
                    value.resize(size, 0u8);
                }

                self.memory.store(dest_offset, size, &value);
            }

            // GASPRICE
            0x3A => {
                self.stack.push(U256::from(1), operation);
            }

            // EXTCODESIZE
            0x3B => {
                self.stack.pop();
                self.stack.push(U256::from(1), operation);
            }

            // EXTCODECOPY
            0x3C => {
                self.stack.pop();
                let dest_offset = self.stack.pop().value;
                self.stack.pop();
                let size = self.stack.pop().value;

                // Safely convert U256 to usize
                let dest_offset: usize = match dest_offset.try_into() {
                    Ok(x) => x,
                    Err(_) => {
                        self.exit(2, Vec::new());
                        return Instruction {
                            instruction: last_instruction,
                            opcode: opcode,
                            opcode_details: Some(opcode_details),
                            inputs: inputs,
                            outputs: Vec::new(),
                            input_operations: input_operations,
                            output_operations: Vec::new(),
                        }
                    }
                };
                let size: usize = match size.try_into() {
                    Ok(x) => x,
                    Err(_) => {
                        self.exit(2, Vec::new());
                        return Instruction {
                            instruction: last_instruction,
                            opcode: opcode,
                            opcode_details: Some(opcode_details),
                            inputs: inputs,
                            outputs: Vec::new(),
                            input_operations: input_operations,
                            output_operations: Vec::new(),
                        }
                    }
                };

                let mut value = Vec::with_capacity(size);
                value.resize(size, 0xff);

                self.memory.store(dest_offset, size, &value);
            }

            // RETURNDATASIZE
            0x3D => {
                self.stack.push(U256::from(1u8), operation);
            }

            // RETURNDATACOPY
            0x3E => {
                let dest_offset = self.stack.pop().value;
                self.stack.pop();
                let size = self.stack.pop().value;

                // Safely convert U256 to usize
                let dest_offset: usize = match dest_offset.try_into() {
                    Ok(x) => x,
                    Err(_) => {
                        self.exit(2, Vec::new());
                        return Instruction {
                            instruction: last_instruction,
                            opcode: opcode,
                            opcode_details: Some(opcode_details),
                            inputs: inputs,
                            outputs: Vec::new(),
                            input_operations: input_operations,
                            output_operations: Vec::new(),
                        }
                    }
                };
                let size: usize = match size.try_into() {
                    Ok(x) => x,
                    Err(_) => {
                        self.exit(2, Vec::new());
                        return Instruction {
                            instruction: last_instruction,
                            opcode: opcode,
                            opcode_details: Some(opcode_details),
                            inputs: inputs,
                            outputs: Vec::new(),
                            input_operations: input_operations,
                            output_operations: Vec::new(),
                        }
                    }
                };

                let mut value = Vec::with_capacity(size);
                value.resize(size, 0xff);

                self.memory.store(dest_offset, size, &value);
            }

            // EXTCODEHASH and BLOCKHASH
            0x3F | 0x40 => {
                self.stack.pop();

                self.stack.push(U256::zero(), operation);
            }

            // COINBASE
            0x41 => {
                self.stack.push(
                    U256::from_str("0x6865696d64616c6c00000000636f696e62617365").unwrap(),
                    operation,
                );
            }

            // TIMESTAMP
            0x42 => {
                let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();

                self.stack.push(U256::from(timestamp), operation);
            }

            // NUMBER -> BASEFEE
            (0x43..=0x48) => {
                self.stack.push(U256::from(1u8), operation);
            }

            // POP
            0x50 => {
                self.stack.pop();
            }

            // MLOAD
            0x51 => {
                let i = self.stack.pop().value;

                // Safely convert U256 to usize
                let i: usize = match i.try_into() {
                    Ok(x) => x,
                    Err(_) => {
                        self.exit(2, Vec::new());
                        return Instruction {
                            instruction: last_instruction,
                            opcode: opcode,
                            opcode_details: Some(opcode_details),
                            inputs: inputs,
                            outputs: Vec::new(),
                            input_operations: input_operations,
                            output_operations: Vec::new(),
                        }
                    }
                };

                let result = U256::from(self.memory.read(i, 32).as_slice());

                self.stack.push(result, operation);
            }

            // MSTORE
            0x52 => {
                let offset = self.stack.pop().value;
                let value = self.stack.pop().value;

                // Safely convert U256 to usize
                let offset: usize = match offset.try_into() {
                    Ok(x) => x,
                    Err(_) => {
                        self.exit(2, Vec::new());
                        return Instruction {
                            instruction: last_instruction,
                            opcode: opcode,
                            opcode_details: Some(opcode_details),
                            inputs: inputs,
                            outputs: Vec::new(),
                            input_operations: input_operations,
                            output_operations: Vec::new(),
                        }
                    }
                };

                self.memory.store(offset, 32, value.encode().as_slice());
            }

            // MSTORE8
            0x53 => {
                let offset = self.stack.pop().value;
                let value = self.stack.pop().value;

                // Safely convert U256 to usize
                let offset: usize = match offset.try_into() {
                    Ok(x) => x,
                    Err(_) => {
                        self.exit(2, Vec::new());
                        return Instruction {
                            instruction: last_instruction,
                            opcode: opcode,
                            opcode_details: Some(opcode_details),
                            inputs: inputs,
                            outputs: Vec::new(),
                            input_operations: input_operations,
                            output_operations: Vec::new(),
                        }
                    }
                };

                self.memory.store(offset, 1, &[value.encode()[31]]);
            }

            // SLOAD
            0x54 => {
                let key = self.stack.pop().value;

                self.stack.push(U256::from(self.storage.load(key.into())), operation)
            }

            // SSTORE
            0x55 => {
                let key = self.stack.pop().value;
                let value = self.stack.pop().value;

                self.storage.store(key.into(), value.into());
            }

            // JUMP
            0x56 => {
                let pc = self.stack.pop().value;

                // Safely convert U256 to u128
                let pc: u128 = match pc.try_into() {
                    Ok(x) => x,
                    Err(_) => {
                        self.exit(2, Vec::new());
                        return Instruction {
                            instruction: last_instruction,
                            opcode: opcode,
                            opcode_details: Some(opcode_details),
                            inputs: inputs,
                            outputs: Vec::new(),
                            input_operations: input_operations,
                            output_operations: Vec::new(),
                        }
                    }
                };

                // Check if JUMPDEST is valid and throw with 790 if not (invalid jump destination)
                if (pc <= self.bytecode.len().try_into().unwrap()) &&
                    (self.bytecode[pc as usize] != 0x5b)
                {
                    self.exit(790, Vec::new());
                    return Instruction {
                        instruction: last_instruction,
                        opcode: opcode,
                        opcode_details: Some(opcode_details),
                        inputs: inputs,
                        outputs: Vec::new(),
                        input_operations: input_operations,
                        output_operations: Vec::new(),
                    }
                } else {
                    self.instruction = pc + 1;
                }
            }

            // JUMPI
            0x57 => {
                let pc = self.stack.pop().value;
                let condition = self.stack.pop().value;

                // Safely convert U256 to u128
                let pc: u128 = match pc.try_into() {
                    Ok(x) => x,
                    Err(_) => {
                        self.exit(2, Vec::new());
                        return Instruction {
                            instruction: last_instruction,
                            opcode: opcode,
                            opcode_details: Some(opcode_details),
                            inputs: inputs,
                            outputs: Vec::new(),
                            input_operations: input_operations,
                            output_operations: Vec::new(),
                        }
                    }
                };

                if !condition.eq(&U256::from(0u8)) {
                    // Check if JUMPDEST is valid and throw with 790 if not (invalid jump
                    // destination)
                    if (pc <= self.bytecode.len().try_into().unwrap()) &&
                        (self.bytecode[pc as usize] != 0x5b)
                    {
                        self.exit(790, Vec::new());
                        return Instruction {
                            instruction: last_instruction,
                            opcode: opcode,
                            opcode_details: Some(opcode_details),
                            inputs: inputs,
                            outputs: Vec::new(),
                            input_operations: input_operations,
                            output_operations: Vec::new(),
                        }
                    } else {
                        self.instruction = pc + 1;
                    }
                }
            }

            // JUMPDEST
            0x5B => {}

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
                self.stack.push(U256::zero(), operation);
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
                let new_operation_inputs = vec![WrappedInput::Raw(U256::from(bytes))];

                operation.inputs = new_operation_inputs;

                // Push the bytes to the stack
                self.stack.push(U256::from(bytes), operation);
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
                let offset = self.stack.pop().value;
                let size = self.stack.pop().value;
                let topics =
                    self.stack.pop_n(topic_count as usize).iter().map(|x| x.value).collect();

                // Safely convert U256 to usize
                let offset: usize = match offset.try_into() {
                    Ok(x) => x,
                    Err(_) => {
                        self.exit(2, Vec::new());
                        return Instruction {
                            instruction: last_instruction,
                            opcode: opcode,
                            opcode_details: Some(opcode_details),
                            inputs: inputs,
                            outputs: Vec::new(),
                            input_operations: input_operations,
                            output_operations: Vec::new(),
                        }
                    }
                };
                let size: usize = match size.try_into() {
                    Ok(x) => x,
                    Err(_) => {
                        self.exit(2, Vec::new());
                        return Instruction {
                            instruction: last_instruction,
                            opcode: opcode,
                            opcode_details: Some(opcode_details),
                            inputs: inputs,
                            outputs: Vec::new(),
                            input_operations: input_operations,
                            output_operations: Vec::new(),
                        }
                    }
                };

                let data = self.memory.read(offset, size);

                // no need for a panic check because the length of events should never be larger
                // than a u128
                self.events.push(Log::new(self.events.len().try_into().unwrap(), topics, &data))
            }

            // CREATE
            0xF0 => {
                self.stack.pop_n(3);

                self.stack.push(
                    U256::from_str("0x6865696d64616c6c000000000000637265617465").unwrap(),
                    operation,
                );
            }

            // CALL, CALLCODE
            0xF1 | 0xF2 => {
                self.stack.pop_n(7);

                self.stack.push(U256::from(1u8), operation);
            }

            // RETURN
            0xF3 => {
                let offset = self.stack.pop().value;
                let size = self.stack.pop().value;

                // Safely convert U256 to usize
                let offset: usize = match offset.try_into() {
                    Ok(x) => x,
                    Err(_) => {
                        self.exit(2, Vec::new());
                        return Instruction {
                            instruction: last_instruction,
                            opcode: opcode,
                            opcode_details: Some(opcode_details),
                            inputs: inputs,
                            outputs: Vec::new(),
                            input_operations: input_operations,
                            output_operations: Vec::new(),
                        }
                    }
                };
                let size: usize = match size.try_into() {
                    Ok(x) => x,
                    Err(_) => {
                        self.exit(2, Vec::new());
                        return Instruction {
                            instruction: last_instruction,
                            opcode: opcode,
                            opcode_details: Some(opcode_details),
                            inputs: inputs,
                            outputs: Vec::new(),
                            input_operations: input_operations,
                            output_operations: Vec::new(),
                        }
                    }
                };

                self.exit(0, self.memory.read(offset, size));
            }

            // DELEGATECALL, STATICCALL
            0xF4 | 0xFA => {
                self.stack.pop_n(6);

                self.stack.push(U256::from(1u8), operation);
            }

            // CREATE2
            0xF5 => {
                self.stack.pop_n(4);

                self.stack.push(
                    U256::from_str("0x6865696d64616c6c000000000063726561746532").unwrap(),
                    operation,
                );
            }

            // REVERT
            0xFD => {
                let offset = self.stack.pop().value;
                let size = self.stack.pop().value;

                // Safely convert U256 to usize
                let offset: usize = match offset.try_into() {
                    Ok(x) => x,
                    Err(_) => {
                        self.exit(2, Vec::new());
                        return Instruction {
                            instruction: last_instruction,
                            opcode: opcode,
                            opcode_details: Some(opcode_details),
                            inputs: inputs,
                            outputs: Vec::new(),
                            input_operations: input_operations,
                            output_operations: Vec::new(),
                        }
                    }
                };
                let size: usize = match size.try_into() {
                    Ok(x) => x,
                    Err(_) => {
                        self.exit(2, Vec::new());
                        return Instruction {
                            instruction: last_instruction,
                            opcode: opcode,
                            opcode_details: Some(opcode_details),
                            inputs: inputs,
                            outputs: Vec::new(),
                            input_operations: input_operations,
                            output_operations: Vec::new(),
                        }
                    }
                };

                self.exit(1, self.memory.read(offset, size));
            }

            // INVALID & SELFDESTRUCT
            _ => {
                self.consume_gas(self.gas_remaining);
                self.exit(1, Vec::new());
            }
        }

        // get outputs
        let output_frames = self.stack.peek_n(opcode_details.outputs as usize);
        let output_operations =
            output_frames.iter().map(|x| x.operation.clone()).collect::<Vec<WrappedOpcode>>();
        let outputs = output_frames.iter().map(|x| x.value).collect::<Vec<U256>>();

        Instruction {
            instruction: last_instruction,
            opcode: opcode,
            opcode_details: Some(opcode_details),
            inputs: inputs,
            outputs: outputs,
            input_operations: input_operations,
            output_operations: output_operations,
        }
    }

    // Executes the next instruction in the VM and returns a snapshot its the state
    pub fn step(&mut self) -> State {
        let instruction = self._step();

        State {
            last_instruction: instruction,
            gas_used: self.gas_used,
            gas_remaining: self.gas_remaining,
            stack: self.stack.clone(),
            memory: self.memory.clone(),
            storage: self.storage.clone(),
            events: self.events.clone(),
        }
    }

    // View the next n instructions without executing them
    pub fn peek(&mut self, n: usize) -> Vec<State> {
        let mut states = Vec::new();
        let mut vm_clone = self.clone();

        for _ in 0..n {
            if vm_clone.bytecode.len() < vm_clone.instruction as usize ||
                vm_clone.exitcode != 255 ||
                !vm_clone.returndata.is_empty()
            {
                break
            }
            states.push(vm_clone.step());
        }

        states
    }

    // Resets the VM state for a new execution
    pub fn reset(&mut self) {
        self.stack = Stack::new();
        self.memory = Memory::new();
        self.instruction = 1;
        self.gas_remaining = u128::max_value();
        self.gas_used = 21000;
        self.events = Vec::new();
        self.returndata = Vec::new();
        self.exitcode = 255;
        self.timestamp = Instant::now();
    }

    // Executes the code until finished
    pub fn execute(&mut self) -> Result {
        while self.bytecode.len() >= self.instruction as usize {
            self.step();

            if self.exitcode != 255 || !self.returndata.is_empty() {
                break
            }
        }

        Result {
            gas_used: self.gas_used,
            gas_remaining: self.gas_remaining,
            returndata: self.returndata.to_owned(),
            exitcode: self.exitcode,
            events: self.events.clone(),
            runtime: self.timestamp.elapsed().as_secs_f64(),
            instruction: self.instruction,
        }
    }

    // Executes provided calldata until finished
    pub fn call(&mut self, calldata: String, value: u128) -> Result {
        // reset the VM temp state
        self.reset();
        self.calldata = decode_hex(&calldata.replacen("0x", "", 1)).unwrap();
        self.value = value;

        self.execute()
    }
}
