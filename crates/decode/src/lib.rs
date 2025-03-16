//! The Decode module provides functionality to decode EVM calldata into
//! human-readable function signatures and parameters.
//!
//! This module enables the analysis of raw transaction data by identifying the
//! function being called and properly parsing its parameters.

/// Error types for the decoder module
pub mod error;

mod core;
mod interfaces;
mod utils;

// re-export the public interface
pub use core::{decode, DecodeResult};
pub use error::Error;
pub use interfaces::{DecodeArgs, DecodeArgsBuilder};
