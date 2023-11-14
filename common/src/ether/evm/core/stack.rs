use std::{
    collections::VecDeque,
    fmt::Display,
    hash::{Hash, Hasher},
};

use ethers::prelude::U256;

use super::opcodes::WrappedOpcode;

/// The [`Stack`] struct represents the EVM stack.
/// It is a LIFO data structure that holds a VecDeque of [`StackFrame`]s.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Stack {
    pub stack: VecDeque<StackFrame>,
}

/// The [`StackFrame`] struct represents a single frame on the stack.
/// It holds a [`U256`] value and the [`WrappedOpcode`] that pushed it onto the stack. \
/// \
/// By doing this, we can keep track of the source of each value on the stack in a recursive manner.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct StackFrame {
    pub value: U256,
    pub operation: WrappedOpcode,
}

impl Stack {
    /// Creates a new [`Stack`].
    ///
    /// ```
    /// use heimdall_common::ether::evm::core::stack::Stack;
    ///
    /// let stack = Stack::new();
    /// assert_eq!(stack.size(), 0);
    /// ```
    pub fn new() -> Stack {
        Stack { stack: VecDeque::new() }
    }

    /// Push a value onto the stack.
    /// Creates a new [`StackFrame`] with the given [`U256`] value and [`WrappedOpcode`].
    ///
    /// ```
    /// use ethers::prelude::U256;
    /// use heimdall_common::ether::evm::core::{opcodes::WrappedOpcode, stack::Stack};
    ///
    /// let mut stack = Stack::new();
    /// stack.push(U256::from(0x00), WrappedOpcode::default());
    /// assert_eq!(stack.size(), 1);
    /// ```
    pub fn push(&mut self, value: U256, operation: WrappedOpcode) {
        self.stack.push_front(StackFrame { value, operation });
    }

    /// Pop a value off the stack.
    /// Returns a [`StackFrame`] with the value and [`WrappedOpcode`] of the popped value.
    ///
    /// ```
    /// use ethers::prelude::U256;
    /// use heimdall_common::ether::evm::core::{opcodes::WrappedOpcode, stack::Stack};
    ///
    /// let mut stack = Stack::new();
    /// stack.push(U256::from(0x00), WrappedOpcode::default());

    /// let frame = stack.pop();
    /// assert_eq!(frame.value, U256::from(0x00));
    /// ```
    pub fn pop(&mut self) -> StackFrame {
        match self.stack.pop_front() {
            Some(value) => value,
            None => StackFrame { value: U256::from(0u8), operation: WrappedOpcode::default() },
        }
    }

    /// Pop n values off the stack.
    /// Returns a Vec of [`StackFrame`]s with the values and [`WrappedOpcode`]s of the popped
    /// values.
    ///
    /// ```
    /// use ethers::prelude::U256;
    /// use heimdall_common::ether::evm::core::{opcodes::WrappedOpcode, stack::Stack};
    ///
    /// let mut stack = Stack::new();
    /// stack.push(U256::from(0x00), WrappedOpcode::default());
    /// stack.push(U256::from(0x01), WrappedOpcode::default());
    /// stack.push(U256::from(0x02), WrappedOpcode::default());
    ///
    /// // stack is now [0x02, 0x01, 0x00]
    /// let frames = stack.pop_n(2);
    /// assert_eq!(frames[0].value, U256::from(0x02));
    /// assert_eq!(frames[1].value, U256::from(0x01));
    ///
    /// // stack is now [0x00]
    /// assert_eq!(stack.pop().value, U256::from(0x00));
    ///
    /// // stack is now []
    /// ```
    pub fn pop_n(&mut self, n: usize) -> Vec<StackFrame> {
        let mut values = Vec::new();
        for _ in 0..n {
            values.push(self.pop());
        }
        values
    }

