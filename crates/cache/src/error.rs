//! Cache errors

/// Generic error type for heimdall cache operations
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Generic error
    #[error("Error: {0}")]
    Generic(String),
    /// An IO error occurred
    #[error("IO error: {0}")]
    IOError(#[from] std::io::Error),
}
