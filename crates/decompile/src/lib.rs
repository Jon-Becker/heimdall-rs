mod error;

mod core;
mod interfaces;
mod utils;

// re-export the public interface
pub use core::{decompile, DecompileResult};
pub use error::Error;
pub use interfaces::{DecompilerArgs, DecompilerArgsBuilder};
