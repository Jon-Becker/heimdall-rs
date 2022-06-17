use std::{str::FromStr, ops::Div};

use ethers::{prelude::U256, abi::AbiEncode};

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
            instruction: 0,
            bytecode: bytecode.replace("0x", ""),
            calldata: calldata.replace("0x", ""),
            address: address.replace("0x", ""),
            origin: origin.replace("0x", ""),
            caller: caller.replace("0x", ""),
            value: value,
            gas_remaining: gas_limit,
            gas_used: 0,

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
                if op ==  1 {
                    let a = self.stack.pop();
                    let b = self.stack.pop();

                    self.stack.push(a.overflowing_add(b).0.encode_hex().as_str());
                }


                // MUL
                if op ==  2 {
                    let a = self.stack.pop();
                    let b = self.stack.pop();

                    self.stack.push(a.overflowing_mul(b).0.encode_hex().as_str());
                }


                // SUB
                if op ==  3 {
                    let a = self.stack.pop();
                    let b = self.stack.pop();

                    self.stack.push(a.overflowing_sub(b).0.encode_hex().as_str());
                }


                // DIV
                if op ==  4 {
                    let a = self.stack.pop();
                    let b = self.stack.pop();

                    self.stack.push(a.div(b).encode_hex().as_str());
                }


                // SDIV
                if op ==  5 {
                    let a = self.stack.pop();
                    let b = self.stack.pop();

                    self.stack.push(sign_uint(a).div(sign_uint(b)).encode_hex().as_str());
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
            String::from("0x7fFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF7fFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFE05"),
            String::from(""),
            String::from("0x6865696d64616c6c000000000061646472657373"),
            String::from("0x6865696d64616c6c0000000000006f726967696e"),
            String::from("0x6865696d64616c6c00000000000063616c6c6572"),
            0,
            9999999999,
            "INFO"
        );

        vm.execute();
        vm.execute();
        vm.execute();

        println!("{:?}", vm.stack.stack);

    }
}