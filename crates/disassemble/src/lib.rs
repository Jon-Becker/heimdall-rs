//! The Disassembler module provides functionality to convert EVM bytecode
//! into human-readable assembly instructions.
//!
//! This module enables the translation of raw bytecode into meaningful operations,
//! which is a critical step for understanding and analyzing smart contracts.

/// Error types for the disassembler module
pub mod error;

mod core;
mod interfaces;

// re-export the public interface
pub use core::disassemble;
pub use error::Error;
pub use interfaces::{DisassemblerArgs, DisassemblerArgsBuilder};
