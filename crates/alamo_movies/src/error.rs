use thiserror::Error;

/// An error produced while configuring or calling the Alamo API.
#[derive(Debug, Error)]
pub enum Error {
    /// The configured Alamo base URL could not be parsed.
    #[error("invalid Alamo base URL: {0}")]
    InvalidBaseUrl(#[from] url::ParseError),
    /// An HTTP request or response operation failed.
    #[error("Alamo request failed: {0}")]
    Request(#[from] reqwest::Error),
}
