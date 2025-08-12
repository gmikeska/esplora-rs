use thiserror::Error;

/// Represents the possible errors that can occur when using the Esplora client.
#[derive(Error, Debug)]
pub enum Error {
    /// An error from the underlying `reqwest` HTTP client.
    #[error("Request error: {0}")]
    Reqwest(#[from] reqwest::Error),
    /// An error parsing a URL.
    #[error("URL parsing error: {0}")]
    Url(#[from] url::ParseError),
    /// An error deserializing JSON data.
    #[error("JSON parsing error: {0}")]
    SerdeJson(#[from] serde_json::Error),
    /// An error related to authentication.
    #[error("Authentication error: {0}")]
    Auth(String),
    /// A required environment variable is missing.
    #[error("Missing environment variable: {0}")]
    EnvVar(String),
    /// A generic API error.
    #[error("API error: {0}")]
    Api(String),
}
