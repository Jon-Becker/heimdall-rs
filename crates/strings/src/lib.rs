#![doc = include_str!("../README.md")]

mod core;
mod interfaces;

/// Error types for the strings module.
pub mod error;

pub use core::{strings, write_strings};
pub use error::Error;
pub use interfaces::{StringsArgs, StringsArgsBuilder};
