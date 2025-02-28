//! Heimdall EVM Virtual Machine implementation
//!
//! This crate provides an Ethereum Virtual Machine (EVM) implementation for the Heimdall toolkit,
//! including core VM components and extension modules for analysis and execution.

/// Core VM implementation, including memory, stack, storage, and opcodes
pub mod core;

/// Extensions to the core VM, including execution utilities, lexers, and selector analysis
pub mod ext;
