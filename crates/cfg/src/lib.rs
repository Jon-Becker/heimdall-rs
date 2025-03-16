//! The CFG module is responsible for generating control-flow graphs from the given
//! contract's source code via symbolic execution.

mod error;

mod core;
mod interfaces;

// re-export the public interface
pub use core::{cfg, CfgResult};
pub use error::Error;
pub use interfaces::{CfgArgs, CfgArgsBuilder};
