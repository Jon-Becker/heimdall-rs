//! Error types for the configuration module

use mesc::MescError;

/// Errors that can occur during configuration operations
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// A generic error with a message
    #[error("Error: {0}")]
    Generic(String),

    /// An error that occurred during parsing
    #[error("Parse error: {0}")]
    ParseError(String),

    /// An error from the MESC (Modular Ethereum Signing Client) system
    #[error("MESC error: {0}")]
    MescError(#[from] MescError),
}
