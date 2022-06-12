use std::{collections::VecDeque, str::FromStr};

use ethers::prelude::U256;

// This implemtation is a simple, (hopefully lightweight) LIFO stack.
// Supports simple push/pop operations, with further helper operations
// such as peek and is_empty.
pub struct Stack {
    pub stack: VecDeque<U256>
}

// TODO: handle panics
// TODO: implement swap, dup
impl Stack {
    pub fn new() -> Stack {
        Stack { stack: VecDeque::new() }
    }


    // Push a value onto the stack.
    pub fn push(&mut self, value: &str) {
        self.stack.push_front(U256::from_str(&value).unwrap());
    }


    // Pop a value off the stack.
    pub fn pop(&mut self) -> U256 {
        self.stack.pop_front().unwrap()
    }


    // Pop n values off the stack.
    pub fn pop_n(&mut self, n: usize) -> Vec<U256> {
        let mut values = Vec::new();
        for _ in 0..n {
            values.push(self.pop());
        }
        values
    }


    // Swap the top value and the nth value on the stack.
    pub fn swap(&mut self, n: usize) {
        self.stack.swap(0, n)
    }

    
    // Duplicate the nth value on the stack.
    pub fn dup(&mut self, n: usize) {
        self.stack.push_front(self.stack[n-1]);
    }


    // Peek at the top value on the stack.
    pub fn peek(&self) -> U256 {
        self.stack.get(0).unwrap().to_owned()
    }


    // Check if the stack is empty.
    pub fn is_empty(&self) -> bool {
        self.stack.is_empty()
    }

}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;

    #[test]
    fn stack() {

        let mut stack = Stack::new();
        stack.push("0x01");
        stack.push("0x02");
        
        // test push, peek and pop
        assert_eq!(stack.stack, vec![U256::from_str("0x02").unwrap(), U256::from_str("0x01").unwrap()]);
        assert_eq!(stack.peek(), U256::from_str("0x02").unwrap());
        assert_eq!(stack.pop(), U256::from_str("0x02").unwrap());

        // testing pop_n
        stack.push("0x03");
        assert_eq!(stack.stack, vec![U256::from_str("0x03").unwrap(), U256::from_str("0x01").unwrap()]);
        assert_eq!(stack.pop_n(2), vec![U256::from_str("0x03").unwrap(), U256::from_str("0x01").unwrap()]);
        
        // test empty stack
        assert_eq!(stack.is_empty(), true);

        // testing swap
        stack.push("0x02");
        stack.push("0x00");
        stack.push("0x00");
        stack.push("0x00");
        stack.push("0x00");
        stack.push("0x00");
        stack.push("0x00");
        stack.push("0x00");
        stack.push("0x01");
        stack.swap(8);
        assert_eq!(stack.peek(), U256::from_str("0x02").unwrap());

        // testing dup
        stack.push("0x01");
        stack.push("0x00");
        stack.push("0x00");
        stack.push("0x00");
        stack.push("0x00");
        stack.push("0x00");
        stack.push("0x00");
        stack.push("0x00");
        stack.dup(8);
        assert_eq!(stack.peek(), U256::from_str("0x01").unwrap());
    }

}
