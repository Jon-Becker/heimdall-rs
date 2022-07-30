use std::{
    str::FromStr,
    ops::{
        Div,
        Rem,
        Shl,
        Shr
    }, 
    time::{
        UNIX_EPOCH,
        SystemTime,
        Instant
    }
};

use ethers::{
    prelude::U256,
    abi::AbiEncode,
    utils::{
        keccak256,
    }
};

use crate::{
    utils::{
        strings::{
            sign_uint,
            decode_hex
        },
    },
    ether::evm::opcodes::Opcode
};

use super::{
    stack::Stack,
    memory::Memory,
    storage::Storage,
    log::Log,
};

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
        mut gas_limit: u128) -> VM {
        if gas_limit < 21000 { gas_limit = 21000; }

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

        return
    }

    pub fn consume_gas(&mut self, amount: u128) -> bool {

        // REVERT if out of gas
        // TODO: make this call the REVERT instruction
        if amount > self.gas_remaining { return false; }

        self.gas_remaining = self.gas_remaining.saturating_sub(amount);
        self.gas_used = self.gas_used.saturating_add(amount);
        return true
    }

    // Steps to the next PC and executes the instruction
    fn _step(&mut self) -> Instruction {

        // sanity check
        if self.bytecode.len() < (self.instruction*2+2) as usize {
            self.exit(2, "0x");
            Instruction {
                instruction: self.instruction,
                opcode: "PANIC".to_string(),
                opcode_details: None,
                inputs: Vec::new(),
                outputs: Vec::new(),
            };
        }

        // get the opcode at the current instruction
        let opcode = self.bytecode[(self.instruction*2) as usize..(self.instruction*2+2) as usize].to_string();
        let last_instruction = self.instruction;
        self.instruction += 1;

        // add the opcode to the trace
        let opcode_details = crate::ether::evm::opcodes::opcode(opcode.replace("0x", "").as_str());
        let inputs = self.stack.peek_n(opcode_details.inputs as usize);

        // Consume the minimum gas for the opcode
        let gas_cost = opcode_details.mingas;
        match self.consume_gas(gas_cost.into()) {
            true => {},
            false => {
                return Instruction {
                    instruction: last_instruction,
                    opcode: opcode,
                    opcode_details: Some(opcode_details),
                    inputs: inputs,
                    outputs: Vec::new(),
                };
            }
        }


        match U256::from_str(&opcode) {

            Ok(_opcode) => {
                let op = _opcode.as_usize();

                // STOP
                if op == 0 {
                    self.exit(0, "0x");
                    return Instruction {
                        instruction: last_instruction,
                        opcode: opcode,
                        opcode_details: Some(opcode_details),
                        inputs: inputs,
                        outputs: Vec::new(),
                    };
                }


                // ADD
                if op == 1 {
                    let a = self.stack.pop();
                    let b = self.stack.pop();

                    self.stack.push(a.overflowing_add(b).0.encode_hex().as_str());
                }


                // MUL
                if op == 2 {
                    let a = self.stack.pop();
                    let b = self.stack.pop();

                    self.stack.push(a.overflowing_mul(b).0.encode_hex().as_str());
                }


                // SUB
                if op == 3 {
                    let a = self.stack.pop();
                    let b = self.stack.pop();

                    self.stack.push(a.overflowing_sub(b).0.encode_hex().as_str());
                }


                // DIV
                if op == 4 {
                    let numerator = self.stack.pop();
                    let denominator = self.stack.pop();

                    if denominator == U256::from(0) {
                        self.stack.push(U256::from(0).encode_hex().as_str());
                    }
                    else {
                        self.stack.push(numerator.div(denominator).encode_hex().as_str());
                    }
                }


                // SDIV
                if op == 5 {
                    let numerator = self.stack.pop();
                    let denominator = self.stack.pop();

                    if denominator == U256::from(0) {
                        self.stack.push(U256::from(0).encode_hex().as_str());
                    }
                    else {
                        self.stack.push(sign_uint(numerator).div(sign_uint(denominator)).encode_hex().as_str());
                    }
                }


                // MOD
                if op == 6 {
                    let a = self.stack.pop();
                    let modulus = self.stack.pop();

                    self.stack.push(a.rem(modulus).encode_hex().as_str());
                }


                // SMOD
                if op == 7 {
                    let a = self.stack.pop();
                    let modulus = self.stack.pop();

                    self.stack.push(sign_uint(a).rem(sign_uint(modulus)).encode_hex().as_str());
                }


                // ADDMOD
                if op == 8 {
                    let a = self.stack.pop();
                    let b = self.stack.pop();
                    let modulus = self.stack.pop();

                    self.stack.push(a.overflowing_add(b).0.rem(modulus).encode_hex().as_str());
                }


                // MULMOD
                if op == 9 {
                    let a = self.stack.pop();
                    let b = self.stack.pop();
                    let modulus = self.stack.pop();

                    self.stack.push(a.overflowing_mul(b).0.rem(modulus).encode_hex().as_str());
                }


                // EXP
                if op == 10 {
                    let a = self.stack.pop();
                    let exponent = self.stack.pop();

                    self.stack.push(a.overflowing_pow(exponent).0.encode_hex().as_str());
                }


                // SIGNEXTEND
                if op == 11 {
                    let x = self.stack.pop();
                    let b = self.stack.pop();

                    let t = x * U256::from(8u32) + U256::from(7u32);
                    let sign_bit = U256::from(1u32) << t;

                    // (b & sign_bit - 1) - (b & sign_bit)
                    self.stack.push(((b & (sign_bit
                        .overflowing_sub(U256::from(1u32)).0))
                        .overflowing_sub(b & sign_bit).0).encode_hex().as_str()
                    )
                }

                
                // LT
                if op == 16 {
                    let a = self.stack.pop();
                    let b = self.stack.pop();

                    match a.lt(&b) {
                        true => self.stack.push("0x01"),
                        false => self.stack.push("0x00"),
                    }
                }


                // GT
                if op == 17 {
                    let a = self.stack.pop();
                    let b = self.stack.pop();

                    match a.gt(&b) {
                        true => self.stack.push("0x01"),
                        false => self.stack.push("0x00"),
                    }
                }


                // SLT
                if op == 18 {
                    let a = self.stack.pop();
                    let b = self.stack.pop();

                    match sign_uint(a).lt(&sign_uint(b)) {
                        true => self.stack.push("0x01"),
                        false => self.stack.push("0x00"),
                    }
                }


                // SGT
                if op == 19 {
                    let a = self.stack.pop();
                    let b = self.stack.pop();

                    match sign_uint(a).gt(&sign_uint(b)) {
                        true => self.stack.push("0x01"),
                        false => self.stack.push("0x00"),
                    }
                }


                // EQ
                if op == 20 {
                    let a = self.stack.pop();
                    let b = self.stack.pop();

                    match a.eq(&b) {
                        true => self.stack.push("0x01"),
                        false => self.stack.push("0x00"),
                    }
                }
                
                
                // ISZERO
                if op == 21 {
                    let a = self.stack.pop();

                    match a.eq(&U256::from(0u8)) {
                        true => self.stack.push("0x01"),
                        false => self.stack.push("0x00"),
                    }
                }


                // AND
                if op == 22 {
                    let a = self.stack.pop();
                    let b = self.stack.pop();

                    self.stack.push((a & b).encode_hex().as_str());
                }


                // OR
                if op == 23 {
                    let a = self.stack.pop();
                    let b = self.stack.pop();

                    self.stack.push((a | b).encode_hex().as_str());
                }

                // XOR
                if op == 24 {
                    let a = self.stack.pop();
                    let b = self.stack.pop();

                    self.stack.push((a ^ b).encode_hex().as_str());
                }


                // NOT
                if op == 25 {
                    let a = self.stack.pop();

                    self.stack.push((!a).encode_hex().as_str());
                }


                // BYTE
                if op == 26 {
                    let b = self.stack.pop();
                    let a = self.stack.pop();

                    match b >= U256::from(32u32) {
                        true => self.stack.push("0x00"),
                        false => {
                            self.stack.push((
                                (a / ( U256::from(256u32).pow(U256::from(31u32) - b) )) % U256::from(256u32)
                            ).encode_hex().as_str());
                        },
                    }
                }

                
                // SHL
                if op == 27 {
                    let a = self.stack.pop();
                    let b = self.stack.pop();

                    self.stack.push(b.shl(a).encode_hex().as_str());   
                }


                // SHR
                if op == 28 {
                    let a = self.stack.pop();
                    let b = self.stack.pop();

                    self.stack.push(b.shr(a).encode_hex().as_str());   
                }


                // SAR
                if op == 29 {
                    let a = self.stack.pop();
                    let b = self.stack.pop();

                    self.stack.push(sign_uint(b).shr(sign_uint(a)).encode_hex().as_str());
                }

                
                // SHA3
                if op == 32 {
                    let offset = self.stack.pop();
                    let size = self.stack.pop();

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
                            };
                        },
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
                            };
                        },
                    };

                    let data = self.memory.read(offset, size);
                    self.stack.push(keccak256(decode_hex(data.as_str()).unwrap()).encode_hex().as_str());
                }


                // ADDRESS
                if op == 48 {
                    self.stack.push(self.address.as_str());
                }

                
                // BALANCE
                if op == 49 {
                    self.stack.pop();

                    // balance is set to 1 wei because we won't run into div by 0 errors
                    self.stack.push("0x01");
                }


                // ORIGIN
                if op == 50 {
                    self.stack.push(self.origin.as_str());
                }


                // CALLER
                if op == 51 {
                    self.stack.push(self.caller.as_str());
                }


                // CALLVALUE
                if op == 52 {
                    self.stack.push(self.value.encode_hex().as_str());
                }


                // CALLDATALOAD
                if op == 53 {
                    let i = self.stack.pop();

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
                            };
                        },
                    };

                    // panic safety
                    if i + 32 > self.calldata.len() / 2usize {
                        let mut value = String::new();
            
                        if i <= self.calldata.len() / 2usize {
                            value = self.calldata[(i*2)..].to_string();
                        }
                        
                        value.push_str(&"00".repeat(32 - value.len() / 2));
                        self.stack.push(U256::from_str(&value).unwrap().encode_hex().as_str());
                    }
                    else {
                        self.stack.push(U256::from_str(&self.calldata[ i*2 .. (i + 32)*2 ]).unwrap().encode_hex().as_str());
                    }
                }


                // CALLDATASIZE
                if op == 54 {
                    self.stack.push(U256::from(&self.calldata.len() / 2usize).encode_hex().as_str());
                }


                // CALLDATACOPY
                if op == 55 {
                    let dest_offset = self.stack.pop();
                    let offset = self.stack.pop();
                    let size = self.stack.pop();

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
                            };
                        },
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
                            };
                        },
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
                            };
                        },
                    };

                    let value_offset_safe = 
                        if (offset + size)*2 > self.calldata.len() {
                            self.calldata.len()
                        }
                        else {
                            (offset + size)*2
                        };

                    let mut value = match self.calldata.get(offset*2 .. value_offset_safe) {
                        Some(x) => x.to_owned(),
                        None => "".to_string(),
                    };

                    if value.len() < size * 2 {
                        value.push_str(&"00".repeat(size - (value.len() / 2) ));
                    }

                    self.memory.store(dest_offset, size, value)
                }


                // CODESIZE
                if op == 56 {
                    self.stack.push(U256::from(&self.bytecode.len() / 2usize).encode_hex().as_str());
                }


                // CODECOPY
                if op == 57 {
                    let dest_offset = self.stack.pop();
                    let offset = self.stack.pop();
                    let size = self.stack.pop();

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
                            };
                        },
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
                            };
                        },
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
                            };
                        },
                    };

                    let value_offset_safe = 
                        if (offset + size)*2 > self.calldata.len() {
                            self.calldata.len()
                        }
                        else {
                            (offset + size)*2
                        };

                    let mut value = match self.bytecode.get(offset*2 .. value_offset_safe) {
                        Some(x) => x.to_owned(),
                        None => "".to_string(),
                    };

                    if value.len() < size * 2 {
                        value.push_str(&"00".repeat(size - (value.len() / 2) ));
                    }
                    
                    self.memory.store(dest_offset, size, value)
                }


                // GASPRICE
                if op == 58 {
                    self.stack.push("0x01");
                }

                // EXTCODESIZE 
                if op == 59 {
                    self.stack.pop();
                    self.stack.push("0x01");
                }

                // EXTCODECOPY
                if op == 60 {
                    self.stack.pop();
                    let dest_offset = self.stack.pop();
                    self.stack.pop();
                    let size = self.stack.pop();

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
                            };
                        },
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
                            };
                        },
                    };

                    self.memory.store(dest_offset, size, "FF".repeat(size / 2))
                }


                // RETURNDATASIZE
                if op == 61 {
                    self.stack.pop();

                    self.stack.push("0x00");
                }


                // RETURNDATACOPY
                if op == 62 {
                    let dest_offset = self.stack.pop();
                    self.stack.pop();
                    let size = self.stack.pop();

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
                            };
                        },
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
                            };
                        },
                    };

                    self.memory.store(dest_offset, size, "FF".repeat(size / 2))
                }


                // EXTCODEHASH and BLOCKHASH
                if op == 63 || op == 64{
                    self.stack.pop();

                    self.stack.push("0x00");
                }


                // COINBASE
                if op == 65 {
                    self.stack.push("0x6865696d64616c6c00000000636f696e62617365");
                }


                // TIMESTAMP
                if op == 66 {
                    let timestamp = SystemTime::now()
                                    .duration_since(UNIX_EPOCH)
                                    .unwrap().as_secs();

                    self.stack.push(U256::from(timestamp).encode_hex().as_str());
                }


                // NUMBER -> BASEFEE
                if op >= 67 && op <= 72 {
                    self.stack.push("0x01");
                }


                // POP
                if op == 80 {
                    self.stack.pop();
                }


                // MLOAD
                if op == 81 {
                    let i = self.stack.pop();

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
                            };
                        },
                    };

                    self.stack.push(U256::from_str(self.memory.read(i, 32).as_str()).unwrap().encode_hex().as_str());
                }


                // MSTORE
                if op == 82 {
                    let offset = self.stack.pop();
                    let value = self.stack.pop().encode_hex().replace("0x", "");

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
                            };
                        },
                    };

                    self.memory.store(offset, 32, value);
                }


                // MSTORE8
                if op == 83 {
                    let offset = self.stack.pop();
                    let value = self.stack.pop().encode_hex().replace("0x", "");

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
                            };
                        },
                    };

                    self.memory.store(offset, 1, value);
                }


                // SLOAD
                if op == 84 {
                    let key = self.stack.pop().encode_hex().replace("0x", "");

                    self.stack.push(&self.storage.load(key))
                }

                
                // SSTORE
                if op == 85 {
                    let key = self.stack.pop().encode_hex().replace("0x", "");
                    let value = self.stack.pop().encode_hex().replace("0x", "");

                    self.storage.store(key, value);
                }


                // JUMP
                if op == 86 {
                    let pc = self.stack.pop();

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
                            };
                        },
                    };

                    self.instruction = pc + 1;
                }


                // JUMPI 
                if op == 87 {
                    let pc = self.stack.pop();
                    let condition = self.stack.pop();

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
                            };
                        },
                    };

                    if !condition.eq(&U256::from(0u8)) {
                        self.instruction = pc + 1;
                    }
                }


                // PC
                if op == 88 {
                    self.stack.push(U256::from(self.instruction).encode_hex().as_str());
                }


                // MSIZE 
                if op == 89 {
                    self.stack.push(U256::from(self.memory.size()).encode_hex().as_str());
                }


                // GAS
                if op == 90 {
                    self.stack.push(U256::from(self.gas_remaining).encode_hex().as_str());
                }
                

                // PUSH1 -> PUSH32
                if op >= 96 && op <= 127 {

                    // Get the number of bytes to push
                    let num_bytes = (op - 95) as u128;

                    // Get the bytes to push from bytecode
                    let bytes = &self.bytecode[(self.instruction*2) as usize..((self.instruction + num_bytes) * 2) as usize];
                    self.instruction += num_bytes;

                    // Push the bytes to the stack
                    self.stack.push(bytes);

                }


                // DUP1 -> DUP16
                if op >= 128 && op <= 143 {

                    // Get the number of items to swap
                    let index = (op - 127) as usize;
                    
                    // Perform the swap
                    self.stack.dup(index);
                }
                

                // SWAP1 -> SWAP16
                if op >= 144 && op <= 159 {

                    // Get the number of items to swap
                    let index = (op - 143) as usize;
                    
                    // Perform the swap
                    self.stack.swap(index);
                }


                // LOG0 -> LOG4
                if op >= 160 && op <= 164 {
                    let topic_count = (op - 160) as usize;
                    let offset = self.stack.pop();
                    let size = self.stack.pop();
                    let topics = self.stack.pop_n(topic_count);

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
                            };
                        },
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
                            };
                        },
                    };
                    
                    let data = self.memory.read(offset, size);

                    // no need for a panic check because the length of events should never be larger than a u128
                    self.events.push(Log::new((self.events.len() as usize).try_into().unwrap(), topics, data))
                }


                // CREATE
                if op == 240 {
                    self.stack.pop_n(3);

                    self.stack.push("0x6865696d64616c6c000000000000637265617465");
                }


                // CALL, CALLCODE
                if op == 241 || op == 242 {
                    self.stack.pop_n(7);

                    self.stack.push("0x01");
                }

                // RETURN
                if op == 243 {
                    let offset = self.stack.pop();
                    let size = self.stack.pop();

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
                            };
                        },
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
                            };
                        },
                    };

                    self.exit(0, self.memory.read(offset, size).as_str());
                }


                // DELEGATECALL, STATICCALL
                if op == 244 || op == 250 {
                    self.stack.pop_n(6);

                    self.stack.push("0x01");
                }


                // CREATE2
                if op == 245 {
                    self.stack.pop_n(4);

                    self.stack.push("0x6865696d64616c6c000000000063726561746532");
                }


                // REVERT
                if op == 253 {
                    let offset = self.stack.pop();
                    let size = self.stack.pop();

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
                            };
                        },
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
                            };
                        },
                    };

                    self.exit(1, self.memory.read(offset, size).as_str());
                }
                

                // INVALID & SELFDESTRUCT
                if op >= 254 {
                    self.consume_gas(self.gas_remaining);
                    self.exit(1, "0x");
                }

                // get outputs
                let outputs = self.stack.peek_n(opcode_details.outputs as usize);

                return Instruction {
                    instruction: last_instruction,
                    opcode: opcode,
                    opcode_details: Some(opcode_details),
                    inputs: inputs,
                    outputs: outputs,
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
                };
            }
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
            events: self.events.clone()
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
        while self.bytecode.len() >= (self.instruction*2+2) as usize {
            self.step();

            if self.exitcode != 255 || self.returndata.len() as usize > 0 {
                break
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
        }
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