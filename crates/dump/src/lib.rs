pub mod error;

mod core;
mod interfaces;
mod utils;

// re-export the public interface
pub use core::dump;
pub use interfaces::{DumpArgs, DumpArgsBuilder};
