mod error;

mod core;
mod interfaces;

// re-export the public interface
pub use core::{cfg, CfgResult};
pub use error::Error;
pub use interfaces::{CfgArgs, CfgArgsBuilder};
