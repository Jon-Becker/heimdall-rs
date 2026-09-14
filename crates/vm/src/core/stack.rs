use std::{
    collections::{BTreeSet, VecDeque},
    fmt::Display,
    hash::{BuildHasher, Hash},
    iter,
};

use alloy::primitives::U256;
use eyre::{OptionExt, Result};
use hashbrown::hash_map::DefaultHashBuilder;

use super::opcodes::WrappedOpcode;

/// A stack value in the finite constant-set abstract domain.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AbstractValue {
    /// A non-empty set of possible concrete values.
    Known(BTreeSet<U256>),
    /// Any 256-bit value.
    Unknown,
}

impl AbstractValue {
    /// Construct a singleton known value.
    pub fn constant(value: U256) -> Self {
        Self::Known(BTreeSet::from([value]))
    }

    /// Return the known alternatives, or `None` when the value is unknown.
    pub fn known_values(&self) -> Option<&BTreeSet<U256>> {
        match self {
            Self::Known(values) => Some(values),
            Self::Unknown => None,
        }
    }

    fn join(&self, other: &Self, max_values: usize) -> Self {
        match (self, other) {
            (Self::Known(left), Self::Known(right)) => {
                let values = left.union(right).copied().collect::<BTreeSet<_>>();
                if values.len() <= max_values {
                    Self::Known(values)
                } else {
                    Self::Unknown
                }
            }
            _ => Self::Unknown,
        }
    }
}

/// Abstract EVM stack, stored from top to bottom.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AbstractStack {
    values: Vec<AbstractValue>,
    unknown_tail: bool,
}

impl AbstractStack {
    /// Construct an empty, exact stack.
    pub fn new() -> Self {
        Self::default()
    }

    /// Construct a stack from explicit values ordered from top to bottom.
    ///
    /// When `unknown_tail` is true, additional values may exist below the provided prefix.
    pub fn from_values(values: Vec<AbstractValue>, unknown_tail: bool) -> Self {
        Self { values, unknown_tail }
    }

    /// Explicit values from the top of the stack downward.
    pub fn values(&self) -> &[AbstractValue] {
        &self.values
    }

    /// Whether additional, untracked values may exist below the explicit values.
    pub fn has_unknown_tail(&self) -> bool {
        self.unknown_tail
    }

    pub(super) fn push(&mut self, value: AbstractValue) {
        self.values.insert(0, value);
    }

    pub(super) fn pop(&mut self) -> Option<AbstractValue> {
        if self.values.is_empty() {
            self.unknown_tail.then_some(AbstractValue::Unknown)
        } else {
            Some(self.values.remove(0))
        }
    }

    pub(super) fn pop_n(&mut self, count: usize) -> bool {
        (0..count).all(|_| self.pop().is_some())
    }

    pub(super) fn peek(&self, index: usize) -> Option<AbstractValue> {
        self.values
            .get(index)
            .cloned()
            .or_else(|| self.unknown_tail.then_some(AbstractValue::Unknown))
    }

    pub(super) fn swap(&mut self, index: usize) -> bool {
        if index >= self.values.len() {
            if !self.unknown_tail {
                return false
            }
            self.values
                .extend(iter::repeat_n(AbstractValue::Unknown, index + 1 - self.values.len()));
        }
        self.values.swap(0, index);
        true
    }

    pub(super) fn join(&self, other: &Self, max_values: usize) -> Self {
        let common_depth = self.values.len().min(other.values.len());
        let values = (0..common_depth)
            .map(|index| self.values[index].join(&other.values[index], max_values))
            .collect();
        Self {
            values,
            unknown_tail: self.unknown_tail ||
                other.unknown_tail ||
                self.values.len() != other.values.len(),
        }
    }
}

/// The [`Stack`] struct represents the EVM stack.
/// It is a LIFO data structure that holds a VecDeque of [`StackFrame`]s.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Stack {
    /// The collection of stack frames in LIFO order.
    ///
    /// The front of the deque represents the top of the stack.
    pub stack: VecDeque<StackFrame>,
}

/// The [`StackFrame`] struct represents a single frame on the stack.
///
/// It holds a [`U256`] value and the [`WrappedOpcode`] that pushed it onto the stack. \
/// \
/// By doing this, we can keep track of the source of each value on the stack in a recursive manner.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct StackFrame {
    /// The value stored in this stack frame.
    ///
    /// In the EVM, all stack values are 256-bit unsigned integers.
    pub value: U256,

    /// The operation that produced this value.
    ///
    /// This allows for tracking the data flow and dependencies between operations.
    pub operation: WrappedOpcode,
}

impl Default for Stack {
    fn default() -> Self {
        Self::new()
    }
}

