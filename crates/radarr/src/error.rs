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
    /// Radarr returned a successful status with a body that was not an added movie resource.
    #[error("Radarr returned invalid add response {status}: {body}: {source}")]
    InvalidAddResponse {
        /// The successful HTTP status returned by Radarr.
        status: StatusCode,
        /// The response body returned by Radarr.
        body: String,
        /// The JSON decoding failure.
        source: serde_json::Error,
    },
    /// Radarr returned success without confirming the requested movie resource.
    #[error(
        "Radarr add response did not confirm TMDB movie {expected_tmdb_id}: id={id}, tmdbId={actual_tmdb_id}"
    )]
    UnexpectedAddResponse {
        /// The TMDB identifier sent in the add request.
        expected_tmdb_id: u32,
        /// The TMDB identifier returned by Radarr.
        actual_tmdb_id: u32,
        /// The Radarr resource identifier returned by Radarr.
        id: u32,
    },
    /// A movie payload was built without a destination root folder.
    #[error("movie payload requires a non-empty root folder path")]
    MissingRootFolder,
}
