use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Request error: {0}")]
    Reqwest(#[from] reqwest::Error),
    #[error("URL parsing error: {0}")]
    Url(#[from] url::ParseError),
    #[error("JSON parsing error: {0}")]
    SerdeJson(#[from] serde_json::Error),
    #[error("Authentication error: {0}")]
    Auth(String),
    #[error("Missing environment variable: {0}")]
    EnvVar(String),
    #[error("API error: {0}")]
    Api(String),
}