    /// Swap the top value and the nth value on the stack.
    ///
    /// ```
    /// use ethers::prelude::U256;
    /// use heimdall_common::ether::evm::core::{opcodes::WrappedOpcode, stack::Stack};
    ///
    /// let mut stack = Stack::new();
    /// stack.push(U256::from(0x00), WrappedOpcode::default());
    /// stack.push(U256::from(0x01), WrappedOpcode::default());
    ///
    /// // stack is now [0x01, 0x00]
    /// stack.swap(1);
    ///
    /// // stack is now [0x00, 0x01]
    /// assert_eq!(stack.pop().value, U256::from(0x00));
    /// assert_eq!(stack.pop().value, U256::from(0x01));
    /// ```
    pub fn swap(&mut self, n: usize) -> bool {
        if self.stack.get_mut(n).is_some() {
            self.stack.swap(0, n);
            true
        } else {
            false
        }
    }

    /// Duplicate the nth value on the stack.
    ///
    /// ```
    /// use ethers::prelude::U256;
    /// use heimdall_common::ether::evm::core::{opcodes::WrappedOpcode, stack::Stack};
    ///
    /// let mut stack = Stack::new();
    /// stack.push(U256::from(0x00), WrappedOpcode::default());
    ///
    /// // stack is now [0x00]
    /// stack.dup(1);
    ///
    /// // stack is now [0x00, 0x00]
    /// assert_eq!(stack.pop().value, U256::from(0x00));
    /// assert_eq!(stack.pop().value, U256::from(0x00));
    ///
    /// // stack is now []
    /// ```
    pub fn dup(&mut self, n: usize) -> bool {
        match self.stack.get_mut(n - 1) {
            Some(_) => {
                self.stack.push_front(self.stack[n - 1].clone());
                true
            }
            None => false,
        }
    }

    /// Peek at the top value on the stack.
    ///
    /// ```
    /// use ethers::prelude::U256;
    /// use heimdall_common::ether::evm::core::{opcodes::WrappedOpcode, stack::Stack};
    ///
    /// let mut stack = Stack::new();
    /// stack.push(U256::from(0x00), WrappedOpcode::default());
    ///
    /// // stack is now [0x00]
    /// assert_eq!(stack.peek(0).value, U256::from(0x00));
    /// ```
    pub fn peek(&self, index: usize) -> StackFrame {
        match self.stack.get(index) {
            Some(value) => value.to_owned(),
            None => StackFrame { value: U256::from(0u8), operation: WrappedOpcode::default() },
        }
    }

    /// gets the top n values of the stack
    ///
    /// ```
    /// use ethers::prelude::U256;
    /// use heimdall_common::ether::evm::core::{opcodes::WrappedOpcode, stack::Stack};
    ///
    /// let mut stack = Stack::new();
    /// stack.push(U256::from(0x00), WrappedOpcode::default());
    /// stack.push(U256::from(0x01), WrappedOpcode::default());
    /// stack.push(U256::from(0x02), WrappedOpcode::default());
    ///
    /// // stack is now [0x02, 0x01, 0x00]
    /// let frames = stack.peek_n(2);
    /// assert_eq!(frames[0].value, U256::from(0x02));
    /// assert_eq!(frames[1].value, U256::from(0x01));
    ///
    /// // stack is still [0x02, 0x01, 0x00]
    /// assert_eq!(stack.pop().value, U256::from(0x02));
    /// assert_eq!(stack.pop().value, U256::from(0x01));
    /// assert_eq!(stack.pop().value, U256::from(0x00));
    ///
    /// // stack is now []
    /// ```
    pub fn peek_n(&self, n: usize) -> Vec<StackFrame> {
        let mut values = Vec::new();
        for i in 0..n {
            values.push(self.peek(i));
        }
        values
    }

    /// Get the size of the stack
    ///
    /// ```
    /// use ethers::prelude::U256;
    /// use heimdall_common::ether::evm::core::{opcodes::WrappedOpcode, stack::Stack};
    ///
    /// let mut stack = Stack::new();
    /// stack.push(U256::from(0x00), WrappedOpcode::default());
    ///
    /// // stack is now [0x00]
    /// assert_eq!(stack.size(), 1);
    /// ```
    pub fn size(&self) -> usize {
        self.stack.len()
    }

