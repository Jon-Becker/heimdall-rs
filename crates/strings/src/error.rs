/// Errors from loading bytecode or writing extracted strings.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The target bytecode could not be loaded.
    #[error("failed to load bytecode: {0}")]
    FetchError(#[from] eyre::Report),
    /// The output writer failed.
    #[error("failed to write strings: {0}")]
    WriteError(#[from] std::io::Error),
}
