use thiserror::Error;

/// Top-level run errors surfaced by the `CLI`.
#[derive(Debug, Error)]
pub enum Error {
    /// The rendered report could not be written to stdout.
    #[error("failed to write output: {0}")]
    Write(#[from] std::io::Error),
}
