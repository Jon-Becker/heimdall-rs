// TODO: after all errors are fixed, remove most instances of Generic for
// specific errors (e.g. ParseError, FilesystemError, etc.)
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Json error: {0}")]
    SerdeError(#[from] serde_json::Error),
    #[error("BoundsError")]
    BoundsError,
    #[error("DecodeError")]
    DecodeError,
    #[error("RPCError: {0}")]
    RpcError(String),
    #[error("Error: {0}")]
    Generic(String),
    #[error("TransposeError: {0}")]
    TransposeError(String),
    #[error("Parse error: {0}")]
    ParseError(String),
}
