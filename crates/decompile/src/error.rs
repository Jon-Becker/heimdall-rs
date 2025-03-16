/// Error type for the Decompiler module
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Error when fetching data from external sources
    #[error("Fetch error: {0}")]
    FetchError(String),
    /// Error during the disassembly process
    #[error("Disassembly error: {0}")]
    DisassemblyError(#[from] heimdall_disassembler::Error),
    /// Generic internal error
    #[error("Internal error: {0}")]
    Eyre(#[from] eyre::Report),
}
