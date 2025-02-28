//! The Core module serves as the central integration point for all Heimdall's
//! functionality, providing access to various analysis tools for Ethereum smart contracts.
//!
//! This module re-exports the public interfaces of all the tool-specific crates,
//! making it easier to use Heimdall's capabilities in other projects.

/// Error types for the core module
pub mod error;

// Re-export all tool-specific modules
pub use heimdall_cfg;
pub use heimdall_decoder;
pub use heimdall_decompiler;
pub use heimdall_disassembler;
pub use heimdall_dump;
pub use heimdall_inspect;
