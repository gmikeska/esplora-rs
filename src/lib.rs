use std::env;

pub mod auth;
pub mod error;
pub mod models;

pub use auth::Auth;
pub use error::Error;
pub use models::{
    AddressInfo, AssetInfo, Block, BlockStatus, FeeEstimates, Mempool, Outspend, RecentTx,
    Transaction, TxStatus, Utxo,
};

use bytes::Bytes;
use reqwest::header::AUTHORIZATION;
use reqwest::Client as ReqwestClient;
use url::Url;

const DEFAULT_TOKEN_URL: &str = "https://login.blockstream.com/realms/blockstream-public/protocol/openid-connect/token";

#[derive(Debug, Clone)]
pub struct Client {
    http_client: ReqwestClient,
    base_url: Url,
    auth: Auth,
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

    /// Creates a new Esplora client from its constituent parts. Useful for testing.
    fn from_parts(
        base_url: &str,
        token_url: Url,
        client_id: String,
        client_secret: String,
    ) -> Result<Self, Error> {
        let auth = Auth::new(client_id, client_secret, token_url);
        let http_client = ReqwestClient::new();
        let base_url = Url::parse(base_url)?;

        Ok(Self {
            http_client,
            base_url,
            auth,
        })
    }

    async fn get<T: serde::de::DeserializeOwned>(&self, path: &str) -> Result<T, Error> {
        let token = self.auth.get_token().await?;
        let url = self.base_url.join(path)?;

        let response = self
            .http_client
            .get(url)
            .header(AUTHORIZATION, format!("Bearer {}", token))
            .send()
            .await?
            .error_for_status()?;

        Ok(response.json().await?)
    }