impl Stack {
    /// Creates a new [`Stack`].
    ///
    /// ```
    /// use heimdall_vm::core::stack::Stack;
    ///
    /// let stack = Stack::new();
    /// assert_eq!(stack.size(), 0);
    /// ```
    pub fn new() -> Stack {
        Stack { stack: VecDeque::with_capacity(1024) }
    }

    /// Push a value onto the stack.
    /// Creates a new [`StackFrame`] with the given [`U256`] value and [`WrappedOpcode`].
    ///
    /// ```
    /// use heimdall_vm::core::{opcodes::WrappedOpcode, stack::Stack};
    /// use alloy::primitives::U256;
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
    /// use heimdall_vm::core::{opcodes::WrappedOpcode, stack::Stack};
    /// use alloy::primitives::U256;
    ///
    /// let mut stack = Stack::new();
    /// stack.push(U256::from(0x00), WrappedOpcode::default());
    ///
    /// let frame = stack.pop();
    /// assert_eq!(frame.unwrap().value, U256::from(0x00));
    /// ```
    pub fn pop(&mut self) -> Result<StackFrame> {
        self.stack.pop_front().ok_or_eyre("stack underflow")
    }

    /// Pop n values off the stack.
    /// Returns a Vec of [`StackFrame`]s with the values and [`WrappedOpcode`]s of the popped
    /// values.
    ///
    /// ```
    /// use heimdall_vm::core::{opcodes::WrappedOpcode, stack::Stack};
    /// use alloy::primitives::U256;
    ///
    /// let mut stack = Stack::new();
    /// stack.push(U256::from(0x00), WrappedOpcode::default());
    /// stack.push(U256::from(0x01), WrappedOpcode::default());
    /// stack.push(U256::from(0x02), WrappedOpcode::default());
    ///
    /// // stack is now [0x02, 0x01, 0x00]
    /// let frames = stack.pop_n(2).unwrap();
    /// assert_eq!(frames[0].value, U256::from(0x02));
    /// assert_eq!(frames[1].value, U256::from(0x01));
    ///
    /// // stack is now [0x00]
    /// assert_eq!(stack.pop().unwrap().value, U256::from(0x00));
    ///
    /// // stack is now []
    /// ```
    pub fn pop_n(&mut self, n: usize) -> Result<Vec<StackFrame>> {
        if n > self.stack.len() {
            return Err(eyre::eyre!("stack underflow"));
        }
        Ok(self.stack.drain(0..n).collect::<Vec<StackFrame>>())
    }

    /// Swap the top value and the nth value on the stack.
    ///
    /// ```
    /// use heimdall_vm::core::{opcodes::WrappedOpcode, stack::Stack};
    /// use alloy::primitives::U256;
    ///
    /// let mut stack = Stack::new();
    /// stack.push(U256::from(0x00), WrappedOpcode::default());
    /// stack.push(U256::from(0x01), WrappedOpcode::default());
    ///
    /// // stack is now [0x01, 0x00]
    /// stack.swap(1);
    ///
    /// // stack is now [0x00, 0x01]
    /// assert_eq!(stack.pop().unwrap().value, U256::from(0x00));
    /// assert_eq!(stack.pop().unwrap().value, U256::from(0x01));
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
    /// use heimdall_vm::core::{opcodes::WrappedOpcode, stack::Stack};
    /// use alloy::primitives::U256;
    ///
    /// let mut stack = Stack::new();
    /// stack.push(U256::from(0x00), WrappedOpcode::default());
    ///
    /// // stack is now [0x00]
    /// stack.dup(1);
    ///
    /// // stack is now [0x00, 0x00]
    /// assert_eq!(stack.pop().unwrap().value, U256::from(0x00));
    /// assert_eq!(stack.pop().unwrap().value, U256::from(0x00));
    ///
    /// // stack is now []
    /// ```
    pub fn dup(&mut self, n: usize) -> bool {
        match self.stack.get(n - 1) {
            Some(item) => {
                self.stack.push_front(item.clone());
                true
            }
            None => false,
        }
    }

    /// Peek at the top value on the stack.
    ///
    /// ```
    /// use heimdall_vm::core::{opcodes::WrappedOpcode, stack::Stack};
    /// use alloy::primitives::U256;
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
    /// use heimdall_vm::core::{opcodes::WrappedOpcode, stack::Stack};
    /// use alloy::primitives::U256;
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
    /// assert_eq!(stack.pop().unwrap().value, U256::from(0x02));
    /// assert_eq!(stack.pop().unwrap().value, U256::from(0x01));
    /// assert_eq!(stack.pop().unwrap().value, U256::from(0x00));
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
    /// use heimdall_vm::core::{opcodes::WrappedOpcode, stack::Stack};
    /// use alloy::primitives::U256;
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
    /// use heimdall_vm::core::{opcodes::WrappedOpcode, stack::Stack};
    /// use alloy::primitives::U256;
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
    /// use heimdall_vm::core::{opcodes::WrappedOpcode, stack::Stack};
    /// use alloy::primitives::U256;
    ///
    /// let mut stack = Stack::new();
    /// stack.push(U256::from(0x00), WrappedOpcode::default());
    ///
    /// // stack is now [0x00]
    /// assert_eq!(stack.hash(), 0x00);
    /// ```
    pub fn hash(&self) -> u64 {
        DefaultHashBuilder::default().hash_one(&self.stack)
    }
}

