use std::{collections::VecDeque, str::FromStr};

use ethers::prelude::U256;

// This implemtation is a simple, (hopefully lightweight) LIFO stack.
// Supports simple push/pop operations, with further helper operations
// such as peek and is_empty.
#[derive(Clone, Debug)]
pub struct Stack {
    pub stack: VecDeque<U256>,
}

// TODO: handle panics
impl Stack {
    pub fn new() -> Stack {
        Stack {
            stack: VecDeque::new(),
        }
    }

    // Push a value onto the stack.
    pub fn push(&mut self, value: &str) {
        self.stack.push_front(U256::from_str(&value).unwrap());
    }

    // Pop a value off the stack.
    pub fn pop(&mut self) -> U256 {
        match self.stack.pop_front() {
            Some(value) => value,
            None => U256::from(0 as u8),
        }
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
    pub fn swap(&mut self, n: usize) -> bool {
        match self.stack.get(n) {
            Some(_) => {
                self.stack.swap(0, n);
                return true;
            }
            None => return false,
        }
    }

    // Duplicate the nth value on the stack.
    pub fn dup(&mut self, n: usize) -> bool {
        match self.stack.get(n - 1) {
            Some(_) => {
                self.stack.push_front(self.stack[n - 1]);
                return true;
            }
            None => return false,
        }
    }

    // Peek at the top value on the stack.
    pub fn peek(&self, index: usize) -> U256 {
        match self.stack.get(index) {
            Some(value) => value.to_owned(),
            None => U256::from(0 as u8),
        }
    }

    pub fn peek_n(&self, n: usize) -> Vec<U256> {
        let mut values = Vec::new();
        for i in 0..n {
            values.push(self.peek(i));
        }
        values
    }

    // Check if the stack is empty.
    pub fn is_empty(&self) -> bool {
        self.stack.is_empty()
    }
}
