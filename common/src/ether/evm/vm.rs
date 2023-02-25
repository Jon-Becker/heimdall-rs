use std::{
    ops::{Div, Rem, Shl, Shr},
    str::FromStr,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use ethers::{abi::AbiEncode, prelude::U256, utils::keccak256, types::I256};

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
    pub bytecode: String,
    pub calldata: String,
    pub address: String,
    pub origin: String,
    pub caller: String,
    pub value: u128,
    pub gas_remaining: u128,
    pub gas_used: u128,
    pub events: Vec<Log>,
    pub returndata: String,
    pub exitcode: u128,
    pub timestamp: Instant,
}

#[derive(Clone, Debug)]
pub struct Result {
    pub gas_used: u128,
    pub gas_remaining: u128,
    pub returndata: String,
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
    pub opcode: String,
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
        mut gas_limit: u128,
    ) -> VM {
        if gas_limit < 21000 {
            gas_limit = 21000;
        }

        VM {
            stack: Stack::new(),
            memory: Memory::new(),
            storage: Storage::new(),
            instruction: 1,
            bytecode: format!("0x{}", bytecode.replace("0x", "")),
            calldata: calldata.replace("0x", ""),
            address: address.replace("0x", ""),
            origin: origin.replace("0x", ""),
            caller: caller.replace("0x", ""),
            value: value,
            gas_remaining: gas_limit - 21000,
            gas_used: 21000,
            events: Vec::new(),
            returndata: String::new(),
            exitcode: 255,
            timestamp: Instant::now(),
        }
    }

    pub fn exit(&mut self, code: u128, returndata: &str) {
        self.exitcode = code;
        self.returndata = returndata.to_string();

        return;
    }

    pub fn consume_gas(&mut self, amount: u128) -> bool {
       
        // REVERT if out of gas
        if amount > self.gas_remaining {
            return false;
        }

        self.gas_remaining = self.gas_remaining.saturating_sub(amount);
        self.gas_used = self.gas_used.saturating_add(amount);
        return true;
    }

    // Steps to the next PC and executes the instruction
    fn _step(&mut self) -> Instruction {

        // sanity check
        if self.bytecode.len() < (self.instruction * 2 + 2) as usize {
            self.exit(2, "0x");
            Instruction {
                instruction: self.instruction,
                opcode: "PANIC".to_string(),
                opcode_details: None,
                inputs: Vec::new(),
                outputs: Vec::new(),
                input_operations: Vec::new(),
                output_operations: Vec::new(),
            };
        }

        // get the opcode at the current instruction
        let opcode = self.bytecode
            [(self.instruction * 2) as usize..(self.instruction * 2 + 2) as usize]
            .to_string();
        let last_instruction = self.instruction;
        self.instruction += 1;

        // add the opcode to the trace
        let opcode_details = crate::ether::evm::opcodes::opcode(opcode.replace("0x", "").as_str());
        let input_frames = self.stack.peek_n(opcode_details.inputs as usize);
        let input_operations = input_frames
            .iter()
            .map(|x| x.operation.clone())
            .collect::<Vec<WrappedOpcode>>();
        let inputs = input_frames.iter().map(|x| x.value).collect::<Vec<U256>>();

        // Consume the minimum gas for the opcode
        let gas_cost = opcode_details.mingas;
        match self.consume_gas(gas_cost.into()) {
            true => {}
            false => {
                self.exit(0, "0x");
                return Instruction {
                    instruction: last_instruction,
                    opcode: opcode,
                    opcode_details: Some(opcode_details),
                    inputs: inputs,
                    outputs: Vec::new(),
                    input_operations: input_operations,
                    output_operations: Vec::new(),
                };
            }
        }

        match U256::from_str(&opcode) {
            Ok(_opcode) => {
                let op = _opcode.as_usize();

                // convert inputs to WrappedInputs
                let wrapped_inputs = input_operations
                    .iter()
                    .map(|x| WrappedInput::Opcode(x.to_owned()))
                    .collect::<Vec<WrappedInput>>();
                let mut operation = WrappedOpcode::new(op, wrapped_inputs);

                // execute the opcode

                // STOP
                if op == 0x00 {
                    self.exit(0, "0x");
                    return Instruction {
                        instruction: last_instruction,
                        opcode: opcode,
                        opcode_details: Some(opcode_details),
                        inputs: inputs,
                        outputs: Vec::new(),
                        input_operations: input_operations,
                        output_operations: Vec::new(),
                    };
                }

                // ADD
                if op == 0x01 {
                    let a = self.stack.pop();
                    let b = self.stack.pop();

                    let result = a.value.overflowing_add(b.value).0;

                    let simplified_operation = 
                        match a.operation.opcode.name.starts_with("PUSH") 
                        &&    b.operation.opcode.name.starts_with("PUSH") {
                            true => {
                                WrappedOpcode::new(
                                    0x7f,
                                    vec![ WrappedInput::Raw(result), ],
                                )
                            },
                            false => operation.clone()
                        };

                    self.stack.push(
                        result.encode_hex().as_str(),
                        simplified_operation
                    );
                }

                // MUL
                if op == 0x02 {
                    let a = self.stack.pop();
                    let b = self.stack.pop();

                    let result = a.value.overflowing_mul(b.value).0;

                    let simplified_operation = 
                        match a.operation.opcode.name.starts_with("PUSH") 
                        &&    b.operation.opcode.name.starts_with("PUSH") {
                            true => {
                                WrappedOpcode::new(
                                    0x7f,
                                    vec![ WrappedInput::Raw(result), ],
                                )
                            },
                            false => operation.clone()
                        };

                    self.stack.push(
                        result.encode_hex().as_str(),
                        simplified_operation
                    );
                }

                // SUB
                if op == 0x03 {
                    let a = self.stack.pop();
                    let b = self.stack.pop();

                    let result = a.value.overflowing_sub(b.value).0;

                    let simplified_operation = 
                        match a.operation.opcode.name.starts_with("PUSH") 
                        &&    b.operation.opcode.name.starts_with("PUSH") {
                            true => {
                                WrappedOpcode::new(
                                    0x7f,
                                    vec![ WrappedInput::Raw(result), ],
                                )
                            },
                            false => operation.clone()
                        };

                    self.stack.push(
                        result.encode_hex().as_str(),
                        simplified_operation
                    );
                }

                // DIV
                if op == 0x04 {
                    let numerator = self.stack.pop();
                    let denominator = self.stack.pop();

                    let mut result = U256::zero();
                    if !denominator.value.is_zero() {
                        result = numerator.value.div(denominator.value);
                    }

                    let simplified_operation = 
                        match numerator.operation.opcode.name.starts_with("PUSH") 
                        &&    denominator.operation.opcode.name.starts_with("PUSH") {
                            true => {
                                WrappedOpcode::new(
                                    0x7f,
                                    vec![ WrappedInput::Raw(result), ],
                                )
                            },
                            false => operation.clone()
                        };

                    self.stack.push(
                        result.encode_hex().as_str(),
                        simplified_operation
                    );
                }

                // SDIV
                if op == 0x05 {
                    let numerator = self.stack.pop();
                    let denominator = self.stack.pop();

                    let mut result = I256::zero();
                    if !denominator.value.is_zero() {
                        result = sign_uint(numerator.value).div(sign_uint(denominator.value));
                    }

                    let simplified_operation = 
                        match numerator.operation.opcode.name.starts_with("PUSH") 
                        &&    denominator.operation.opcode.name.starts_with("PUSH") {
                            true => {
                                WrappedOpcode::new(
                                    0x7f,
                                    vec![ WrappedInput::Raw(U256::from_str(result.encode_hex().as_str()).unwrap()), ],
                                )
                            },
                            false => operation.clone()
                        };

                    self.stack.push(
                        result.encode_hex().as_str(),
                        simplified_operation
                    );
                }

                // MOD
                if op == 0x06 {
                    let a = self.stack.pop();
                    let modulus = self.stack.pop();

                    let mut result = U256::zero();
                    if !modulus.value.is_zero() {
                        result = a.value.rem(modulus.value);
                    }

                    let simplified_operation = 
                        match a.operation.opcode.name.starts_with("PUSH") 
                        &&    modulus.operation.opcode.name.starts_with("PUSH") {
                            true => {
                                WrappedOpcode::new(
                                    0x7f,
                                    vec![ WrappedInput::Raw(result), ],
                                )
                            },
                            false => operation.clone()
                        };

                        self.stack.push(
                            result.encode_hex().as_str(),
                            simplified_operation
                        );
                }

                // SMOD
                if op == 0x07 {
                    let a = self.stack.pop();
                    let modulus = self.stack.pop();

                    let mut result = I256::zero();
                    if !modulus.value.is_zero() {
                        result = sign_uint(a.value).rem(sign_uint(modulus.value));
                    }

                    let simplified_operation = 
                        match a.operation.opcode.name.starts_with("PUSH") 
                        &&    modulus.operation.opcode.name.starts_with("PUSH") {
                            true => {
                                WrappedOpcode::new(
                                    0x7f,
                                    vec![ WrappedInput::Raw(U256::from_str(result.encode_hex().as_str()).unwrap()), ],
                                )
                            },
                            false => operation.clone()
                        };

                    self.stack.push(
                        result.encode_hex().as_str(),
                        simplified_operation
                    );
                }

                // ADDMOD
                if op == 0x08 {
                    let a = self.stack.pop();
                    let b = self.stack.pop();
                    let modulus = self.stack.pop();

                    let mut result = U256::zero();
                    if !modulus.value.is_zero() {
                        result = a.value.overflowing_add(b.value).0.rem(modulus.value);
                    }

                    let simplified_operation = 
                        match a.operation.opcode.name.starts_with("PUSH") 
                        &&    modulus.operation.opcode.name.starts_with("PUSH") {
                            true => {
                                WrappedOpcode::new(
                                    0x7f,
                                    vec![ WrappedInput::Raw(result), ],
                                )
                            },
                            false => operation.clone()
                        };

                    self.stack.push(
                        result.encode_hex().as_str(),
                        simplified_operation
                    );
                }

                // MULMOD
                if op == 0x09 {
                    let a = self.stack.pop();
                    let b = self.stack.pop();
                    let modulus = self.stack.pop();

                    let mut result = U256::zero();
                    if !modulus.value.is_zero() {
                        result = a.value.overflowing_mul(b.value).0.rem(modulus.value);
                    }

                    let simplified_operation = 
                        match a.operation.opcode.name.starts_with("PUSH") 
                        &&    modulus.operation.opcode.name.starts_with("PUSH") {
                            true => {
                                WrappedOpcode::new(
                                    0x7f,
                                    vec![ WrappedInput::Raw(result), ],
                                )
                            },
                            false => operation.clone()
                        };

                    self.stack.push(
                        result.encode_hex().as_str(),
                        simplified_operation
                    );
                }

                // EXP
                if op == 0x0A {
                    let a = self.stack.pop();
                    let exponent = self.stack.pop();

                    let result = a.value.overflowing_pow(exponent.value).0;

                    let simplified_operation = 
                        match a.operation.opcode.name.starts_with("PUSH") 
                        &&    exponent.operation.opcode.name.starts_with("PUSH") {
                            true => {
                                WrappedOpcode::new(
                                    0x7f,
                                    vec![ WrappedInput::Raw(result), ],
                                )
                            },
                            false => operation.clone()
                        };

                    self.stack.push(
                        result.encode_hex().as_str(),
                        simplified_operation
                    );
                }

                // SIGNEXTEND
                if op == 0x0B {
                    let x = self.stack.pop().value;
                    let b = self.stack.pop().value;

                    let t = x * U256::from(8u32) + U256::from(7u32);
                    let sign_bit = U256::from(1u32) << t;

                    // (b & sign_bit - 1) - (b & sign_bit)
                    let result = (b & (sign_bit.overflowing_sub(U256::from(1u32)).0)).overflowing_sub(b & sign_bit).0;

                    self.stack.push(
                        result.encode_hex().as_str(),
                        operation.clone(),
                    )
                }

                // LT
                if op == 0x10 {
                    let a = self.stack.pop().value;
                    let b = self.stack.pop().value;

                    match a.lt(&b) {
                        true => self.stack.push("0x01", operation.clone()),
                        false => self.stack.push("0x00", operation.clone()),
                    }
                }

                // GT
                if op == 0x11 {
                    let a = self.stack.pop().value;
                    let b = self.stack.pop().value;

                    match a.gt(&b) {
                        true => self.stack.push("0x01", operation.clone()),
                        false => self.stack.push("0x00", operation.clone()),
                    }
                }

                // SLT
                if op == 0x12 {
                    let a = self.stack.pop().value;
                    let b = self.stack.pop().value;

                    match sign_uint(a).lt(&sign_uint(b)) {
                        true => self.stack.push("0x01", operation.clone()),
                        false => self.stack.push("0x00", operation.clone()),
                    }
                }

                // SGT
                if op == 0x13 {
                    let a = self.stack.pop().value;
                    let b = self.stack.pop().value;

                    match sign_uint(a).gt(&sign_uint(b)) {
                        true => self.stack.push("0x01", operation.clone()),
                        false => self.stack.push("0x00", operation.clone()),
                    }
                }

                // EQ
                if op == 0x14 {
                    let a = self.stack.pop().value;
                    let b = self.stack.pop().value;

                    match a.eq(&b) {
                        true => self.stack.push("0x01", operation.clone()),
                        false => self.stack.push("0x00", operation.clone()),
                    }
                }

                // ISZERO
                if op == 0x15 {
                    let a = self.stack.pop().value;

                    match a.eq(&U256::from(0u8)) {
                        true => self.stack.push("0x01", operation.clone()),
                        false => self.stack.push("0x00", operation.clone()),
                    }
                }

                // AND
                if op == 0x16 {
                    let a = self.stack.pop();
                    let b = self.stack.pop();

                    let result = a.value & b.value;

                    let simplified_operation = 
                        match a.operation.opcode.name.starts_with("PUSH") 
                        &&    b.operation.opcode.name.starts_with("PUSH") {
                            true => {
                                WrappedOpcode::new(
                                    0x7f,
                                    vec![ WrappedInput::Raw(result), ],
                                )
                            },
                            false => operation.clone()
                        };

                    self.stack.push(result.encode_hex().as_str(), simplified_operation);
                }

                // OR
                if op == 0x17 {
                    let a = self.stack.pop();
                    let b = self.stack.pop();

                    let result = a.value | b.value;

                    let simplified_operation = 
                        match a.operation.opcode.name.starts_with("PUSH") 
                        &&    b.operation.opcode.name.starts_with("PUSH") {
                            true => {
                                WrappedOpcode::new(
                                    0x7f,
                                    vec![ WrappedInput::Raw(result), ],
                                )
                            },
                            false => operation.clone()
                        };

                    self.stack.push(result.encode_hex().as_str(), simplified_operation);
                }

                // XOR
                if op == 0x18 {
                    let a = self.stack.pop();
                    let b = self.stack.pop();

                    let result = a.value ^ b.value;

                    let simplified_operation = 
                        match a.operation.opcode.name.starts_with("PUSH") 
                        &&    b.operation.opcode.name.starts_with("PUSH") {
                            true => {
                                WrappedOpcode::new(
                                    0x7f,
                                    vec![ WrappedInput::Raw(result), ],
                                )
                            },
                            false => operation.clone()
                        };

                    self.stack.push(result.encode_hex().as_str(), simplified_operation);
                }

                // NOT
                if op == 0x19 {
                    let a = self.stack.pop();

                    let result = !a.value;

                    let simplified_operation = 
                        match a.operation.opcode.name.starts_with("PUSH") {
                            true => {
                                WrappedOpcode::new(
                                    0x7f,
                                    vec![ WrappedInput::Raw(result), ],
                                )
                            },
                            false => operation.clone()
                        };

                    self.stack.push((result).encode_hex().as_str(), simplified_operation);
                }

                // BYTE
                if op == 0x1A {
                    let b = self.stack.pop().value;
                    let a = self.stack.pop().value;

                    match b >= U256::from(32u32) {
                        true => self.stack.push("0x00", operation.clone()),
                        false => {
                            let result = a / (U256::from(256u32).pow(U256::from(31u32) - b)) % U256::from(256u32);

                            self.stack.push(
                                result.encode_hex().as_str(),
                                operation.clone(),
                            );
                        }
                    }
                }

                // SHL
                if op == 0x1B {
                    let a = self.stack.pop();
                    let b = self.stack.pop();

                    let result = b.value.shl(a.value);

                    let simplified_operation = 
                        match a.operation.opcode.name.starts_with("PUSH") 
                        &&    b.operation.opcode.name.starts_with("PUSH") {
                            true => {
                                WrappedOpcode::new(
                                    0x7f,
                                    vec![ WrappedInput::Raw(result), ],
                                )
                            },
                            false => operation.clone()
                        };

                    self.stack.push(
                        result.encode_hex().as_str(),
                        simplified_operation
                    );
                }

                // SHR
                if op == 0x1C {
                    let a = self.stack.pop();
                    let b = self.stack.pop();

                    let mut result = U256::zero();
                    if !b.value.is_zero() {
                        result = b.value.shr(a.value);
                    }

                    let simplified_operation = 
                        match a.operation.opcode.name.starts_with("PUSH") 
                        &&    b.operation.opcode.name.starts_with("PUSH") {
                            true => {
                                WrappedOpcode::new(
                                    0x7f,
                                    vec![ WrappedInput::Raw(result), ],
                                )
                            },
                            false => operation.clone()
                        };

                    self.stack.push(result.encode_hex().as_str(), simplified_operation);
                }

                // SAR
                if op == 0x1D {
                    let a = self.stack.pop();
                    let b = self.stack.pop();

                    let mut result = I256::zero();
                    if !b.value.is_zero() {
                        result = sign_uint(b.value).shr(sign_uint(a.value));
                    }

                    let simplified_operation = 
                        match a.operation.opcode.name.starts_with("PUSH") 
                        &&    b.operation.opcode.name.starts_with("PUSH") {
                            true => {
                                WrappedOpcode::new(
                                    0x7f,
                                    vec![ WrappedInput::Raw(U256::from_str(result.encode_hex().as_str()).unwrap()), ],
                                )
                            },
                            false => operation.clone()
                        };

                    self.stack.push(
                        result.encode_hex().as_str(),
                        simplified_operation
                    );
                }

                // SHA3
                if op == 0x20 {
                    let offset = self.stack.pop().value;
                    let size = self.stack.pop().value;

                    // Safely convert U256 to usize
                    let offset: usize = match offset.try_into() {
                        Ok(x) => x,
                        Err(_) => {
                            self.exit(2, "0x");
                            return Instruction {
                                instruction: last_instruction,
                                opcode: opcode,
                                opcode_details: Some(opcode_details),
                                inputs: inputs,
                                outputs: Vec::new(),
                                input_operations: input_operations,
                                output_operations: Vec::new(),
                            };
                        }
                    };
                    let size: usize = match size.try_into() {
                        Ok(x) => x,
                        Err(_) => {
                            self.exit(2, "0x");
                            return Instruction {
                                instruction: last_instruction,
                                opcode: opcode,
                                opcode_details: Some(opcode_details),
                                inputs: inputs,
                                outputs: Vec::new(),
                                input_operations: input_operations,
                                output_operations: Vec::new(),
                            };
                        }
                    };

                    let data = self.memory.read(offset, size);
                    let result = keccak256(decode_hex(data.as_str()).unwrap());

                    self.stack.push(
                        result.encode_hex().as_str(),
                        operation.clone(),
                    );
                }

                // ADDRESS
                if op == 0x30 {
                    self.stack.push(self.address.as_str(), operation.clone());
                }

                // BALANCE
                if op == 0x31 {
                    self.stack.pop().value;

                    // balance is set to 1 wei because we won't run into div by 0 errors
                    self.stack.push("0x01", operation.clone());
                }

                // ORIGIN
                if op == 0x32 {
                    self.stack.push(self.origin.as_str(), operation.clone());
                }

                // CALLER
                if op == 0x33 {
                    self.stack.push(self.caller.as_str(), operation.clone());
                }

                // CALLVALUE
                if op == 0x34 {
                    self.stack
                        .push(self.value.encode_hex().as_str(), operation.clone());
                }

                // CALLDATALOAD
                if op == 0x35 {
                    let i = self.stack.pop().value;

                    // Safely convert U256 to usize
                    let i: usize = match i.try_into() {
                        Ok(x) => x,
                        Err(_) => {
                            self.exit(2, "0x");
                            return Instruction {
                                instruction: last_instruction,
                                opcode: opcode,
                                opcode_details: Some(opcode_details),
                                inputs: inputs,
                                outputs: Vec::new(),
                                input_operations: input_operations,
                                output_operations: Vec::new(),
                            };
                        }
                    };

                    // panic safety
                    if i + 32 > self.calldata.len() / 2usize {
                        let mut value = String::new();

                        if i <= self.calldata.len() / 2usize {
                            value = self.calldata[(i * 2)..].to_string();
                        }

                        value.push_str(&"00".repeat(32 - value.len() / 2));
                        let result = U256::from_str(&value).unwrap();

                        self.stack.push(
                            result.encode_hex().as_str(),
                            operation.clone(),
                        );
                    } else {
                        let result = U256::from_str(&self.calldata[i * 2..(i + 32) * 2]).unwrap();

                        self.stack.push(
                            result.encode_hex().as_str(),
                            operation.clone(),
                        );
                    }
                }

                // CALLDATASIZE
                if op == 0x36 {
                    let result = U256::from(&self.calldata.len() / 2usize);

                    self.stack.push(
                        result.encode_hex().as_str(),
                        operation.clone(),
                    );
                }

                // CALLDATACOPY
                if op == 0x37 {
                    let dest_offset = self.stack.pop().value;
                    let offset = self.stack.pop().value;
                    let size = self.stack.pop().value;

                    // Safely convert U256 to usize
                    let dest_offset: usize = match dest_offset.try_into() {
                        Ok(x) => x,
                        Err(_) => {
                            self.exit(2, "0x");
                            return Instruction {
                                instruction: last_instruction,
                                opcode: opcode,
                                opcode_details: Some(opcode_details),
                                inputs: inputs,
                                outputs: Vec::new(),
                                input_operations: input_operations,
                                output_operations: Vec::new(),
                            };
                        }
                    };
                    let offset: usize = match offset.try_into() {
                        Ok(x) => x,
                        Err(_) => {
                            self.exit(2, "0x");
                            return Instruction {
                                instruction: last_instruction,
                                opcode: opcode,
                                opcode_details: Some(opcode_details),
                                inputs: inputs,
                                outputs: Vec::new(),
                                input_operations: input_operations,
                                output_operations: Vec::new(),
                            };
                        }
                    };
                    let size: usize = match size.try_into() {
                        Ok(x) => x,
                        Err(_) => {
                            self.exit(2, "0x");
                            return Instruction {
                                instruction: last_instruction,
                                opcode: opcode,
                                opcode_details: Some(opcode_details),
                                inputs: inputs,
                                outputs: Vec::new(),
                                input_operations: input_operations,
                                output_operations: Vec::new(),
                            };
                        }
                    };

                    let value_offset_safe = if (offset + size) * 2 > self.calldata.len() {
                        self.calldata.len()
                    } else {
                        (offset + size) * 2
                    };

                    let mut value = match self.calldata.get(offset * 2..value_offset_safe) {
                        Some(x) => x.to_owned(),
                        None => "".to_string(),
                    };

                    if value.len() < size * 2 {
                        value.push_str(&"00".repeat(size - (value.len() / 2)));
                    }

                    self.memory.store(dest_offset, size, value)
                }

                // CODESIZE
                if op == 0x38 {
                    let result = U256::from(&self.bytecode.len() / 2usize) - U256::from(1);

                    self.stack.push(
                        result.encode_hex().as_str(),
                        operation.clone(),
                    );
                }

                // CODECOPY
                if op == 0x39 {
                    let dest_offset = self.stack.pop().value;
                    let offset = self.stack.pop().value;
                    let size = self.stack.pop().value;

                    // Safely convert U256 to usize
                    let dest_offset: usize = match dest_offset.try_into() {
                        Ok(x) => x,
                        Err(_) => {
                            self.exit(2, "0x");
                            return Instruction {
                                instruction: last_instruction,
                                opcode: opcode,
                                opcode_details: Some(opcode_details),
                                inputs: inputs,
                                outputs: Vec::new(),
                                input_operations: input_operations,
                                output_operations: Vec::new(),
                            };
                        }
                    };
                    let offset: usize = match offset.try_into() {
                        Ok(x) => x,
                        Err(_) => {
                            self.exit(2, "0x");
                            return Instruction {
                                instruction: last_instruction,
                                opcode: opcode,
                                opcode_details: Some(opcode_details),
                                inputs: inputs,
                                outputs: Vec::new(),
                                input_operations: input_operations,
                                output_operations: Vec::new(),
                            };
                        }
                    };
                    let size: usize = match size.try_into() {
                        Ok(x) => x,
                        Err(_) => {
                            self.exit(2, "0x");
                            return Instruction {
                                instruction: last_instruction,
                                opcode: opcode,
                                opcode_details: Some(opcode_details),
                                inputs: inputs,
                                outputs: Vec::new(),
                                input_operations: input_operations,
                                output_operations: Vec::new(),
                            };
                        }
                    };

                    let value_offset_safe = if (offset + size) * 2 > self.calldata.len() {
                        self.calldata.len()
                    } else {
                        (offset + size) * 2
                    };

                    let mut value = match self.bytecode.get(offset * 2..value_offset_safe) {
                        Some(x) => x.to_owned(),
                        None => "".to_string(),
                    };

                    if value.len() < size * 2 {
                        value.push_str(&"00".repeat(size - (value.len() / 2)));
                    }

                    self.memory.store(dest_offset, size, value)
                }

                // GASPRICE
                if op == 0x3A {
                    self.stack.push("0x01", operation.clone());
                }

                // EXTCODESIZE
                if op == 0x3B {
                    self.stack.pop().value;
                    self.stack.push("0x01", operation.clone());
                }

                // EXTCODECOPY
                if op == 0x3C {
                    self.stack.pop().value;
                    let dest_offset = self.stack.pop().value;
                    self.stack.pop().value;
                    let size = self.stack.pop().value;

                    // Safely convert U256 to usize
                    let dest_offset: usize = match dest_offset.try_into() {
                        Ok(x) => x,
                        Err(_) => {
                            self.exit(2, "0x");
                            return Instruction {
                                instruction: last_instruction,
                                opcode: opcode,
                                opcode_details: Some(opcode_details),
                                inputs: inputs,
                                outputs: Vec::new(),
                                input_operations: input_operations,
                                output_operations: Vec::new(),
                            };
                        }
                    };
                    let size: usize = match size.try_into() {
                        Ok(x) => x,
                        Err(_) => {
                            self.exit(2, "0x");
                            return Instruction {
                                instruction: last_instruction,
                                opcode: opcode,
                                opcode_details: Some(opcode_details),
                                inputs: inputs,
                                outputs: Vec::new(),
                                input_operations: input_operations,
                                output_operations: Vec::new(),
                            };
                        }
                    };

                    self.memory.store(dest_offset, size, "FF".repeat(size / 2))
                }

                // RETURNDATASIZE
                if op == 0x3D {
                    self.stack.push("0x00", operation.clone());
                }

                // RETURNDATACOPY
                if op == 0x3E {
                    let dest_offset = self.stack.pop().value;
                    self.stack.pop().value;
                    let size = self.stack.pop().value;

                    // Safely convert U256 to usize
                    let dest_offset: usize = match dest_offset.try_into() {
                        Ok(x) => x,
                        Err(_) => {
                            self.exit(2, "0x");
                            return Instruction {
                                instruction: last_instruction,
                                opcode: opcode,
                                opcode_details: Some(opcode_details),
                                inputs: inputs,
                                outputs: Vec::new(),
                                input_operations: input_operations,
                                output_operations: Vec::new(),
                            };
                        }
                    };
                    let size: usize = match size.try_into() {
                        Ok(x) => x,
                        Err(_) => {
                            self.exit(2, "0x");
                            return Instruction {
                                instruction: last_instruction,
                                opcode: opcode,
                                opcode_details: Some(opcode_details),
                                inputs: inputs,
                                outputs: Vec::new(),
                                input_operations: input_operations,
                                output_operations: Vec::new(),
                            };
                        }
                    };

                    self.memory.store(dest_offset, size, "FF".repeat(size / 2))
                }

                // EXTCODEHASH and BLOCKHASH
                if op == 0x3F || op == 0x40 {
                    self.stack.pop().value;

                    self.stack.push("0x00", operation.clone());
                }

                // COINBASE
                if op == 0x41 {
                    self.stack.push(
                        "0x6865696d64616c6c00000000636f696e62617365",
                        operation.clone(),
                    );
                }

                // TIMESTAMP
                if op == 0x42 {
                    let timestamp = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_secs();

                    self.stack.push(
                        U256::from(timestamp).encode_hex().as_str(),
                        operation.clone(),
                    );
                }

                // NUMBER -> BASEFEE
                if op >= 0x43 && op <= 0x48 {
                    self.stack.push("0x01", operation.clone());
                }

                // POP
                if op == 0x50 {
                    self.stack.pop().value;
                }

                // MLOAD
                if op == 0x51 {
                    let i = self.stack.pop().value;

                    // Safely convert U256 to usize
                    let i: usize = match i.try_into() {
                        Ok(x) => x,
                        Err(_) => {
                            self.exit(2, "0x");
                            return Instruction {
                                instruction: last_instruction,
                                opcode: opcode,
                                opcode_details: Some(opcode_details),
                                inputs: inputs,
                                outputs: Vec::new(),
                                input_operations: input_operations,
                                output_operations: Vec::new(),
                            };
                        }
                    };

                    let result = U256::from_str(self.memory.read(i, 32).as_str()).unwrap();

                    self.stack.push(
                        result.encode_hex().as_str(),
                        operation.clone(),
                    );
                }

                // MSTORE
                if op == 0x52 {
                    let offset = self.stack.pop().value;
                    let value = self.stack.pop().value.encode_hex().replace("0x", "");

                    // Safely convert U256 to usize
                    let offset: usize = match offset.try_into() {
                        Ok(x) => x,
                        Err(_) => {
                            self.exit(2, "0x");
                            return Instruction {
                                instruction: last_instruction,
                                opcode: opcode,
                                opcode_details: Some(opcode_details),
                                inputs: inputs,
                                outputs: Vec::new(),
                                input_operations: input_operations,
                                output_operations: Vec::new(),
                            };
                        }
                    };

                    self.memory.store(offset, 32, value);
                }

                // MSTORE8
                if op == 0x53 {
                    let offset = self.stack.pop().value;
                    let value = self.stack.pop().value.encode_hex().replace("0x", "");

                    // Safely convert U256 to usize
                    let offset: usize = match offset.try_into() {
                        Ok(x) => x,
                        Err(_) => {
                            self.exit(2, "0x");
                            return Instruction {
                                instruction: last_instruction,
                                opcode: opcode,
                                opcode_details: Some(opcode_details),
                                inputs: inputs,
                                outputs: Vec::new(),
                                input_operations: input_operations,
                                output_operations: Vec::new(),
                            };
                        }
                    };

                    self.memory.store(offset, 1, value);
                }

                // SLOAD
                if op == 0x54 {
                    let key = self.stack.pop().value.encode_hex().replace("0x", "");

                    self.stack.push(
                        &self.storage.load(key),
                        operation.clone()
                    )
                }

                // SSTORE
                if op == 0x55 {
                    let key = self.stack.pop().value.encode_hex().replace("0x", "");
                    let value = self.stack.pop().value.encode_hex().replace("0x", "");

                    self.storage.store(key, value);
                }

                // JUMP
                if op == 0x56 {
                    let pc = self.stack.pop().value;

                    // Safely convert U256 to u128
                    let pc: u128 = match pc.try_into() {
                        Ok(x) => x,
                        Err(_) => {
                            self.exit(2, "0x");
                            return Instruction {
                                instruction: last_instruction,
                                opcode: opcode,
                                opcode_details: Some(opcode_details),
                                inputs: inputs,
                                outputs: Vec::new(),
                                input_operations: input_operations,
                                output_operations: Vec::new(),
                            };
                        }
                    };

                    // Check if JUMPDEST is valid and throw with 790 if not (invalid jump destination)
                    if (((pc + 1) * 2 + 2) as usize <= self.bytecode.len()) &&
                       (self.bytecode[((pc + 1) * 2) as usize..((pc + 1) * 2 + 2) as usize].to_string() != "5b")
                    {
                        self.exit(790, "0x");
                        return Instruction {
                            instruction: last_instruction,
                            opcode: opcode,
                            opcode_details: Some(opcode_details),
                            inputs: inputs,
                            outputs: Vec::new(),
                            input_operations: input_operations,
                            output_operations: Vec::new(),
                        };
                    } else {
                        self.instruction = pc + 1;
                    }
                }

                // JUMPI
                if op == 0x57 {
                    let pc = self.stack.pop().value;
                    let condition = self.stack.pop().value;

                    // Safely convert U256 to u128
                    let pc: u128 = match pc.try_into() {
                        Ok(x) => x,
                        Err(_) => {
                            self.exit(2, "0x");
                            return Instruction {
                                instruction: last_instruction,
                                opcode: opcode,
                                opcode_details: Some(opcode_details),
                                inputs: inputs,
                                outputs: Vec::new(),
                                input_operations: input_operations,
                                output_operations: Vec::new(),
                            };
                        }
                    };

                    if !condition.eq(&U256::from(0u8)) {
                        
                        // Check if JUMPDEST is valid and throw with 790 if not (invalid jump destination)
                        if (((pc + 1) * 2 + 2) as usize <= self.bytecode.len()) &&
                           (self.bytecode[((pc + 1) * 2) as usize..((pc + 1) * 2 + 2) as usize].to_string() != "5b")
                        {
                            self.exit(790, "0x");
                            return Instruction {
                                instruction: last_instruction,
                                opcode: opcode,
                                opcode_details: Some(opcode_details),
                                inputs: inputs,
                                outputs: Vec::new(),
                                input_operations: input_operations,
                                output_operations: Vec::new(),
                            };
                        } else {
                            self.instruction = pc + 1;
                        }
                    }
                }

                // PC
                if op == 0x58 {
                    self.stack.push(
                        U256::from(self.instruction).encode_hex().as_str(),
                        operation.clone(),
                    );
                }

                // MSIZE
                if op == 0x59 {
                    self.stack.push(
                        U256::from(self.memory.size()).encode_hex().as_str(),
                        operation.clone(),
                    );
                }

                // GAS
                if op == 0x5a {
                    self.stack.push(
                        U256::from(self.gas_remaining).encode_hex().as_str(),
                        operation.clone(),
                    );
                }

                // PUSH1 -> PUSH32
                if op >= 0x60 && op <= 0x7F {
                    // Get the number of bytes to push
                    let num_bytes = (op - 95) as u128;

                    // Get the bytes to push from bytecode
                    let bytes = &self.bytecode[(self.instruction * 2) as usize
                        ..((self.instruction + num_bytes) * 2) as usize];
                    self.instruction += num_bytes;

                    // update the operation's inputs
                    let new_operation_inputs =
                        vec![WrappedInput::Raw(U256::from_str(bytes).unwrap())];

                    operation.inputs = new_operation_inputs;

                    // Push the bytes to the stack
                    self.stack.push(bytes, operation.clone());
                }

                // DUP1 -> DUP16
                if op >= 0x80 && op <= 0x8F {
                    // Get the number of items to swap
                    let index = (op - 127) as usize;

                    // Perform the swap
                    self.stack.dup(index);
                }

                // SWAP1 -> SWAP16
                if op >= 0x90 && op <= 0x9F {
                    // Get the number of items to swap
                    let index = (op - 143) as usize;

                    // Perform the swap
                    self.stack.swap(index);
                }

                // LOG0 -> LOG4
                if op >= 0xA0 && op <= 0xA4 {
                    let topic_count = (op - 160) as usize;
                    let offset = self.stack.pop().value;
                    let size = self.stack.pop().value;
                    let topics = self
                        .stack
                        .pop_n(topic_count)
                        .iter()
                        .map(|x| x.value)
                        .collect();

                    // Safely convert U256 to usize
                    let offset: usize = match offset.try_into() {
                        Ok(x) => x,
                        Err(_) => {
                            self.exit(2, "0x");
                            return Instruction {
                                instruction: last_instruction,
                                opcode: opcode,
                                opcode_details: Some(opcode_details),
                                inputs: inputs,
                                outputs: Vec::new(),
                                input_operations: input_operations,
                                output_operations: Vec::new(),
                            };
                        }
                    };
                    let size: usize = match size.try_into() {
                        Ok(x) => x,
                        Err(_) => {
                            self.exit(2, "0x");
                            return Instruction {
                                instruction: last_instruction,
                                opcode: opcode,
                                opcode_details: Some(opcode_details),
                                inputs: inputs,
                                outputs: Vec::new(),
                                input_operations: input_operations,
                                output_operations: Vec::new(),
                            };
                        }
                    };

                    let data = self.memory.read(offset, size);

                    // no need for a panic check because the length of events should never be larger than a u128
                    self.events.push(Log::new(
                        (self.events.len() as usize).try_into().unwrap(),
                        topics,
                        data,
                    ))
                }

                // CREATE
                if op == 0xF0 {
                    self.stack.pop_n(3);

                    self.stack.push(
                        "0x6865696d64616c6c000000000000637265617465",
                        operation.clone(),
                    );
                }

                // CALL, CALLCODE
                if op == 0xF1 || op == 0xF2 {
                    self.stack.pop_n(7);

                    self.stack.push("0x01", operation.clone());
                }

                // RETURN
                if op == 0xF3 {
                    let offset = self.stack.pop().value;
                    let size = self.stack.pop().value;

                    // Safely convert U256 to usize
                    let offset: usize = match offset.try_into() {
                        Ok(x) => x,
                        Err(_) => {
                            self.exit(2, "0x");
                            return Instruction {
                                instruction: last_instruction,
                                opcode: opcode,
                                opcode_details: Some(opcode_details),
                                inputs: inputs,
                                outputs: Vec::new(),
                                input_operations: input_operations,
                                output_operations: Vec::new(),
                            };
                        }
                    };
                    let size: usize = match size.try_into() {
                        Ok(x) => x,
                        Err(_) => {
                            self.exit(2, "0x");
                            return Instruction {
                                instruction: last_instruction,
                                opcode: opcode,
                                opcode_details: Some(opcode_details),
                                inputs: inputs,
                                outputs: Vec::new(),
                                input_operations: input_operations,
                                output_operations: Vec::new(),
                            };
                        }
                    };

                    self.exit(0, self.memory.read(offset, size).as_str());
                }

                // DELEGATECALL, STATICCALL
                if op == 0xF4 || op == 0xFA {
                    self.stack.pop_n(6);

                    self.stack.push("0x01", operation.clone());
                }

                // CREATE2
                if op == 0xF5 {
                    self.stack.pop_n(4);

                    self.stack.push(
                        "0x6865696d64616c6c000000000063726561746532",
                        operation.clone(),
                    );
                }

                // REVERT
                if op == 0xFD {
                    let offset = self.stack.pop().value;
                    let size = self.stack.pop().value;

                    // Safely convert U256 to usize
                    let offset: usize = match offset.try_into() {
                        Ok(x) => x,
                        Err(_) => {
                            self.exit(2, "0x");
                            return Instruction {
                                instruction: last_instruction,
                                opcode: opcode,
                                opcode_details: Some(opcode_details),
                                inputs: inputs,
                                outputs: Vec::new(),
                                input_operations: input_operations,
                                output_operations: Vec::new(),
                            };
                        }
                    };
                    let size: usize = match size.try_into() {
                        Ok(x) => x,
                        Err(_) => {
                            self.exit(2, "0x");
                            return Instruction {
                                instruction: last_instruction,
                                opcode: opcode,
                                opcode_details: Some(opcode_details),
                                inputs: inputs,
                                outputs: Vec::new(),
                                input_operations: input_operations,
                                output_operations: Vec::new(),
                            };
                        }
                    };

                    self.exit(1, self.memory.read(offset, size).as_str());
                }

                // INVALID & SELFDESTRUCT
                if op >= 0xFE {
                    self.consume_gas(self.gas_remaining);
                    self.exit(1, "0x");
                }

                // get outputs
                let output_frames = self.stack.peek_n(opcode_details.outputs as usize);
                let output_operations = output_frames
                    .iter()
                    .map(|x| x.operation.clone())
                    .collect::<Vec<WrappedOpcode>>();
                let outputs = output_frames.iter().map(|x| x.value).collect::<Vec<U256>>();

                return Instruction {
                    instruction: last_instruction,
                    opcode: opcode,
                    opcode_details: Some(opcode_details),
                    inputs: inputs,
                    outputs: outputs,
                    input_operations: input_operations,
                    output_operations: output_operations,
                };
            }
            _ => {
                
                // we reached an INVALID opcode, consume all remaining gas
                self.exit(4, "0x");
                return Instruction {
                    instruction: last_instruction,
                    opcode: "unknown".to_string(),
                    opcode_details: Some(opcode_details),
                    inputs: inputs,
                    outputs: Vec::new(),
                    input_operations: input_operations,
                    output_operations: Vec::new(),
                };
            }
        }
    }

    // Executes the next instruction in the VM and returns a snapshot its the state
    pub fn step(&mut self) -> State {
        let instruction = self._step();

        //println!("{}({:?})", instruction.clone().opcode_details.unwrap().name, instruction.inputs);

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

    // Resets the VM state for a new execution
    pub fn reset(&mut self) {
        self.stack = Stack::new();
        self.memory = Memory::new();
        self.instruction = 1;
        self.gas_remaining = u128::max_value();
        self.gas_used = 21000;
        self.events = Vec::new();
        self.returndata = String::new();
        self.exitcode = 255;
        self.timestamp = Instant::now();
    }

    // Executes the code until finished
    pub fn execute(&mut self) -> Result {
        while self.bytecode.len() >= (self.instruction * 2 + 2) as usize {
            self.step();

            if self.exitcode != 255 || self.returndata.len() as usize > 0 {
                break;
            }
        }

        return Result {
            gas_used: self.gas_used,
            gas_remaining: self.gas_remaining,
            returndata: self.returndata.to_owned(),
            exitcode: self.exitcode,
            events: self.events.clone(),
            runtime: self.timestamp.elapsed().as_secs_f64(),
            instruction: self.instruction,
        };
    }

    // Executes provided calldata until finished
    pub fn call(&mut self, calldata: String, value: u128) -> Result {
        // reset the VM temp state
        self.reset();
        self.calldata = calldata.replace("0x", "");
        self.value = value;

        return self.execute();
    }
}
