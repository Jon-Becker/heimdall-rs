#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Fetch error: {0}")]
    FetchError(String),
    #[error("Disassembly error: {0}")]
    DisassemblyError(#[from] heimdall_disassembler::Error),
    #[error("Internal error: {0}")]
    Eyre(#[from] eyre::Report),
}
