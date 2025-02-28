/// Error type for the Disassembler module
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Generic internal error that may occur during disassembly
    #[error("Internal error: {0}")]
    Eyre(#[from] eyre::Report),
}
