#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Error: {0}")]
    Generic(String),
    #[error("Parse error: {0}")]
    ParseError(String),
    #[error("Filesystem error: {0}")]
    FilesystemError(#[from] std::io::Error),
    #[error("HTTP error: {0}")]
    HttpError(#[from] reqwest::Error),
    #[error("{0}")]
    Eyre(#[from] eyre::Report),
}
