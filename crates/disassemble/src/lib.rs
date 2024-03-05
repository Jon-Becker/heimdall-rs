pub mod error;

mod core;
mod interfaces;

// re-export the public interface
pub use core::disassemble;
pub use interfaces::{DisassemblerArgs, DisassemblerArgsBuilder};
