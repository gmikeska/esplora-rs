//! An async Rust client for the [Blockstream Esplora] HTTP API, plus the
//! [Waterfalls / QuickSync] descriptor-scan endpoint.
//!
//! The crate is deliberately **`bitcoin`-dependency-free**: every value in and
//! out is a `String`/integer DTO (see [`models`]), so downstream crates on any
//! `bitcoin`/`bdk` version can consume it without a version conflict.
//!
//! # Constructing a client
//! A [`Client`] is created with the **base URL** of the Esplora instance (the
//! part ending in `/api`); endpoint paths are joined onto it.
//!
//! - [`Client::new_public`] — unauthenticated public instances
//!   (e.g. `https://blockstream.info/testnet/api`).
//! - [`Client::new`] — **enterprise** instances that require an OAuth Bearer
//!   token (e.g. `https://enterprise.blockstream.info/testnet/api`).
//!
//! # Environment variables (read by this crate)
//! ⚠️ The **base URL is a parameter**, but the **enterprise credentials are read
//! implicitly from the process environment** by [`Client::new`]:
//!
//! | Variable | Read by | Purpose |
//! |---|---|---|
//! | `ESPLORA_CLIENT_ID` | [`Client::new`] | OAuth `client_id` |
//! | `ESPLORA_CLIENT_SECRET` | [`Client::new`] | OAuth `client_secret` |
//! | `ESPLORA_TEST_LIVE` | test suite only | gate for the crate's live tests |
//!
//! Any `APP_*` variables (`APP_ESPLORA_URL`, `APP_CHAIN_BACKEND`, …) belong to
//! the **implementing program**, not this crate — that program reads them and
//! passes the resulting URL in as `base_url`. *(An explicit
//! `with_credentials(url, id, secret)` constructor that removes the env read is
//! tracked in `docs/TODO.md` as E4.)*
//!
//! # Waterfalls / QuickSync
//! [`Client::get_waterfalls`] / [`Client::get_waterfalls_all`] hit
//! `<base>/waterfalls/v2/waterfalls` — one descriptor query returns a wallet's
//! full per-index history, replacing an address-by-address gap scan. Enterprise
//! tier only (no signet). See the [Waterfalls / QuickSync] docs.
//!
//! [Blockstream Esplora]: https://github.com/Blockstream/esplora/blob/master/API.md
//! [Waterfalls / QuickSync]: https://github.com/Blockstream/waterfalls

use std::env;

pub mod auth;
pub mod error;
pub mod models;

pub use auth::Auth;
pub use error::Error;
pub use models::{
    AddressInfo, AssetInfo, Block, BlockStatus, FeeEstimates, Mempool, Outspend, RecentTx,
    Transaction, TxSeen, TxStatus, Utxo, WaterfallResponse,
};

use bytes::Bytes;
use reqwest::header::{ACCEPT, AUTHORIZATION, RETRY_AFTER};
use reqwest::Client as ReqwestClient;
use tracing::{debug, error, info, trace};
use url::Url;

const DEFAULT_TOKEN_URL: &str =
    "https://login.blockstream.com/realms/blockstream-public/protocol/openid-connect/token";

/// An asynchronous client for the Blockstream Esplora API.
#[derive(Debug, Clone)]
pub struct Client {
    http_client: ReqwestClient,
    base_url: Url,
    auth: Auth,
}

/// Ensure the base URL ends with `/` so [`Url::join`] appends path segments
/// instead of replacing the final one (`…/api` + `tx` → `…/tx`, a silent
/// endpoint bug).
fn ensure_base_slash(url: &str) -> String {
    if url.ends_with('/') {
        url.to_string()
    } else {
        format!("{}/", url)
    }
}

/// Parse the `Retry-After` header (delta-seconds form) from a response, if
/// present and numeric. HTTP-date form is not parsed (returns `None`).
fn retry_after_secs(response: &reqwest::Response) -> Option<u64> {
    response
        .headers()
        .get(RETRY_AFTER)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.trim().parse::<u64>().ok())
}

impl Client {
    /// Creates a new Esplora client for the specified enterprise API URL.
    ///
    /// Reads `ESPLORA_CLIENT_ID` and `ESPLORA_CLIENT_SECRET` from the environment.
    /// Uses the default Blockstream token endpoint.
    ///
    /// # Errors
    ///
    /// Returns an error if the environment variables are not set or if the URL is invalid.
    pub fn new(base_url: &str) -> Result<Self, Error> {
        let client_id = env::var("ESPLORA_CLIENT_ID")
            .map_err(|_| Error::EnvVar("ESPLORA_CLIENT_ID".to_string()))?;
        let client_secret = env::var("ESPLORA_CLIENT_SECRET")
            .map_err(|_| Error::EnvVar("ESPLORA_CLIENT_SECRET".to_string()))?;
        let token_url = Url::parse(DEFAULT_TOKEN_URL).expect("Failed to parse default token URL");

        Self::from_parts(base_url, token_url, client_id, client_secret)
    }

    /// Creates a new Esplora client for a public API URL.
    ///
    /// # Errors
    ///
    /// Returns an error if the URL is invalid.
    pub fn new_public(base_url: &str) -> Result<Self, Error> {
        let auth = Auth::new_public();
        // Force HTTP/1.1 to avoid issues with some servers and HTTP/2 POST requests
        let http_client = ReqwestClient::builder()
            .http1_only()
            .build()
            .map_err(Error::Reqwest)?;
        let base_url = Url::parse(&ensure_base_slash(base_url))?;

        Ok(Self {
            http_client,
            base_url,
            auth,
        })
    }

    /// Creates a new Esplora client from its constituent parts. Useful for testing.
    fn from_parts(
        base_url: &str,
        token_url: Url,
        client_id: String,
        client_secret: String,
    ) -> Result<Self, Error> {
        let auth = Auth::new(client_id, client_secret, token_url);
        // Force HTTP/1.1 to avoid issues with some servers and HTTP/2 POST requests
        let http_client = ReqwestClient::builder()
            .http1_only()
            .build()
            .map_err(Error::Reqwest)?;
        let base_url = Url::parse(&ensure_base_slash(base_url))?;

        Ok(Self {
            http_client,
            base_url,
            auth,
        })
    }