    /// Check if the stack is empty.
    ///
    /// ```
    /// use ethers::prelude::U256;
    /// use heimdall_common::ether::evm::core::{opcodes::WrappedOpcode, stack::Stack};
    ///
    /// let mut stack = Stack::new();
    /// stack.push(U256::from(0x00), WrappedOpcode::default());
    ///
    /// // stack is now [0x00]
    /// assert_eq!(stack.is_empty(), false);
    ///
    /// stack.pop();
    ///
    /// // stack is now []
    /// assert_eq!(stack.is_empty(), true);
    /// ```
    pub fn is_empty(&self) -> bool {
        self.stack.is_empty()
    }

    /// A simple hash of the stack. Used in various symbolic execution optimizations.
    ///
    /// ```no_run
    /// use ethers::prelude::U256;
    /// use heimdall_common::ether::evm::core::{opcodes::WrappedOpcode, stack::Stack};
    ///
    /// let mut stack = Stack::new();
    /// stack.push(U256::from(0x00), WrappedOpcode::default());
    ///
    /// // stack is now [0x00]
    /// assert_eq!(stack.hash(), 0x00);
    /// ```
    pub fn hash(&self) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.stack.hash(&mut hasher);
        hasher.finish()
    }
}

impl Display for Stack {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut stack = String::new();
        for frame in self.stack.iter() {
            stack.push_str(&format!("{}, ", frame.value));
        }
        write!(f, "[{:#02x?}]", stack)
    }
}

#[cfg(test)]
mod tests {
    use crate::ether::evm::core::{opcodes::WrappedOpcode, stack::Stack};
    use ethers::types::U256;

    #[test]
    fn test_push_pop() {
        let mut stack = Stack::new();
        stack.push(U256::from(1), WrappedOpcode::default());
        stack.push(U256::from(2), WrappedOpcode::default());
        assert_eq!(stack.pop().value, U256::from(2));
        assert_eq!(stack.pop().value, U256::from(1));
        assert!(stack.is_empty());
    }

    #[test]
    fn test_pop_n() {
        let mut stack = Stack::new();
        stack.push(U256::from(1), WrappedOpcode::default());
        stack.push(U256::from(2), WrappedOpcode::default());
        stack.push(U256::from(3), WrappedOpcode::default());
        let values = stack.pop_n(2);
        assert_eq!(values.len(), 2);
        assert_eq!(values[0].value, U256::from(3));
        assert_eq!(values[1].value, U256::from(2));
        assert_eq!(stack.pop().value, U256::from(1));
        assert!(stack.is_empty());
    }

    #[test]
    fn test_swap() {
        let mut stack = Stack::new();
        stack.push(U256::from(1), WrappedOpcode::default());
        stack.push(U256::from(2), WrappedOpcode::default());
        stack.push(U256::from(3), WrappedOpcode::default());
        assert!(stack.swap(1));
        assert_eq!(stack.pop().value, U256::from(2));
        assert_eq!(stack.pop().value, U256::from(3));
        assert_eq!(stack.pop().value, U256::from(1));
        assert!(stack.is_empty());
        assert!(!stack.swap(1));
    }

    #[test]
    fn test_dup() {
        let mut stack = Stack::new();
        stack.push(U256::from(1), WrappedOpcode::default());
        stack.push(U256::from(2), WrappedOpcode::default());
        stack.push(U256::from(3), WrappedOpcode::default());
        assert!(stack.dup(1));
        assert_eq!(stack.pop().value, U256::from(3));
        assert_eq!(stack.pop().value, U256::from(3));
        assert_eq!(stack.pop().value, U256::from(2));
        assert_eq!(stack.pop().value, U256::from(1));
        assert!(stack.is_empty());
    }

    #[test]
    fn test_peek() {
        let mut stack = Stack::new();
        stack.push(U256::from(1), WrappedOpcode::default());
        stack.push(U256::from(2), WrappedOpcode::default());
        stack.push(U256::from(3), WrappedOpcode::default());
        assert_eq!(stack.peek(0).value, U256::from(3));
        assert_eq!(stack.peek(1).value, U256::from(2));
        assert_eq!(stack.peek(2).value, U256::from(1));
        assert_eq!(stack.peek(3).value, U256::from(0));
    }
}
