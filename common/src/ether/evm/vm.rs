use std::{str::FromStr, ops::{Div, Rem, Shl, Shr}, time::{UNIX_EPOCH, SystemTime}};

use ethers::{
    prelude::U256,
    abi::AbiEncode,
    utils::{
        keccak256, 
        rlp::Encodable
    }
};

use crate::{
    io::logging::Logger,
    utils::sign_uint
};

use super::{
    stack::Stack,
    memory::Memory,
    storage::Storage
};


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
    pub events: Vec<>,

    pub logger: Logger,
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
        verbosity: &str) -> VM {
        VM {
            stack: Stack::new(),
            memory: Memory::new(),
            storage: Storage::new(),
            instruction: 0, // TODO: increase this to 1
            bytecode: bytecode.replace("0x", ""),
            calldata: calldata.replace("0x", ""),
            address: address.replace("0x", ""),
            origin: origin.replace("0x", ""),
            caller: caller.replace("0x", ""),
            value: value,
            gas_remaining: gas_limit,
            gas_used: 0,
            events: Vec::new(),

            logger: Logger::new(&verbosity),
        }
    }

    pub fn consume_gas(&mut self, amount: u128) {

        // REVERT if out of gas
        // TODO: make this call the REVERT instruction
        if amount > self.gas_remaining {
            self.logger.error("Execution Reverted: Out of gas.");
            std::process::exit(1);
        }

        self.gas_remaining = self.gas_remaining.saturating_sub(amount);
        self.gas_used = self.gas_used.saturating_add(amount);
    }

    pub fn execute(&mut self) {

        // get the opcode at the current instruction
        let opcode = self.bytecode[(self.instruction*2) as usize..(self.instruction*2+2) as usize].to_string();
        self.instruction += 1;

        // Consume the minimum gas for the opcode
        let gas_cost = crate::ether::opcodes::opcode(opcode.replace("0x", "").as_str()).mingas;
        self.consume_gas(gas_cost.into());

        match U256::from_str(&opcode) {

            Ok(opcode) => {
                let op = opcode.as_usize();

                // STOP
                if op == 0 {

                    // TODO: stop execution
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

                    self.stack.push(numerator.div(denominator).encode_hex().as_str());
                }


                // SDIV
                if op == 5 {
                    let numerator = self.stack.pop();
                    let denominator = self.stack.pop();

                    self.stack.push(sign_uint(numerator).div(sign_uint(denominator)).encode_hex().as_str());
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

                    let t = x * U256::from(8 as u32) + U256::from(7 as u32);
                    let sign_bit = U256::from(1 as u32) << t;

                    // (b & sign_bit - 1) - (b & sign_bit)
                    self.stack.push(((b & (sign_bit
                        .overflowing_sub(U256::from(1 as u32)).0))
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

                    match a.eq(&U256::from(0 as u32)) {
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

                    match b >= U256::from(32 as u32) {
                        true => self.stack.push("0x00"),
                        false => {
                            self.stack.push((
                                (a / ( U256::from(256 as u32).pow(U256::from(31 as u32) - b) )) % U256::from(256 as u32)
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

                    let data = self.memory.read(offset.as_usize(), size.as_usize());

                    self.stack.push(keccak256(data).encode_hex().as_str());
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

                    self.stack.push(U256::from_str(&self.calldata[ i.as_usize()*2 .. (i.as_usize() + 32)*2 ]).unwrap().encode_hex().as_str());
                }


                // CALLDATASIZE
                if op == 54 {
                    self.stack.push(U256::from(&self.calldata.len() / 2 as usize).encode_hex().as_str());
                }


                // CALLDATACOPY
                if op == 55 {
                    let dest_offset = self.stack.pop();
                    let offset = self.stack.pop();
                    let size = self.stack.pop();
                    
                    self.memory.store(dest_offset.try_into().unwrap(), size.try_into().unwrap(), self.calldata[ offset.as_usize()*2 .. (offset.as_usize() + size.as_usize())*2 ].to_string())
                }


                // CODESIZE
                if op == 56 {
                    self.stack.push(U256::from(&self.bytecode.len() / 2 as usize).encode_hex().as_str());
                }


                // CODECOPY
                if op == 57 {
                    let dest_offset = self.stack.pop();
                    let offset = self.stack.pop();
                    let size = self.stack.pop();
                    
                    self.memory.store(dest_offset.try_into().unwrap(), size.try_into().unwrap(), self.bytecode[ offset.as_usize()*2 .. (offset.as_usize() + size.as_usize())*2 ].to_string())
                }


                // GASPRICE and EXTCODESIZE
                if op == 58 || op == 59{
                    self.stack.push("0x01");
                }

                // EXTCODECOPY
                if op == 60 {
                    self.stack.pop();
                    let dest_offset = self.stack.pop();
                    self.stack.pop();
                    let size = self.stack.pop();

                    self.memory.store(dest_offset.try_into().unwrap(), size.try_into().unwrap(), "FF".repeat(size.as_usize() / 2))
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

                    self.memory.store(dest_offset.try_into().unwrap(), size.try_into().unwrap(), "FF".repeat(size.as_usize() / 2))
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

                    self.stack.push(self.memory.read(i.as_usize(), 32).encode_hex().as_str());
                }


                // MSTORE
                if op == 82 {
                    let offset = self.stack.pop();
                    let value = self.stack.pop();

                    self.memory.store(offset.as_usize(), 32, value.to_string());
                }


                // MSTORE8
                if op == 83 {
                    let offset = self.stack.pop();
                    let value = self.stack.pop();

                    self.memory.store(offset.as_usize(), 1, value.to_string());
                }


                // SLOAD
                if op == 84 {
                    let key = self.stack.pop();

                    self.stack.push(&self.storage.load(key.to_string()))
                }

                
                // SSTORE
                if op == 85 {
                    let key = self.stack.pop();
                    let value = self.stack.pop();

                    self.storage.store(key.to_string(), value.encode_hex().to_string());
                }


                // JUMP
                if op == 86 {
                    let pc = self.stack.pop();
                    self.instruction = pc.as_u128();
                }


                // JUMPI 
                if op == 87 {
                    let pc = self.stack.pop();
                    let condition = self.stack.pop();

                    if !condition.eq(&U256::from(0 as u8)) {
                        self.instruction = pc.as_u128();
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
                    self.stack.dup(index);
                }

                


            }
            _ => {
                
                // we reached an INVALID opcode, consume all remaining gas
                self.consume_gas(self.gas_remaining);
            }
        }

    }

}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_vm() {

        let mut vm = VM::new(
            String::from("0x600435"),
            String::from("0xffffffff000000000000000000000000000000000000000000000000000000000000000000000000"),
            String::from("0x6865696d64616c6c000000000061646472657373"),
            String::from("0x6865696d64616c6c0000000000006f726967696e"),
            String::from("0x6865696d64616c6c00000000000063616c6c6572"),
            0,
            9999999999,
            "INFO"
        );

        vm.execute();
        vm.execute();

        println!("{:?}", vm.stack.peek().encode_hex());

    }
}