    async fn get<T: serde::de::DeserializeOwned>(&self, path: &str) -> Result<T, Error> {
        let token = self.auth.get_token().await?;
        let url = self.base_url.join(path)?;
        debug!(target: "esplora_rs", "GET {}", url);

        let mut req = self
            .http_client
            .get(url.clone())
            .header(ACCEPT, "application/json");
        if let Some(token) = token {
            req = req.header(AUTHORIZATION, format!("Bearer {}", token));
            trace!(target: "esplora_rs", "Using auth token");
        }

        let response = req.send().await?;
        let status = response.status();
        debug!(target: "esplora_rs", "GET {} -> {}", url, status);

        if !status.is_success() {
            let retry_after = retry_after_secs(&response);
            let body = response.text().await.unwrap_or_default();
            error!(target: "esplora_rs", "GET {} failed ({}): {}", url, status, body);
            return Err(Error::from_status(status, &url, body, retry_after));
        }

        Ok(response.json().await?)
    }

    /// Like [`Self::get`], but attaches URL-encoded query parameters. The `url`
    /// crate percent-encodes values, so callers may pass raw descriptors,
    /// addresses, etc. without hand-encoding `#`, `/`, `*`, `<`, `;`, `>`.
    async fn get_query<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        params: Vec<(String, String)>,
    ) -> Result<T, Error> {
        // Build the URL (consuming the owned `params`) *before* any await, and
        // take `params` by value. Holding any borrowed query slice
        // (`&[(&str, &str)]`) alive across an await point trips the "Send is not
        // general enough" HRTB bound when this future is driven from an async
        // request handler; owned `Vec<(String, String)>` sidesteps it.
        let mut url = self.base_url.join(path)?;
        url.query_pairs_mut()
            .extend_pairs(params.iter().map(|(k, v)| (k.as_str(), v.as_str())));
        drop(params);
        let token = self.auth.get_token().await?;
        debug!(target: "esplora_rs", "GET {}", url);

        let mut req = self
            .http_client
            .get(url.clone())
            .header(ACCEPT, "application/json");
        if let Some(token) = token {
            req = req.header(AUTHORIZATION, format!("Bearer {}", token));
            trace!(target: "esplora_rs", "Using auth token");
        }

        let response = req.send().await?;
        let status = response.status();
        debug!(target: "esplora_rs", "GET {} -> {}", url, status);

        if !status.is_success() {
            let retry_after = retry_after_secs(&response);
            let body = response.text().await.unwrap_or_default();
            error!(target: "esplora_rs", "GET {} failed ({}): {}", url, status, body);
            return Err(Error::from_status(status, &url, body, retry_after));
        }

        Ok(response.json().await?)
    }

    /// Generic POST returning a JSON body. Unused since `broadcast_tx` reads
    /// the plain-text txid directly; kept for future POST-JSON endpoints.
    #[allow(dead_code)]
    async fn post<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        body: String,
    ) -> Result<T, Error> {
        let token = self.auth.get_token().await?;
        let url = self.base_url.join(path)?;
        debug!(target: "esplora_rs", "POST {} (body_len={})", url, body.len());
        trace!(target: "esplora_rs", "POST body: {}", &body[..body.len().min(200)]);

        let mut req = self
            .http_client
            .post(url.clone())
            .header(ACCEPT, "application/json");
        if let Some(token) = token {
            req = req.header(AUTHORIZATION, format!("Bearer {}", token));
            trace!(target: "esplora_rs", "Using auth token");
        }

        let response = req.body(body).send().await?;
        let status = response.status();
        debug!(target: "esplora_rs", "POST {} -> {}", url, status);

        if !status.is_success() {
            let retry_after = retry_after_secs(&response);
            let resp_body = response.text().await.unwrap_or_default();
            error!(target: "esplora_rs", "POST {} failed ({}): {}", url, status, resp_body);
            return Err(Error::from_status(status, &url, resp_body, retry_after));
        }

        Ok(response.json().await?)
    }

    async fn get_plain(&self, path: &str) -> Result<String, Error> {
        let token = self.auth.get_token().await?;
        let url = self.base_url.join(path)?;
        debug!(target: "esplora_rs", "GET (plain) {}", url);

        let mut req = self
            .http_client
            .get(url.clone())
            .header(ACCEPT, "text/plain");
        if let Some(token) = token {
            req = req.header(AUTHORIZATION, format!("Bearer {}", token));
            trace!(target: "esplora_rs", "Using auth token");
        }

        let response = req.send().await?;
        let status = response.status();
        let retry_after = retry_after_secs(&response);
        let body = response.text().await?;

        debug!(target: "esplora_rs", "GET (plain) {} -> {} (len={})", url, status, body.len());

        if !status.is_success() {
            error!(target: "esplora_rs", "GET (plain) {} failed ({}): {}", url, status, body);
            return Err(Error::from_status(status, &url, body, retry_after));
        }

        trace!(target: "esplora_rs", "GET (plain) response: {}", &body[..body.len().min(200)]);
        Ok(body)
    }

    async fn get_raw(&self, path: &str) -> Result<Bytes, Error> {
        let token = self.auth.get_token().await?;
        let url = self.base_url.join(path)?;
        debug!(target: "esplora_rs", "GET (raw) {}", url);

        let mut req = self
            .http_client
            .get(url.clone())
            .header(ACCEPT, "application/octet-stream");
        if let Some(token) = token {
            req = req.header(AUTHORIZATION, format!("Bearer {}", token));
            trace!(target: "esplora_rs", "Using auth token");
        }

        let response = req.send().await?;
        let status = response.status();
        debug!(target: "esplora_rs", "GET (raw) {} -> {}", url, status);

        if !status.is_success() {
            let retry_after = retry_after_secs(&response);
            let body = response.text().await.unwrap_or_default();
            error!(target: "esplora_rs", "GET (raw) {} failed ({}): {}", url, status, body);
            return Err(Error::from_status(status, &url, body, retry_after));
        }

        let bytes = response.bytes().await?;
        debug!(target: "esplora_rs", "GET (raw) {} returned {} bytes", url, bytes.len());
        Ok(bytes)
    }

    // Blocks
    /// Gets a block by its hash.
    ///
    /// # Errors
    /// Returns [`Error`] if the request fails, the endpoint returns a non-2xx
    /// status, or the response body cannot be decoded.
    pub async fn get_block(&self, hash: &str) -> Result<Block, Error> {
        self.get(&format!("block/{}", hash)).await
    }

    /// Gets the hex-encoded block header by its hash.
    ///
    /// # Errors
    /// Returns [`Error`] if the request fails, the endpoint returns a non-2xx
    /// status, or the response body cannot be decoded.
    pub async fn get_block_header(&self, hash: &str) -> Result<String, Error> {
        self.get_plain(&format!("block/{}/header", hash)).await
    }

    /// Gets the status of a block by its hash.
    ///
    /// # Errors
    /// Returns [`Error`] if the request fails, the endpoint returns a non-2xx
    /// status, or the response body cannot be decoded.
    pub async fn get_block_status(&self, hash: &str) -> Result<BlockStatus, Error> {
        self.get(&format!("block/{}/status", hash)).await
    }

    /// Gets a list of transaction IDs in a block.
    ///
    /// # Errors
    /// Returns [`Error`] if the request fails, the endpoint returns a non-2xx
    /// status, or the response body cannot be decoded.
    pub async fn get_block_txids(&self, hash: &str) -> Result<Vec<String>, Error> {
        self.get(&format!("block/{}/txids", hash)).await
    }

    /// Gets the transaction ID at a specific index in a block.
    ///
    /// # Errors
    /// Returns [`Error`] if the request fails, the endpoint returns a non-2xx
    /// status, or the response body cannot be decoded.
    pub async fn get_block_txid_at_index(&self, hash: &str, index: u64) -> Result<String, Error> {
        self.get_plain(&format!("block/{}/txid/{}", hash, index))
            .await
    }

    /// Gets the raw block by its hash.
    ///
    /// # Errors
    /// Returns [`Error`] if the request fails, the endpoint returns a non-2xx
    /// status, or the response body cannot be decoded.
    pub async fn get_raw_block(&self, hash: &str) -> Result<Bytes, Error> {
        self.get_raw(&format!("block/{}/raw", hash)).await
    }

    /// Gets the block hash at a specific height.
    ///
    /// # Errors
    /// Returns [`Error`] if the request fails, the endpoint returns a non-2xx
    /// status, or the response body cannot be decoded.
    pub async fn get_block_hash_from_height(&self, height: u64) -> Result<String, Error> {
        debug!(target: "esplora_rs", "get_block_hash_from_height: Fetching hash for height {}", height);
        let hash = self.get_plain(&format!("block-height/{}", height)).await?;
        info!(target: "esplora_rs", "get_block_hash_from_height: Height {} -> {}", height, hash.trim());
        Ok(hash)
    }

    /// Gets a list of blocks starting from a specific height.
    ///
    /// # Errors
    /// Returns [`Error`] if the request fails, the endpoint returns a non-2xx
    /// status, or the response body cannot be decoded.
    pub async fn get_blocks(&self, start_height: Option<u64>) -> Result<Vec<Block>, Error> {
        let path = if let Some(height) = start_height {
            format!("blocks/{}", height)
        } else {
            "blocks".to_string()
        };
        self.get(&path).await
    }

    /// Gets the hash of the current tip of the chain.
    ///
    /// # Errors
    /// Returns [`Error`] if the request fails, the endpoint returns a non-2xx
    /// status, or the response body cannot be decoded.
    pub async fn get_tip_hash(&self) -> Result<String, Error> {
        self.get_plain("blocks/tip/hash").await
    }

    /// Gets the height of the current tip of the chain.
    ///
    /// # Errors
    /// Returns [`Error`] if the request fails, the endpoint returns a non-2xx
    /// status, or the response body cannot be decoded.
    pub async fn get_tip_height(&self) -> Result<u64, Error> {
        let height_str = self.get_plain("blocks/tip/height").await?;
        height_str
            .parse::<u64>()
            .map_err(|e| Error::Decode(format!("Failed to parse height: {}", e)))
    }

    /// Gets a list of transactions in a block.
    pub async fn get_block_txs(
        &self,
        hash: &str,
        start_index: Option<u64>,
    ) -> Result<Vec<Transaction>, Error> {
        let path = if let Some(start) = start_index {
            format!("block/{}/txs/{}", hash, start)
        } else {
            format!("block/{}/txs", hash)
        };
        self.get(&path).await
    }

    // Transactions
    /// Gets a transaction by its ID.
    ///
    /// # Errors
    /// Returns [`Error`] if the request fails, the endpoint returns a non-2xx
    /// status, or the response body cannot be decoded.
    pub async fn get_tx(&self, txid: &str) -> Result<Transaction, Error> {
        self.get(&format!("tx/{}", txid)).await
    }

    /// Gets the status of a transaction by its ID.
    ///
    /// # Errors
    /// Returns [`Error`] if the request fails, the endpoint returns a non-2xx
    /// status, or the response body cannot be decoded.
    pub async fn get_tx_status(&self, txid: &str) -> Result<TxStatus, Error> {
        self.get(&format!("tx/{}/status", txid)).await
    }

    /// Gets the hex-encoded transaction by its ID.
    ///
    /// # Errors
    /// Returns [`Error`] if the request fails, the endpoint returns a non-2xx
    /// status, or the response body cannot be decoded.
    pub async fn get_tx_hex(&self, txid: &str) -> Result<String, Error> {
        self.get_plain(&format!("tx/{}/hex", txid)).await
    }

    /// Gets the raw transaction by its ID.
    ///
    /// # Errors
    /// Returns [`Error`] if the request fails, the endpoint returns a non-2xx
    /// status, or the response body cannot be decoded.
    pub async fn get_raw_tx(&self, txid: &str) -> Result<Bytes, Error> {
        self.get_raw(&format!("tx/{}/raw", txid)).await
    }

    /// Gets the Merkle block proof for a transaction.
    ///
    /// # Errors
    /// Returns [`Error`] if the request fails, the endpoint returns a non-2xx
    /// status, or the response body cannot be decoded.
    pub async fn get_tx_merkle_block_proof(&self, txid: &str) -> Result<String, Error> {
        self.get_plain(&format!("tx/{}/merkleblock-proof", txid))
            .await
    }

    /// Gets the spending status of a transaction output.
    ///
    /// # Errors
    /// Returns [`Error`] if the request fails, the endpoint returns a non-2xx
    /// status, or the response body cannot be decoded.
    pub async fn get_outspend(&self, txid: &str, vout: u32) -> Result<Outspend, Error> {
        self.get(&format!("tx/{}/outspend/{}", txid, vout)).await
    }

    /// Gets the spending status of all outputs of a transaction.
    ///
    /// # Errors
    /// Returns [`Error`] if the request fails, the endpoint returns a non-2xx
    /// status, or the response body cannot be decoded.
    pub async fn get_outspends(&self, txid: &str) -> Result<Vec<Outspend>, Error> {
        self.get(&format!("tx/{}/outspends", txid)).await
    }

    /// Broadcasts a transaction to the network.
    ///
    /// Returns the transaction ID on success, or an error with the rejection reason.
    ///
    /// # Errors
    /// Returns [`Error`] if the request fails, the endpoint returns a non-2xx
    /// status, or the response body cannot be decoded.
    pub async fn broadcast_tx(&self, tx_hex: &str) -> Result<String, Error> {
        info!(target: "esplora_rs", "broadcast_tx: Starting broadcast of {} byte tx", tx_hex.len() / 2);
        debug!(target: "esplora_rs", "broadcast_tx: tx_hex first 100 chars: {}", &tx_hex[..tx_hex.len().min(100)]);

        let token = self.auth.get_token().await?;
        let url = self.base_url.join("tx")?;

        info!(target: "esplora_rs", "broadcast_tx: POST {}", url);
        debug!(target: "esplora_rs", "broadcast_tx: Headers - Accept: text/plain, Content-Type: text/plain");

        let mut req = self
            .http_client
            .post(url.clone())
            .header(ACCEPT, "text/plain")
            .header(reqwest::header::CONTENT_TYPE, "text/plain");
        if let Some(token) = token {
            req = req.header(AUTHORIZATION, format!("Bearer {}", token));
            debug!(target: "esplora_rs", "broadcast_tx: Using auth token");
        }

        debug!(target: "esplora_rs", "broadcast_tx: Sending request...");
        let response = match req.body(tx_hex.to_string()).send().await {
            Ok(resp) => {
                debug!(target: "esplora_rs", "broadcast_tx: Request sent successfully");
                resp
            }
            Err(e) => {
                error!(target: "esplora_rs", "broadcast_tx: Request failed to send: {}", e);
                return Err(e.into());
            }
        };

        let status = response.status();
        let retry_after = retry_after_secs(&response);
        info!(target: "esplora_rs", "broadcast_tx: Response status: {} {}", status.as_u16(), status.canonical_reason().unwrap_or(""));

        debug!(target: "esplora_rs", "broadcast_tx: Reading response body...");
        let body = match response.text().await {
            Ok(text) => {
                debug!(target: "esplora_rs", "broadcast_tx: Response body ({} bytes): {}", text.len(), text.trim());
                text
            }
            Err(e) => {
                error!(target: "esplora_rs", "broadcast_tx: Failed to read response body: {}", e);
                return Err(e.into());
            }
        };

        if status.is_success() {
            let txid = body.trim().to_string();
            info!(target: "esplora_rs", "broadcast_tx: SUCCESS! txid={}", txid);
            Ok(txid)
        } else {
            error!(
                target: "esplora_rs",
                "broadcast_tx: FAILED - HTTP {}: {}",
                status.as_u16(),
                body.trim()
            );
            Err(Error::from_status(
                status,
                &url,
                body.trim().to_string(),
                retry_after,
            ))
        }
    }

    // Addresses
    /// Gets information about an address.
    ///
    /// # Errors
    /// Returns [`Error`] if the request fails, the endpoint returns a non-2xx
    /// status, or the response body cannot be decoded.
    pub async fn get_address_info(&self, address: &str) -> Result<AddressInfo, Error> {
        self.get(&format!("address/{}", address)).await
    }

    /// Gets information about a scripthash.
    ///
    /// # Errors
    /// Returns [`Error`] if the request fails, the endpoint returns a non-2xx
    /// status, or the response body cannot be decoded.
    pub async fn get_scripthash_info(&self, hash: &str) -> Result<AddressInfo, Error> {
        self.get(&format!("scripthash/{}", hash)).await
    }

    /// Gets a list of transactions for an address.
    ///
    /// # Errors
    /// Returns [`Error`] if the request fails, the endpoint returns a non-2xx
    /// status, or the response body cannot be decoded.
    pub async fn get_address_txs(&self, address: &str) -> Result<Vec<Transaction>, Error> {
        self.get(&format!("address/{}/txs", address)).await
    }

    /// Gets a list of transactions for an address, starting from a specific transaction.
    pub async fn get_address_txs_chain(
        &self,
        address: &str,
        last_seen_txid: Option<&str>,
    ) -> Result<Vec<Transaction>, Error> {
        let path = if let Some(txid) = last_seen_txid {
            format!("address/{}/txs/chain/{}", address, txid)
        } else {
            format!("address/{}/txs/chain", address)
        };
        self.get(&path).await
    }

    /// Gets a list of unconfirmed transactions for an address.
    ///
    /// # Errors
    /// Returns [`Error`] if the request fails, the endpoint returns a non-2xx
    /// status, or the response body cannot be decoded.
    pub async fn get_address_mempool_txs(&self, address: &str) -> Result<Vec<Transaction>, Error> {
        self.get(&format!("address/{}/txs/mempool", address)).await
    }

    /// Gets a list of unspent transaction outputs for an address.
    ///
    /// # Errors
    /// Returns [`Error`] if the request fails, the endpoint returns a non-2xx
    /// status, or the response body cannot be decoded.
    pub async fn get_address_utxos(&self, address: &str) -> Result<Vec<Utxo>, Error> {
        debug!(target: "esplora_rs", "get_address_utxos: Fetching UTXOs for {}", address);
        let utxos: Vec<Utxo> = self.get(&format!("address/{}/utxo", address)).await?;
        info!(target: "esplora_rs", "get_address_utxos: Found {} UTXOs for {}", utxos.len(), address);
        Ok(utxos)
    }

    /// Searches for addresses with a given prefix.
    ///
    /// # Errors
    /// Returns [`Error`] if the request fails, the endpoint returns a non-2xx
    /// status, or the response body cannot be decoded.
    pub async fn search_addresses(&self, prefix: &str) -> Result<Vec<String>, Error> {
        self.get(&format!("address-prefix/{}", prefix)).await
    }

    // QuickSync / Waterfalls

    /// Fetches one page of `/v2/waterfalls` history for a descriptor, scanning
    /// derivation indices `0..=to_index`.
    ///
    /// A single call returns the descriptor's per-index transaction history
    /// (one `txs_seen` key per single-path branch), replacing the many
    /// per-address round-trips of a gap-limit scan. `to_index` bounds the
    /// server-side derivation (its default is `0`, so pass a real depth).
    ///
    /// The endpoint is `<base>/waterfalls/v2/waterfalls`; point `base_url` at a
    /// host that serves QuickSync (e.g. `enterprise.blockstream.info/<chain>/api`,
    /// authenticated, or a self-hosted `waterfalls` instance).
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails, the endpoint is unavailable, or the
    /// response cannot be decoded.
    pub async fn get_waterfalls(
        &self,
        descriptor: &str,
        to_index: u32,
        page: u16,
    ) -> Result<WaterfallResponse, Error> {
        self.get_query(
            "waterfalls/v2/waterfalls",
            vec![
                ("descriptor".to_string(), descriptor.to_string()),
                ("to_index".to_string(), to_index.to_string()),
                ("page".to_string(), page.to_string()),
            ],
        )
        .await
    }

    /// Pages through `/v2/waterfalls` from page 0 and merges the result into a
    /// single [`WaterfallResponse`], concatenating each descriptor's per-index
    /// history across pages. Stops once a page returns no activity (or a hard
    /// cap is hit) and reports the latest `page`/`tip`.
    ///
    /// # Errors
    ///
    /// Returns an error if any page request fails or cannot be decoded.
    pub async fn get_waterfalls_all(
        &self,
        descriptor: String,
        to_index: u32,
    ) -> Result<WaterfallResponse, Error> {
        /// Safety cap so a misbehaving server can't loop forever.
        const MAX_PAGES: u16 = 100;

        fn has_activity(resp: &WaterfallResponse) -> bool {
            resp.txs_seen.values().flatten().any(|idx| !idx.is_empty())
        }

        // Owned `descriptor` (not a borrowed `&str`) so no external-lifetime
        // borrow is held across an await — keeps the future `for<'a> Send`, as
        // async request handlers require.
        let mut acc = self.get_waterfalls(&descriptor, to_index, 0).await?;
        let mut keep_going = has_activity(&acc);
        let mut page: u16 = 1;
        while keep_going && page < MAX_PAGES {
            let next = self.get_waterfalls(&descriptor, to_index, page).await?;
            keep_going = has_activity(&next);
            for (key, mut sightings) in next.txs_seen {
                acc.txs_seen.entry(key).or_default().append(&mut sightings);
            }
            acc.page = next.page;
            acc.tip = next.tip;
            page += 1;
        }
        Ok(acc)
    }

    // Mempool
    /// Gets information about the mempool.
    ///
    /// # Errors
    /// Returns [`Error`] if the request fails, the endpoint returns a non-2xx
    /// status, or the response body cannot be decoded.
    pub async fn get_mempool_info(&self) -> Result<Mempool, Error> {
        self.get("mempool").await
    }

    /// Gets a list of transaction IDs in the mempool.
    ///
    /// # Errors
    /// Returns [`Error`] if the request fails, the endpoint returns a non-2xx
    /// status, or the response body cannot be decoded.
    pub async fn get_mempool_txids(&self) -> Result<Vec<String>, Error> {
        self.get("mempool/txids").await
    }

    /// Gets a list of recent transactions in the mempool.
    ///
    /// # Errors
    /// Returns [`Error`] if the request fails, the endpoint returns a non-2xx
    /// status, or the response body cannot be decoded.
    pub async fn get_mempool_recent_txs(&self) -> Result<Vec<RecentTx>, Error> {
        self.get("mempool/recent").await
    }

    // Fee Estimates
    /// Gets fee estimates for various confirmation targets.
    ///
    /// # Errors
    /// Returns [`Error`] if the request fails, the endpoint returns a non-2xx
    /// status, or the response body cannot be decoded.
    pub async fn get_fee_estimates(&self) -> Result<FeeEstimates, Error> {
        self.get("fee-estimates").await
    }

    // Assets
    /// Gets information about an asset.
    ///
    /// # Errors
    /// Returns [`Error`] if the request fails, the endpoint returns a non-2xx
    /// status, or the response body cannot be decoded.
    pub async fn get_asset_info(&self, asset_id: &str) -> Result<AssetInfo, Error> {
        self.get(&format!("asset/{}", asset_id)).await
    }

    /// Gets a list of transactions for an asset.
    ///
    /// # Errors
    /// Returns [`Error`] if the request fails, the endpoint returns a non-2xx
    /// status, or the response body cannot be decoded.
    pub async fn get_asset_txs(&self, asset_id: &str) -> Result<Vec<Transaction>, Error> {
        self.get(&format!("asset/{}/txs", asset_id)).await
    }

    /// Gets a list of transactions for an asset, starting from a specific transaction.
    pub async fn get_asset_txs_chain(
        &self,
        asset_id: &str,
        last_seen: Option<&str>,
    ) -> Result<Vec<Transaction>, Error> {
        let path = if let Some(txid) = last_seen {
            format!("asset/{}/txs/chain/{}", asset_id, txid)
        } else {
            format!("asset/{}/txs/chain", asset_id)
        };
        self.get(&path).await
    }

    /// Gets a list of unconfirmed transactions for an asset.
    ///
    /// # Errors
    /// Returns [`Error`] if the request fails, the endpoint returns a non-2xx
    /// status, or the response body cannot be decoded.
    pub async fn get_asset_mempool_txs(&self, asset_id: &str) -> Result<Vec<Transaction>, Error> {
        self.get(&format!("asset/{}/txs/mempool", asset_id)).await
    }

    /// Gets the total supply of an asset.
    ///
    /// # Errors
    /// Returns [`Error`] if the request fails, the endpoint returns a non-2xx
    /// status, or the response body cannot be decoded.
    pub async fn get_asset_supply(&self, asset_id: &str) -> Result<u64, Error> {
        let supply_str = self
            .get_plain(&format!("asset/{}/supply", asset_id))
            .await?;
        supply_str
            .parse::<u64>()
            .map_err(|e| Error::Decode(format!("Failed to parse supply: {}", e)))
    }

    /// Gets the total supply of an asset, in decimal form.
    ///
    /// # Errors
    /// Returns [`Error`] if the request fails, the endpoint returns a non-2xx
    /// status, or the response body cannot be decoded.
    pub async fn get_asset_supply_decimal(&self, asset_id: &str) -> Result<f64, Error> {
        let supply_str = self
            .get_plain(&format!("asset/{}/supply/decimal", asset_id))
            .await?;
        supply_str
            .parse::<f64>()
            .map_err(|e| Error::Decode(format!("Failed to parse decimal supply: {}", e)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use httpmock::prelude::*;
    use std::env;

    fn should_run_live_tests() -> bool {
        env::var("ESPLORA_TEST_LIVE").as_deref() == Ok("live")
    }

    fn mock_auth_server(server: &MockServer) {
        server.mock(|when, then| {
            when.method(POST).path("/token");
            then.status(200)
                .header("content-type", "application/json")
                .body(r#"{"access_token": "test_token", "expires_in": 300}"#);
        });
    }

    #[tokio::test]
    async fn test_get_block_mocked() {
        let server = MockServer::start();
        mock_auth_server(&server);

        let block_hash = "00000000000000000005930aa4894de96644480436473138535038e9e4933eb9";
        let api_mock = server.mock(|when, then| {
            when.method(GET)
                .path(format!("/block/{}", block_hash))
                .header("Authorization", "Bearer test_token");
            then.status(200)
                .header("content-type", "application/json")
                .body_from_file("src/testdata/block.json");
        });

        let token_url = Url::parse(&server.url("/token")).unwrap();
        let client = Client::from_parts(
            &server.base_url(),
            token_url,
            "test_id".to_string(),
            "test_secret".to_string(),
        )
        .unwrap();
        let result = client.get_block(block_hash).await;

        api_mock.assert();
        assert!(result.is_ok());
        let block = result.unwrap();
        assert_eq!(block.id, block_hash);
        assert_eq!(block.height, 600000);
    }

    #[tokio::test]
    async fn test_get_tip_hash_mocked() {
        let server = MockServer::start();
        mock_auth_server(&server);

        let tip_hash = "00000000000000000005930aa4894de96644480436473138535038e9e4933eb9";
        let api_mock = server.mock(|when, then| {
            when.method(GET)
                .path("/blocks/tip/hash")
                .header("Authorization", "Bearer test_token");
            then.status(200)
                .header("content-type", "text/plain")
                .body(tip_hash);
        });

        let token_url = Url::parse(&server.url("/token")).unwrap();
        let client = Client::from_parts(
            &server.base_url(),
            token_url,
            "test_id".to_string(),
            "test_secret".to_string(),
        )
        .unwrap();
        let result = client.get_tip_hash().await;

        api_mock.assert();
        assert_eq!(result.unwrap(), tip_hash);
    }

    #[tokio::test]
    async fn test_get_tip_height_mocked() {
        let server = MockServer::start();
        mock_auth_server(&server);

        let tip_height = "600000";
        let api_mock = server.mock(|when, then| {
            when.method(GET)
                .path("/blocks/tip/height")
                .header("Authorization", "Bearer test_token");
            then.status(200)
                .header("content-type", "text/plain")
                .body(tip_height);
        });

        let token_url = Url::parse(&server.url("/token")).unwrap();
        let client = Client::from_parts(
            &server.base_url(),
            token_url,
            "test_id".to_string(),
            "test_secret".to_string(),
        )
        .unwrap();
        let result = client.get_tip_height().await;

        api_mock.assert();
        assert_eq!(result.unwrap(), 600000);
    }

    // ── E1: structured HTTP error classification ────────────────────────────

    fn test_client(server: &MockServer) -> Client {
        let token_url = Url::parse(&server.url("/token")).unwrap();
        Client::from_parts(
            &server.base_url(),
            token_url,
            "test_id".to_string(),
            "test_secret".to_string(),
        )
        .unwrap()
    }

    #[tokio::test]
    async fn test_non_2xx_maps_to_http_with_status_and_body() {
        let server = MockServer::start();
        mock_auth_server(&server);
        let api_mock = server.mock(|when, then| {
            when.method(GET).path("/address/tb1qbad");
            then.status(404).body("not found");
        });

        let err = test_client(&server)
            .get_address_info("tb1qbad")
            .await
            .unwrap_err();
        api_mock.assert();
        match err {
            Error::Http { status, body, url } => {
                assert_eq!(status, 404);
                assert!(body.contains("not found"), "body: {body}");
                assert!(url.ends_with("/address/tb1qbad"), "url: {url}");
            }
            other => panic!("expected Error::Http, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_429_maps_to_rate_limited_with_retry_after() {
        let server = MockServer::start();
        mock_auth_server(&server);
        let api_mock = server.mock(|when, then| {
            when.method(GET).path("/blocks/tip/height");
            then.status(429)
                .header("Retry-After", "7")
                .body("slow down");
        });

        let err = test_client(&server).get_tip_height().await.unwrap_err();
        api_mock.assert();
        match err {
            Error::RateLimited { retry_after, .. } => assert_eq!(retry_after, Some(7)),
            other => panic!("expected Error::RateLimited, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_broadcast_rejection_maps_to_http() {
        let server = MockServer::start();
        mock_auth_server(&server);
        let api_mock = server.mock(|when, then| {
            when.method(POST).path("/tx");
            then.status(400).body("bad-txns-inputs-missingorspent");
        });

        let err = test_client(&server)
            .broadcast_tx("deadbeef")
            .await
            .unwrap_err();
        api_mock.assert();
        match err {
            Error::Http { status, body, .. } => {
                assert_eq!(status, 400);
                assert!(body.contains("missingorspent"), "body: {body}");
            }
            other => panic!("expected Error::Http, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_unparseable_tip_height_maps_to_decode() {
        let server = MockServer::start();
        mock_auth_server(&server);
        let api_mock = server.mock(|when, then| {
            when.method(GET).path("/blocks/tip/height");
            then.status(200).body("not-a-number");
        });

        let err = test_client(&server).get_tip_height().await.unwrap_err();
        api_mock.assert();
        assert!(matches!(err, Error::Decode(_)), "got {err:?}");
    }

    #[test]
    fn error_display_is_readable() {
        let http = Error::Http {
            status: 404,
            url: "http://x/tx".to_string(),
            body: "nope".to_string(),
        };
        assert!(http.to_string().contains("404"));
        let limited = Error::RateLimited {
            url: "http://x".to_string(),
            retry_after: Some(3),
            body: String::new(),
        };
        assert!(limited.to_string().contains("retry_after"));
    }

    #[tokio::test]
    async fn test_get_block_live() {
        if !should_run_live_tests() {
            println!("Skipping live test for get_block");
            return;
        }
        let client = Client::new_public("https://blockstream.info/testnet/api/").unwrap();
        // A known testnet block
        let block_hash = "0000000053f3c29ea7eab85dfbf7849bc3ddf9a22f1166169989e834ef984db4";
        let result = client.get_block(block_hash).await;

        assert!(result.is_ok(), "API call failed: {:?}", result.err());
        let block = result.unwrap();
        assert_eq!(block.id, block_hash);
    }

    #[tokio::test]
    async fn test_get_tip_height_live() {
        if !should_run_live_tests() {
            println!("Skipping live test for get_tip_height");
            return;
        }
        let client = Client::new_public("https://blockstream.info/testnet/api/").unwrap();
        let result = client.get_tip_height().await;
        assert!(result.is_ok(), "API call failed: {:?}", result.err());
        assert!(
            result.unwrap() > 2_000_000,
            "Testnet height should be over 2M"
        );
    }

    #[tokio::test]
    async fn test_get_tx_mocked() {
        let server = MockServer::start();
        mock_auth_server(&server);

        let txid = "f5de79f0312d803666e3a83f12423cc5825227ee055c56f2d2b58a1d741f8713";
        let api_mock = server.mock(|when, then| {
            when.method(GET)
                .path(format!("/tx/{}", txid))
                .header("Authorization", "Bearer test_token");
            then.status(200)
                .header("content-type", "application/json")
                .body_from_file("src/testdata/transaction.json");
        });

        let token_url = Url::parse(&server.url("/token")).unwrap();
        let client = Client::from_parts(
            &server.base_url(),
            token_url,
            "test_id".to_string(),
            "test_secret".to_string(),
        )
        .unwrap();
        let result = client.get_tx(txid).await;

        api_mock.assert();
        assert!(result.is_ok());
        let tx = result.unwrap();
        assert_eq!(tx.txid, txid);
        assert_eq!(tx.vin.len(), 1);
        assert_eq!(tx.vout.len(), 1);
    }

    #[tokio::test]
    async fn test_get_outspends_mocked() {
        let server = MockServer::start();
        mock_auth_server(&server);

        let txid = "f5de79f0312d803666e3a83f12423cc5825227ee055c56f2d2b58a1d741f8713";
        let api_mock = server.mock(|when, then| {
            when.method(GET)
                .path(format!("/tx/{}/outspends", txid))
                .header("Authorization", "Bearer test_token");
            then.status(200)
                .header("content-type", "application/json")
                .body_from_file("src/testdata/outspends.json");
        });

        let token_url = Url::parse(&server.url("/token")).unwrap();
        let client = Client::from_parts(
            &server.base_url(),
            token_url,
            "test_id".to_string(),
            "test_secret".to_string(),
        )
        .unwrap();
        let result = client.get_outspends(txid).await;

        api_mock.assert();
        assert!(result.is_ok());
        let outspends = result.unwrap();
        assert_eq!(outspends.len(), 2);
        assert!(outspends[0].spent);
        assert!(!outspends[1].spent);
    }

    #[tokio::test]
    async fn test_get_tx_live() {
        if !should_run_live_tests() {
            println!("Skipping live test for get_tx");
            return;
        }
        let client = Client::new_public("https://blockstream.info/testnet/api/").unwrap();
        // A known testnet transaction
        let txid = "29e7085b084db673f7c70cc1fdbbbe8dd75ad075ca5c0aeb233b74b23710c4d4";
        let result = client.get_tx(txid).await;

        assert!(result.is_ok(), "API call failed: {:?}", result.err());
        let tx = result.unwrap();
        assert_eq!(tx.txid, txid);
    }

    #[tokio::test]
    async fn test_get_address_info_mocked() {
        let server = MockServer::start();
        mock_auth_server(&server);

        let address = "2N8DcqzfkYi8CkYzvNNS5amoq3SbAcW_test";
        let api_mock = server.mock(|when, then| {
            when.method(GET)
                .path(format!("/address/{}", address))
                .header("Authorization", "Bearer test_token");
            then.status(200)
                .header("content-type", "application/json")
                .body_from_file("src/testdata/address_info.json");
        });

        let token_url = Url::parse(&server.url("/token")).unwrap();
        let client = Client::from_parts(
            &server.base_url(),
            token_url,
            "test_id".to_string(),
            "test_secret".to_string(),
        )
        .unwrap();
        let result = client.get_address_info(address).await;

        api_mock.assert();
        assert!(result.is_ok());
        let info = result.unwrap();
        assert_eq!(info.address, address);
        assert_eq!(info.chain_stats.tx_count, 10);
    }

    #[tokio::test]
    async fn test_get_address_utxos_mocked() {
        let server = MockServer::start();
        mock_auth_server(&server);

        let address = "2N8DcqzfkYi8CkYzvNNS5amoq3SbAcW_test";
        let api_mock = server.mock(|when, then| {
            when.method(GET)
                .path(format!("/address/{}/utxo", address))
                .header("Authorization", "Bearer test_token");
            then.status(200)
                .header("content-type", "application/json")
                .body_from_file("src/testdata/utxos.json");
        });

        let token_url = Url::parse(&server.url("/token")).unwrap();
        let client = Client::from_parts(
            &server.base_url(),
            token_url,
            "test_id".to_string(),
            "test_secret".to_string(),
        )
        .unwrap();
        let result = client.get_address_utxos(address).await;

        api_mock.assert();
        assert!(result.is_ok());
        let utxos = result.unwrap();
        assert_eq!(utxos.len(), 2);
        assert_eq!(utxos[0].value, 10000);
    }

    #[tokio::test]
    async fn test_get_address_info_live() {
        if !should_run_live_tests() {
            println!("Skipping live test for get_address_info");
            return;
        }
        let client = Client::new_public("https://blockstream.info/testnet/api/").unwrap();
        // A known testnet address with some history
        let address = "tb1qxdjp5w4y7449cm5qensttdeauzlxquqtr289ql";
        let result = client.get_address_info(address).await;

        assert!(result.is_ok(), "API call failed: {:?}", result.err());
        let info = result.unwrap();
        assert_eq!(info.address, address);
        assert!(info.chain_stats.tx_count > 0);
    }

    #[tokio::test]
    async fn test_get_mempool_info_mocked() {
        let server = MockServer::start();
        mock_auth_server(&server);

        let api_mock = server.mock(|when, then| {
            when.method(GET)
                .path("/mempool")
                .header("Authorization", "Bearer test_token");
            then.status(200)
                .header("content-type", "application/json")
                .body_from_file("src/testdata/mempool.json");
        });

        let token_url = Url::parse(&server.url("/token")).unwrap();
        let client = Client::from_parts(
            &server.base_url(),
            token_url,
            "test_id".to_string(),
            "test_secret".to_string(),
        )
        .unwrap();
        let result = client.get_mempool_info().await;

        api_mock.assert();
        assert!(result.is_ok());
        let info = result.unwrap();
        assert_eq!(info.count, 1234);
        assert_eq!(info.fee_histogram.len(), 3);
    }

    #[tokio::test]
    async fn test_get_fee_estimates_mocked() {
        let server = MockServer::start();
        mock_auth_server(&server);

        let api_mock = server.mock(|when, then| {
            when.method(GET)
                .path("/fee-estimates")
                .header("Authorization", "Bearer test_token");
            then.status(200)
                .header("content-type", "application/json")
                .body_from_file("src/testdata/fee-estimates.json");
        });

        let token_url = Url::parse(&server.url("/token")).unwrap();
        let client = Client::from_parts(
            &server.base_url(),
            token_url,
            "test_id".to_string(),
            "test_secret".to_string(),
        )
        .unwrap();
        let result = client.get_fee_estimates().await;

        api_mock.assert();
        assert!(result.is_ok());
        let estimates = result.unwrap();
        assert!(estimates.contains_key("6"));
        assert_eq!(estimates["6"], 68.285);
    }

    #[tokio::test]
    async fn test_get_fee_estimates_live() {
        if !should_run_live_tests() {
            println!("Skipping live test for get_fee_estimates");
            return;
        }
        let client = Client::new_public("https://blockstream.info/testnet/api/").unwrap();
        let result = client.get_fee_estimates().await;

        assert!(result.is_ok(), "API call failed: {:?}", result.err());
        let estimates = result.unwrap();
        assert!(estimates.contains_key("1"));
    }

    #[tokio::test]
    async fn test_get_asset_info_mocked() {
        let server = MockServer::start();
        mock_auth_server(&server);

        let asset_id = "d8a317ce2c14241192cbb3ebdb9696250ca1251a58ba6251c29fcfe126c9ca1f";
        let api_mock = server.mock(|when, then| {
            when.method(GET)
                .path(format!("/asset/{}", asset_id))
                .header("Authorization", "Bearer test_token");
            then.status(200)
                .header("content-type", "application/json")
                .body_from_file("src/testdata/asset.json");
        });

        let token_url = Url::parse(&server.url("/token")).unwrap();
        let client = Client::from_parts(
            &server.base_url(),
            token_url,
            "test_id".to_string(),
            "test_secret".to_string(),
        )
        .unwrap();
        let result = client.get_asset_info(asset_id).await;

        api_mock.assert();
        assert!(result.is_ok());
        let info = result.unwrap();
        assert_eq!(info.asset_id, asset_id);
        assert_eq!(info.ticker.unwrap(), "TEST");
    }

    #[tokio::test]
    async fn test_get_asset_info_live() {
        if !should_run_live_tests() {
            println!("Skipping live test for get_asset_info");
            return;
        }
        // Use the liquid testnet for this
        let client = Client::new_public("https://blockstream.info/liquidtestnet/api/").unwrap();
        // L-BTC asset ID for liquid testnet
        let asset_id = "144c654344aa716d6f3abcc1ca90e5641e4e2a7f633bc09fe3baf64585819a49";
        let result = client.get_asset_info(asset_id).await;

        assert!(result.is_ok(), "API call failed: {:?}", result.err());
        let info = result.unwrap();
        assert_eq!(info.asset_id, asset_id);
    }

    #[test]
    fn test_waterfalls_empty_addresses_parses() {
        // The exact shape returned by a live `?addresses=…` call with no history
        // (captured against enterprise testnet, 2026-07-12). Locks the address
        // form + the empty inner vec + absent optional fields.
        let body = r#"{"txs_seen":{"addresses":[[]]},"page":0,"tip":"00000000000000bcef2b46cddb481fd75a834f2272d58226c097892538660c36"}"#;
        let resp: WaterfallResponse = serde_json::from_str(body).unwrap();
        assert_eq!(resp.page, 0);
        assert_eq!(
            resp.tip,
            "00000000000000bcef2b46cddb481fd75a834f2272d58226c097892538660c36"
        );
        let addrs = resp.txs_seen.get("addresses").unwrap();
        assert_eq!(addrs.len(), 1);
        assert!(addrs[0].is_empty());
    }

    #[tokio::test]
    async fn test_get_waterfalls_mocked() {
        let server = MockServer::start();
        mock_auth_server(&server);

        let descriptor = "wpkh(tpubDTEST/<0;1>/*)#abcdefgh";
        let api_mock = server.mock(|when, then| {
            when.method(GET)
                .path("/waterfalls/v2/waterfalls")
                .query_param("descriptor", descriptor)
                .query_param("to_index", "20")
                .query_param("page", "0")
                .header("Authorization", "Bearer test_token");
            then.status(200)
                .header("content-type", "application/json")
                .body_from_file("src/testdata/waterfalls_v2.json");
        });

        let token_url = Url::parse(&server.url("/token")).unwrap();
        let client = Client::from_parts(
            &server.base_url(),
            token_url,
            "test_id".to_string(),
            "test_secret".to_string(),
        )
        .unwrap();
        let result = client.get_waterfalls(descriptor, 20, 0).await;

        api_mock.assert();
        let resp = result.unwrap();
        assert_eq!(resp.page, 0);
        let sightings = resp.txs_seen.get("wpkh(tpubDTEST/0/*)#abcdefgh").unwrap();
        assert_eq!(sightings.len(), 2); // two derivation indices
        assert_eq!(sightings[0].len(), 2); // index 0 has two txs
        assert_eq!(
            sightings[0][0].txid,
            "6ac214c3833ee06f7a30636dac66f0e5c025ece2693cc3f85a8c22fb2dcb2fa1"
        );
        assert_eq!(sightings[0][0].block_timestamp, Some(1_715_939_108));
        // index 1's tx is unconfirmed (height 0, no block fields)
        assert_eq!(sightings[1][0].height, 0);
        assert!(sightings[1][0].block_hash.is_none());
    }

    #[tokio::test]
    async fn test_get_waterfalls_live_testnet() {
        if !should_run_live_tests() {
            println!("Skipping live waterfalls test");
            return;
        }
        // Enterprise QuickSync (testnet). Requires ESPLORA_CLIENT_ID/SECRET and a
        // paid tier; signet is not offered by Blockstream QuickSync.
        let client = Client::new("https://enterprise.blockstream.info/testnet/api/").unwrap();
        let address = "tb1qrjywckd6n2j9nd0sg82w84mstuydt5fksd5szn";
        let result = client
            .get_query::<WaterfallResponse>(
                "waterfalls/v2/waterfalls",
                vec![("addresses".to_string(), address.to_string())],
            )
            .await;
        assert!(result.is_ok(), "API call failed: {:?}", result.err());
        let resp = result.unwrap();
        assert!(!resp.tip.is_empty());
    }
}
