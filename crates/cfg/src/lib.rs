pub mod error;

mod core;
mod interfaces;
mod utils;

// re-export the public interface
pub use core::{cfg, CFGResult};
pub use interfaces::{CFGArgs, CFGArgsBuilder};
