/// Error type for the Inspect module
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Error when fetching data from external sources
    #[error("Fetch error: {0}")]
    FetchError(String),
    /// Generic internal error
    #[error("Internal error: {0}")]
    Eyre(#[from] eyre::Report),
    /// Error when decoding transaction data
    #[error("Decoder error: {0}")]
    DecodeError(#[from] heimdall_decoder::error::Error),
    /// Error when transforming data structures
    #[error("Transpose error: {0}")]
    TransposeError(String),
}
