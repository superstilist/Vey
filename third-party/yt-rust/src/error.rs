use thiserror::Error;

/// Errors returned by providers and discovery helpers.
#[derive(Debug, Error)]
pub enum Error {
    #[error("provider does not support {0}")]
    Unsupported(&'static str),
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("provider configuration is missing: {0}")]
    MissingConfiguration(&'static str),
    #[error("provider quota was exceeded: {0}")]
    QuotaExceeded(String),
    #[error("provider rejected the request ({status}): {message}")]
    Provider { status: u16, message: String },
    #[error("response could not be decoded: {0}")]
    Decode(#[from] serde_json::Error),
    #[cfg(any(feature = "youtube-data", feature = "youtube-music"))]
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
