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
    /// A non-2xx HTTP response from the Esplora API. `status` is the HTTP status
    /// code, `url` the request URL, and `body` the (possibly empty) response
    /// body — so callers can match on `401`/`402`/`404`/`5xx` and surface the
    /// server's message. Rate limiting (`429`) is reported as
    /// [`Error::RateLimited`] instead.
    #[error("HTTP {status} from {url}: {body}")]
    Http {
        /// The HTTP status code (e.g. `401`, `402`, `404`, `503`).
        status: u16,
        /// The request URL that produced this response.
        url: String,
        /// The response body, if any.
        body: String,
    },
    /// A `429 Too Many Requests` response. `retry_after` carries the server's
    /// `Retry-After` header (in seconds) when present, so callers can back off.
    #[error("rate limited by {url} (retry_after={retry_after:?})")]
    RateLimited {
        /// The request URL that was rate limited.
        url: String,
        /// The `Retry-After` value in seconds, when the server supplied one.
        retry_after: Option<u64>,
        /// The response body, if any.
        body: String,
    },
    /// A successful response whose body couldn't be parsed into the expected
    /// non-JSON type (e.g. the plain-text block height).
    #[error("decode error: {0}")]
    Decode(String),
}

impl Error {
    /// Classify a non-2xx response into [`Error::RateLimited`] (for `429`) or
    /// [`Error::Http`]. `retry_after` should be parsed from the `Retry-After`
    /// header before the body is consumed.
    pub(crate) fn from_status(
        status: reqwest::StatusCode,
        url: &url::Url,
        body: String,
        retry_after: Option<u64>,
    ) -> Self {
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            Error::RateLimited {
                url: url.to_string(),
                retry_after,
                body,
            }
        } else {
            Error::Http {
                status: status.as_u16(),
                url: url.to_string(),
                body,
            }
        }
    }
}
