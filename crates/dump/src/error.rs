// TODO: after all errors are fixed, remove most instances of Generic for
// specific errors (e.g. ParseError, FilesystemError, etc.)
/// Generic error type for the Dump Module
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Generic internal error
    #[error("Internal error: {0}")]
    Eyre(#[from] eyre::Report),
}