    async fn post<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        body: String,
    ) -> Result<T, Error> {
        let token = self.auth.get_token().await?;
        let url = self.base_url.join(path)?;

        let response = self
            .http_client
            .post(url)
            .header(AUTHORIZATION, format!("Bearer {}", token))
            .body(body)
            .send()
            .await?
            .error_for_status()?;

        Ok(response.json().await?)
    }

     async fn get_plain(&self, path: &str) -> Result<String, Error> {
        let token = self.auth.get_token().await?;
        let url = self.base_url.join(path)?;

        let response = self
            .http_client
            .get(url)
            .header(AUTHORIZATION, format!("Bearer {}", token))
            .send()
            .await?
            .error_for_status()?;

        Ok(response.text().await?)
    }

    async fn get_raw(&self, path: &str) -> Result<Bytes, Error> {
        let token = self.auth.get_token().await?;
        let url = self.base_url.join(path)?;

        let response = self
            .http_client
            .get(url)
            .header(AUTHORIZATION, format!("Bearer {}", token))
            .send()
            .await?
            .error_for_status()?;

        Ok(response.bytes().await?)
    }

    // Blocks
    pub async fn get_block(&self, hash: &str) -> Result<Block, Error> {
        self.get(&format!("block/{}", hash)).await
    }

    pub async fn get_block_header(&self, hash: &str) -> Result<String, Error> {
        self.get_plain(&format!("block/{}/header", hash)).await
    }

    pub async fn get_block_status(&self, hash: &str) -> Result<BlockStatus, Error> {
        self.get(&format!("block/{}/status", hash)).await
    }

    pub async fn get_block_txids(&self, hash: &str) -> Result<Vec<String>, Error> {
        self.get(&format!("block/{}/txids", hash)).await
    }

    pub async fn get_block_txid_at_index(&self, hash: &str, index: u64) -> Result<String, Error> {
        self.get_plain(&format!("block/{}/txid/{}", hash, index)).await
    }

    pub async fn get_raw_block(&self, hash: &str) -> Result<Bytes, Error> {
        self.get_raw(&format!("block/{}/raw", hash)).await
    }

    pub async fn get_block_hash_from_height(&self, height: u64) -> Result<String, Error> {
        self.get_plain(&format!("block-height/{}", height)).await
    }

    pub async fn get_blocks(&self, start_height: Option<u64>) -> Result<Vec<Block>, Error> {
        let path = if let Some(height) = start_height {
            format!("blocks/{}", height)
        } else {
            "blocks".to_string()
        };
        self.get(&path).await
    }

    pub async fn get_tip_hash(&self) -> Result<String, Error> {
        self.get_plain("blocks/tip/hash").await
    }

    pub async fn get_tip_height(&self) -> Result<u64, Error> {
        let height_str = self.get_plain("blocks/tip/height").await?;
        height_str
            .parse::<u64>()
            .map_err(|e| Error::Api(format!("Failed to parse height: {}", e)))
    }

    pub async fn get_block_txs(&self, hash: &str, start_index: Option<u64>) -> Result<Vec<Transaction>, Error> {
        let path = if let Some(start) = start_index {
            format!("block/{}/txs/{}", hash, start)
        } else {
            format!("block/{}/txs", hash)
        };
        self.get(&path).await
    }

    // Transactions
    pub async fn get_tx(&self, txid: &str) -> Result<Transaction, Error> {
        self.get(&format!("tx/{}", txid)).await
    }

    pub async fn get_tx_status(&self, txid: &str) -> Result<TxStatus, Error> {
        self.get(&format!("tx/{}/status", txid)).await
    }

    pub async fn get_tx_hex(&self, txid: &str) -> Result<String, Error> {
        self.get_plain(&format!("tx/{}/hex", txid)).await
    }

    pub async fn get_raw_tx(&self, txid: &str) -> Result<Bytes, Error> {
        self.get_raw(&format!("tx/{}/raw", txid)).await
    }

    pub async fn get_tx_merkle_block_proof(&self, txid: &str) -> Result<String, Error> {
        self.get_plain(&format!("tx/{}/merkleblock-proof", txid)).await
    }

    pub async fn get_outspend(&self, txid: &str, vout: u32) -> Result<Outspend, Error> {
        self.get(&format!("tx/{}/outspend/{}", txid, vout)).await
    }

    pub async fn get_outspends(&self, txid: &str) -> Result<Vec<Outspend>, Error> {
        self.get(&format!("tx/{}/outspends", txid)).await
    }

    pub async fn broadcast_tx(&self, tx_hex: &str) -> Result<String, Error> {
        self.post("tx", tx_hex.to_string()).await
    }

    // Addresses
    pub async fn get_address_info(&self, address: &str) -> Result<AddressInfo, Error> {
        self.get(&format!("address/{}", address)).await
    }

    pub async fn get_scripthash_info(&self, hash: &str) -> Result<AddressInfo, Error> {
        self.get(&format!("scripthash/{}", hash)).await
    }

    pub async fn get_address_txs(&self, address: &str) -> Result<Vec<Transaction>, Error> {
        self.get(&format!("address/{}/txs", address)).await
    }

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

    pub async fn get_address_mempool_txs(&self, address: &str) -> Result<Vec<Transaction>, Error> {
        self.get(&format!("address/{}/txs/mempool", address)).await
    }

    pub async fn get_address_utxos(&self, address: &str) -> Result<Vec<Utxo>, Error> {
        self.get(&format!("address/{}/utxo", address)).await
    }

    pub async fn search_addresses(&self, prefix: &str) -> Result<Vec<String>, Error> {
        self.get(&format!("address-prefix/{}", prefix)).await
    }

    // Mempool
    pub async fn get_mempool_info(&self) -> Result<Mempool, Error> {
        self.get("mempool").await
    }

    pub async fn get_mempool_txids(&self) -> Result<Vec<String>, Error> {
        self.get("mempool/txids").await
    }

    pub async fn get_mempool_recent_txs(&self) -> Result<Vec<RecentTx>, Error> {
        self.get("mempool/recent").await
    }

    // Fee Estimates
    pub async fn get_fee_estimates(&self) -> Result<FeeEstimates, Error> {
        self.get("fee-estimates").await
    }

    // Assets
    pub async fn get_asset_info(&self, asset_id: &str) -> Result<AssetInfo, Error> {
        self.get(&format!("asset/{}", asset_id)).await
    }

    pub async fn get_asset_txs(&self, asset_id: &str) -> Result<Vec<Transaction>, Error> {
        self.get(&format!("asset/{}/txs", asset_id)).await
    }

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

    pub async fn get_asset_mempool_txs(&self, asset_id: &str) -> Result<Vec<Transaction>, Error> {
        self.get(&format!("asset/{}/txs/mempool", asset_id)).await
    }

    pub async fn get_asset_supply(&self, asset_id: &str) -> Result<u64, Error> {
        let supply_str = self.get_plain(&format!("asset/{}/supply", asset_id)).await?;
        supply_str.parse::<u64>().map_err(|e| Error::Api(format!("Failed to parse supply: {}", e)))
    }

    pub async fn get_asset_supply_decimal(&self, asset_id: &str) -> Result<f64, Error> {
        let supply_str = self.get_plain(&format!("asset/{}/supply/decimal", asset_id)).await?;
        supply_str.parse::<f64>().map_err(|e| Error::Api(format!("Failed to parse decimal supply: {}", e)))
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
            when.method(POST)
                .path("/token");
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

    #[tokio::test]
    async fn test_get_block_live() {
        if !should_run_live_tests() {
            println!("Skipping live test for get_block");
            return;
        }
        let client = Client::new("https://enterprise.blockstream.info/testnet/api").unwrap();
        // A known testnet block
        let block_hash = "0000000000000034a3646d53e345e8284835d88e07c875104a371343f76d3ba0";
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
        let client = Client::new("https://enterprise.blockstream.info/testnet/api").unwrap();
        let result = client.get_tip_height().await;
        assert!(result.is_ok(), "API call failed: {:?}", result.err());
        assert!(result.unwrap() > 2_000_000, "Testnet height should be over 2M");
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
        let client = Client::new("https://enterprise.blockstream.info/testnet/api").unwrap();
        // A known testnet transaction
        let txid = "e1bfa234b5c178342323c2153297a9b0498a445e434d3137e1b8581a1e41131c";
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
        let client = Client::new("https://enterprise.blockstream.info/testnet/api").unwrap();
        // A known testnet address with some history
        let address = "tb1qg398h9k5j2zgjfgz0w6py5k23z5d5x4m4q0z0h";
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
        let client = Client::new("https://enterprise.blockstream.info/testnet/api").unwrap();
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
        let client = Client::new("https://enterprise.blockstream.info/liquidtestnet/api").unwrap();
        // L-BTC asset ID for liquid testnet
        let asset_id = "144c654344aa716d6f3abcc1ca90e5641e4e2a7f633bc09fe3baf64585819a49";
        let result = client.get_asset_info(asset_id).await;

        assert!(result.is_ok(), "API call failed: {:?}", result.err());
        let info = result.unwrap();
        assert_eq!(info.asset_id, asset_id);
    }
}
