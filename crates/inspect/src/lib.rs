//! The Inspect module provides functionality to decode and analyze transaction
//! traces, offering insights into the execution flow of Ethereum transactions.
//!
//! This module enables the examination of contract interactions, function calls,
//! and state changes that occur during a transaction's execution.

/// Error types for the inspect module
pub mod error;

mod core;
mod interfaces;
mod utils;

// re-export the public interface
pub use core::{inspect, InspectResult};
pub use error::Error;
pub use interfaces::{InspectArgs, InspectArgsBuilder};