impl Display for Stack {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut stack = String::new();
        for frame in self.stack.iter() {
            stack.push_str(&format!("{}, ", frame.value));
        }
        write!(f, "[{stack:#02x?}]")
    }
}

#[cfg(test)]
mod tests {
    use super::{AbstractStack, AbstractValue};
    use std::collections::BTreeSet;

    use alloy::primitives::U256;

    #[test]
    fn widens_large_value_sets_to_unknown() {
        let left = AbstractValue::Known(BTreeSet::from([U256::from(1), U256::from(2)]));
        let right = AbstractValue::Known(BTreeSet::from([U256::from(3), U256::from(4)]));
        assert_eq!(left.join(&right, 3), AbstractValue::Unknown);
    }

    #[test]
    fn abstract_stack_preserves_top_to_bottom_order() {
        let mut stack = AbstractStack::new();
        let one = AbstractValue::constant(U256::from(1));
        let two = AbstractValue::constant(U256::from(2));
        stack.push(one.clone());
        stack.push(two.clone());
        assert_eq!(stack.peek(0), Some(two.clone()));
        assert!(stack.swap(1));
        assert_eq!(stack.pop(), Some(one));
        assert_eq!(stack.pop(), Some(two));
        assert_eq!(stack.pop(), None);
        assert!(!stack.swap(1));
    }

    #[test]
    fn abstract_stack_unknown_tail_supplies_untracked_values() {
        let mut stack = AbstractStack::from_values(Vec::new(), true);
        assert_eq!(stack.peek(2), Some(AbstractValue::Unknown));
        assert!(stack.swap(2));
        assert_eq!(stack.values(), vec![AbstractValue::Unknown; 3].as_slice());
        assert!(stack.pop_n(4));
        assert!(stack.has_unknown_tail());
    }

    #[test]
    fn abstract_stack_join_widens_values_and_unequal_depths() {
        let one = AbstractValue::constant(U256::from(1));
        let two = AbstractValue::constant(U256::from(2));
        let left = AbstractStack::from_values(vec![one.clone(), two.clone()], false);
        let right = AbstractStack::from_values(vec![two], false);
        let joined = left.join(&right, 1);
        assert_eq!(joined.values(), &[AbstractValue::Unknown]);
        assert!(joined.has_unknown_tail());
        assert_eq!(left.join(&left, 1), left);
    }

    use crate::core::{opcodes::WrappedOpcode, stack::Stack};

    #[test]
    fn test_push_pop() {
        let mut stack = Stack::new();
        stack.push(U256::from(1), WrappedOpcode::default());
        stack.push(U256::from(2), WrappedOpcode::default());
        assert_eq!(stack.pop().unwrap().value, U256::from(2));
        assert_eq!(stack.pop().unwrap().value, U256::from(1));
        assert!(stack.is_empty());
    }

    #[test]
    fn test_pop_n() {
        let mut stack = Stack::new();
        stack.push(U256::from(1), WrappedOpcode::default());
        stack.push(U256::from(2), WrappedOpcode::default());
        stack.push(U256::from(3), WrappedOpcode::default());
        let values = stack.pop_n(2).unwrap();
        assert_eq!(values.len(), 2);
        assert_eq!(values[0].value, U256::from(3));
        assert_eq!(values[1].value, U256::from(2));
        assert_eq!(stack.pop().unwrap().value, U256::from(1));
        assert!(stack.is_empty());
    }

    #[test]
    fn test_swap() {
        let mut stack = Stack::new();
        stack.push(U256::from(1), WrappedOpcode::default());
        stack.push(U256::from(2), WrappedOpcode::default());
        stack.push(U256::from(3), WrappedOpcode::default());
        assert!(stack.swap(1));
        assert_eq!(stack.pop().unwrap().value, U256::from(2));
        assert_eq!(stack.pop().unwrap().value, U256::from(3));
        assert_eq!(stack.pop().unwrap().value, U256::from(1));
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
        assert_eq!(stack.pop().unwrap().value, U256::from(3));
        assert_eq!(stack.pop().unwrap().value, U256::from(3));
        assert_eq!(stack.pop().unwrap().value, U256::from(2));
        assert_eq!(stack.pop().unwrap().value, U256::from(1));
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
