mod error;

mod core;
mod interfaces;

// re-export the public interface
pub use core::{cfg, CFGResult};
pub use error::Error;
pub use interfaces::{CFGArgs, CFGArgsBuilder};
