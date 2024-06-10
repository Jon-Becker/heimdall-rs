pub mod error;

mod core;
mod interfaces;
mod utils;

// re-export the public interface
pub use core::{decode, DecodeResult};
pub use error::Error;
pub use interfaces::{DecodeArgs, DecodeArgsBuilder};
