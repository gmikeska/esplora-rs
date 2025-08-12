use crate::error::Error;
use chrono::{DateTime, Duration, Utc};
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::Mutex;
use url::Url;

const TOKEN_EXPIRY_BUFFER_SECONDS: i64 = 30;

#[derive(Debug, Clone, Deserialize)]
struct TokenResponse {
    access_token: String,
    expires_in: i64,
}

#[derive(Debug, Clone)]
struct Token {
    access_token: String,
    expires: DateTime<Utc>,
}

impl Token {
    fn is_expired(&self) -> bool {
        Utc::now() >= self.expires
    }
}

#[derive(Debug)]
struct AuthInner {
    client_id: String,
    client_secret: String,
    http_client: reqwest::Client,
    token_url: Url,
    token: Option<Token>,
}

/// Handles authentication by automatically fetching and refreshing bearer tokens.
#[derive(Debug, Clone)]
pub struct Auth {
    inner: Arc<Mutex<AuthInner>>,
}

impl Auth {
    /// Creates a new `Auth` instance.
    pub fn new(client_id: String, client_secret: String, token_url: Url) -> Self {
        let inner = AuthInner {
            client_id,
            client_secret,
            http_client: reqwest::Client::new(),
            token_url,
            token: None,
        };
        Self {
            inner: Arc::new(Mutex::new(inner)),
        }
    }

    /// Returns a valid bearer token, fetching a new one if necessary.
    pub async fn get_token(&self) -> Result<String, Error> {
        let mut inner = self.inner.lock().await;

        if let Some(token) = &inner.token {
            if !token.is_expired() {
                return Ok(token.access_token.clone());
            }
        }

        // Token is missing or expired, fetch a new one
        let new_token = self.fetch_token(&mut inner).await?;
        Ok(new_token.access_token)
    }

    async fn fetch_token(&self, inner: &mut AuthInner) -> Result<Token, Error> {
        let params = [
            ("grant_type", "client_credentials"),
            ("client_id", &inner.client_id),
            ("client_secret", &inner.client_secret),
            ("scope", "openid"),
        ];

        let response = inner
            .http_client
            .post(inner.token_url.clone())
            .form(&params)
            .send()
            .await?
            .error_for_status()?; // Ensure we got a 2xx response

        let token_response: TokenResponse = response.json().await?;

        let new_token = Token {
            access_token: token_response.access_token,
            expires: Utc::now() + Duration::seconds(token_response.expires_in - TOKEN_EXPIRY_BUFFER_SECONDS),
        };

        inner.token = Some(new_token.clone());
        Ok(new_token)
    }
}
