pub mod error;

mod core;
mod interfaces;
mod utils;

// re-export the public interface
pub use core::{snapshot, SnapshotResult};
pub use interfaces::{SnapshotArgs, SnapshotArgsBuilder};
