#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Generic(String),
    #[error("IO error: {0}")]
    IOError(#[from] std::io::Error),
    #[error("Serde error: {0}")]
    SerdeError(#[from] serde_json::Error),
    #[error("Decompile error: {0}")]
    DecompileError(#[from] heimdall_core::heimdall_decompiler::Error),
    #[error("Disassemble error: {0}")]
    DisassembleError(#[from] heimdall_core::heimdall_disassembler::Error),
}
