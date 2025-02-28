/// Error type for the Decoder module
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Error when fetching data from external sources
    #[error("Fetch error: {0}")]
    FetchError(String),
    /// Generic internal error
    #[error("Internal error: {0}")]
    Eyre(#[from] eyre::Report),
    /// Error when accessing data out of bounds
    #[error("Bounds error")]
    BoundsError,
}
