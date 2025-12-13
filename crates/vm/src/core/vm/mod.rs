//! Virtual Machine implementation for EVM execution.
//!
//! This module provides the core VM struct and its execution logic,
//! organized into submodules for better maintainability.

mod core;
mod execution;

/// Opcode handlers organized by category.
pub mod handlers;

pub use self::core::VM;
pub use execution::{ExecutionResult, Instruction, State};
