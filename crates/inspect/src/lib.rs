pub mod error;

mod core;
mod interfaces;
mod utils;

// re-export the public interface
pub use core::{inspect, InspectResult};
pub use interfaces::{InspectArgs, InspectArgsBuilder};
