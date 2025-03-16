// TODO: after all errors are fixed, remove most instances of Generic for
// specific errors (e.g. ParseError, FilesystemError, etc.)
/// Error type for the Core module
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Error when serializing or deserializing JSON data
    #[error("Json error: {0}")]
    SerdeError(#[from] serde_json::Error),
    /// Error when accessing data out of bounds
    #[error("BoundsError")]
    BoundsError,
    /// Error when decoding data
    #[error("DecodeError")]
    DecodeError,
    /// Error when interacting with an RPC endpoint
    #[error("RPCError: {0}")]
    RpcError(String),
    /// Generic error with a message
    #[error("Error: {0}")]
    Generic(String),
    /// Error when transforming data structures
    #[error("TransposeError: {0}")]
    TransposeError(String),
    /// Error when parsing data
    #[error("Parse error: {0}")]
    ParseError(String),
}
