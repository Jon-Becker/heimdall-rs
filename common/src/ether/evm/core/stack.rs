use std::collections::VecDeque;

use ethers::prelude::U256;

use super::opcodes::WrappedOpcode;

// This implemtation is a simple, (hopefully lightweight) LIFO stack.
// Supports simple push/pop operations, with further helper operations
// such as peek and is_empty.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Stack {
    pub stack: VecDeque<StackFrame>,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct StackFrame {
    pub value: U256,
    pub operation: WrappedOpcode,
}

impl Stack {
    pub fn new() -> Stack {
        Stack { stack: VecDeque::new() }
    }

    // Push a value onto the stack.
    pub fn push(&mut self, value: U256, operation: WrappedOpcode) {
        self.stack.push_front(StackFrame { value: value, operation });
    }

    // Pop a value off the stack.
    pub fn pop(&mut self) -> StackFrame {
        match self.stack.pop_front() {
            Some(value) => value,
            None => StackFrame { value: U256::from(0u8), operation: WrappedOpcode::default() },
        }
    }

    // Pop n values off the stack.
    pub fn pop_n(&mut self, n: usize) -> Vec<StackFrame> {
        let mut values = Vec::new();
        for _ in 0..n {
            values.push(self.pop());
        }
        values
    }

    // Swap the top value and the nth value on the stack.
    pub fn swap(&mut self, n: usize) -> bool {
        if self.stack.get_mut(n).is_some() {
            self.stack.swap(0, n);
            true
        } else {
            false
        }
    }

    // Duplicate the nth value on the stack.
    pub fn dup(&mut self, n: usize) -> bool {
        match self.stack.get_mut(n - 1) {
            Some(_) => {
                self.stack.push_front(self.stack[n - 1].clone());
                true
            }
            None => false,
        }
    }

    // Peek at the top value on the stack.
    pub fn peek(&self, index: usize) -> StackFrame {
        match self.stack.get(index) {
            Some(value) => value.to_owned(),
            None => StackFrame { value: U256::from(0u8), operation: WrappedOpcode::default() },
        }
    }

    // gets the top n values of the stack
    pub fn peek_n(&self, n: usize) -> Vec<StackFrame> {
        let mut values = Vec::new();
        for i in 0..n {
            values.push(self.peek(i));
        }
        values
    }

    // Get the size of the stack
    pub fn size(&self) -> usize {
        self.stack.len()
    }

    // Check if the stack is empty.
    pub fn is_empty(&self) -> bool {
        self.stack.is_empty()
    }
}
