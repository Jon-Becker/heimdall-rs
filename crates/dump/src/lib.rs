pub mod error;

mod core;
mod interfaces;

// re-export the public interface
pub use core::dump;
pub use interfaces::{DumpArgs, DumpArgsBuilder};
