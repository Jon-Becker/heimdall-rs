#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Fetch error: {0}")]
    FetchError(String),
    #[error("Internal error: {0}")]
    Eyre(#[from] eyre::Report),
    #[error("Bounds error")]
    BoundsError,
}
