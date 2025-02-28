//! Common utilities, constants, and resources used across the Heimdall codebase.
//!
//! This crate provides shared functionality for the Heimdall toolkit, including
//! Ethereum-related utilities, common resources, and general utility functions.

/// Constants used throughout the Heimdall codebase.
pub mod constants;

/// Utilities for interacting with Ethereum, including bytecode, calldata,
/// and RPC functionality.
pub mod ether;

/// External resources and API integrations, such as OpenAI and Transpose.
pub mod resources;

/// General utility functions and types for common tasks.
pub mod utils;
