#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Fetch error: {0}")]
    FetchError(String),
    #[error("Internal error: {0}")]
    Eyre(#[from] eyre::Report),
    #[error("Decoder error: {0}")]
    DecodeError(#[from] heimdall_decoder::error::Error),
    #[error("Transpose error: {0}")]
    TransposeError(String),
}
