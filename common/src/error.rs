#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Error: {0}")]
    Generic(String),
    #[error("IO error: {0}")]
    IOError(String),
}
