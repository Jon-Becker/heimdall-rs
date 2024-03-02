use mesc::MescError;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Error: {0}")]
    Generic(String),
    #[error("Parse error: {0}")]
    ParseError(String),
    #[error("MESC error: {0}")]
    MescError(#[from] MescError),
}
