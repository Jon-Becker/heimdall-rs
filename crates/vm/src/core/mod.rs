/// Persistent versions for abstract memory and storage
pub mod abstract_state;

/// Worklist-based abstract CFG analysis
pub mod analysis;

/// Known chain IDs for common networks
pub mod chains;

/// Constants used throughout the VM implementation
pub mod constants;

/// Shrinking continuation context for abstract CFG analysis
pub mod context;

/// Lightweight path predicates for abstract analysis
pub mod facts;

/// Ethereum hard fork definitions
pub mod hardfork;

/// Log implementation for event handling
pub mod log;

/// Memory implementation for VM memory management
pub mod memory;

/// Opcode definitions and implementations
pub mod opcodes;

/// Canonical bytecode decoding and basic-block recovery
pub mod program;

/// Stack implementation for the VM
pub mod stack;

/// Context-sensitive stack SSA and effect lowering
pub mod ssa;

/// Interned symbolic expressions for abstract analysis
pub mod symbolic;

#[cfg(feature = "smt")]
/// Demand-driven SMT refinement for abstract analysis
pub mod smt;

/// Storage implementation for contract storage
pub mod storage;

/// Common types and utilities for the VM
pub mod types;

/// Core virtual machine implementation
pub mod vm;

pub use hardfork::HardFork;
pub use vm::{ExecutionResult, Instruction, State, VM};
