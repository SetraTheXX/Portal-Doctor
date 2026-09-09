use thiserror::Error;

/// Top-level run errors surfaced by the `CLI`.
#[derive(Debug, Error)]
pub enum Error {
    /// The rendered report could not be written to stdout.
    #[error("failed to write output: {0}")]
    Write(#[from] std::io::Error),
    /// The short-lived runtime required by an explicit active probe could not
    /// be initialized. Expected portal/runtime outcomes are represented by
    /// the machine-readable `ProbeResult` instead.
    #[error("active probe runtime could not be initialized: {0}")]
    ProbeRuntime(String),
    /// The standalone active-probe result could not be serialized for output.
    #[error("failed to serialize active probe output: {0}")]
    ProbeOutput(String),
    /// Applying remediation is deliberately outside the current preview-only
    /// contract.
    #[error("only --dry-run remediation previews are supported; apply is not implemented")]
    RemediationApplyUnsupported,
}
