//! CFG Errors

/// Generic error type for the CFG Module
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Error when trying to fetch information from the chain
    #[error("Fetch error: {0}")]
    FetchError(String),
    /// Error when disassembling contract bytecode
    #[error("Disassembly error: {0}")]
    DisassemblyError(#[from] heimdall_disassembler::Error),
    /// Generic error
    #[error("Internal error: {0}")]
    Eyre(#[from] eyre::Report),
}
