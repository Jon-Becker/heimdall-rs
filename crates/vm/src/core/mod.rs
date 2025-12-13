/// Known chain IDs for common networks
pub mod chains;

/// Constants used throughout the VM implementation
pub mod constants;

/// Ethereum hard fork definitions
pub mod hardfork;

/// Log implementation for event handling
pub mod log;

/// Memory implementation for VM memory management
pub mod memory;

/// Opcode definitions and implementations
pub mod opcodes;

/// Stack implementation for the VM
pub mod stack;

/// Storage implementation for contract storage
pub mod storage;

/// Common types and utilities for the VM
pub mod types;

/// Core virtual machine implementation
pub mod vm;

pub use hardfork::HardFork;
pub use vm::{ExecutionResult, Instruction, State, VM};
