use reqwest::StatusCode;
use thiserror::Error;

/// An error produced while configuring or calling Radarr.
#[derive(Debug, Error)]
pub enum Error {
    /// No non-empty Radarr API token was provided.
    #[error("a Radarr API token is required")]
    MissingApiToken,
    /// The configured Radarr base URL could not be parsed.
    #[error("invalid Radarr base URL: {0}")]
    InvalidBaseUrl(#[from] url::ParseError),
    /// The API token could not be represented as an HTTP header value.
    #[error("invalid Radarr API token: {0}")]
    InvalidApiToken(#[from] reqwest::header::InvalidHeaderValue),
    /// An HTTP request or response operation failed.
    #[error("Radarr request failed: {0}")]
    Request(#[from] reqwest::Error),
    /// Radarr returned a non-success HTTP response.
    #[error("Radarr returned {status}: {body}")]
    Api {
        /// The HTTP status returned by Radarr.
        status: StatusCode,
        /// The response body returned by Radarr.
        body: String,
    },
    /// A movie payload was built without a destination root folder.
    #[error("movie payload requires a non-empty root folder path")]
    MissingRootFolder,
}
