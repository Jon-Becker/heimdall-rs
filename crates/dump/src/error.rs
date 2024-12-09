// TODO: after all errors are fixed, remove most instances of Generic for
// specific errors (e.g. ParseError, FilesystemError, etc.)
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Internal error: {0}")]
    Eyre(#[from] eyre::Report),
}
