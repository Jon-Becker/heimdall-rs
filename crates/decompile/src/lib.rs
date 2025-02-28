//! The Decompile module provides functionality to convert EVM bytecode
//! into higher-level Solidity-like code.
//!
//! This module enables the analysis of compiled smart contracts by reconstructing
//! the original source code structure, making bytecode more human-readable and
//! understandable.

/// Error types for the decompiler module
mod error;

mod core;
mod interfaces;
mod utils;

// re-export the public interface
pub use core::{decompile, DecompileResult};
pub use error::Error;
pub use interfaces::{DecompilerArgs, DecompilerArgsBuilder};
