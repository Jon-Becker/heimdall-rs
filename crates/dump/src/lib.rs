//! The Dump module allows for storage slot data extraction from a contract.
//! It provides functionality to dump the storage slots for a given contract.

/// Error types for the dump module
pub mod error;

mod core;
mod interfaces;

// re-export the public interface
pub use core::dump;
pub use error::Error;
pub use interfaces::{DumpArgs, DumpArgsBuilder};
