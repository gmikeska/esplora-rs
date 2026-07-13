# Esplora Client

A Rust client for the Blockstream Esplora API.

This client provides asynchronous access to the Esplora API. It supports both the public, unauthenticated API and the enterprise API, which requires authentication.

Implements all endpoints documented at https://github.com/Blockstream/esplora/blob/master/API.md, plus the enterprise **Waterfalls / QuickSync** descriptor-scan endpoint.

The crate is deliberately `bitcoin`-dependency-free (String/int DTOs), so it composes with any downstream `bitcoin`/`bdk` version.

## Adding to Your Project

To use this client in your Rust project, add the following to your `Cargo.toml` file:

```toml
[dependencies]
esplora-rs = { git = "https://github.com/gmikeska/esplora-rs" }
```

## Usage

The client is initialized with the base URL of the Esplora instance you want to use. You can create a client for a public API or an enterprise API.

### Public API

Here's a simple example of how to create a client for the public API and get the current tip height of the testnet blockchain:

```rust
use esplora_rs::Client;

#[tokio::main]
async fn main() {
    let client = Client::new_public("https://blockstream.info/testnet/api/").unwrap();
    match client.get_tip_height().await {
        Ok(height) => println!("Current tip height: {}", height),
        Err(e) => eprintln!("Error: {}", e),
    }
}
```

### Enterprise API

`Client::new` targets an authenticated **enterprise** instance. It performs the
OAuth `client_credentials` flow against Blockstream's token endpoint and sends a
`Bearer` token on every request. The credentials are **read from the process
environment** (see [Environment variables](#environment-variables) below):

```rust
use esplora_rs::Client;

#[tokio::main]
async fn main() {
    // export ESPLORA_CLIENT_ID="your_client_id"
    // export ESPLORA_CLIENT_SECRET="your_client_secret"

    let client = Client::new("https://enterprise.blockstream.info/testnet/api/").unwrap();
    match client.get_tip_height().await {
        Ok(height) => println!("Current tip height: {}", height),
        Err(e) => eprintln!("Error: {}", e),
    }
}
```

### Environment variables

Only the **enterprise credentials** are read from the environment (by
`Client::new`). The base URL is always an explicit argument.

| Variable | Read by | Purpose |
|---|---|---|
| `ESPLORA_CLIENT_ID` | `Client::new` | OAuth `client_id` |
| `ESPLORA_CLIENT_SECRET` | `Client::new` | OAuth `client_secret` |
| `ESPLORA_TEST_LIVE` | test suite | set to `live` to run the crate's live tests |

> Note: any `APP_*` variables (e.g. `APP_ESPLORA_URL`) belong to the *program
> using* this crate, not to esplora-rs — that program reads them and passes the
> URL in as `base_url`. An explicit `with_credentials(url, id, secret)`
> constructor (so callers can inject creds instead of relying on env) is planned
> — see [`docs/TODO.md`](docs/TODO.md) item **E4**.

### Waterfalls / QuickSync (descriptor scan)

`get_waterfalls` / `get_waterfalls_all` hit `<base>/waterfalls/v2/waterfalls`:
one query returns a whole wallet's per-index history from a **descriptor**,
instead of walking addresses one by one. This is a Blockstream **enterprise**
feature (mainnet / testnet / liquid / liquidtestnet — not signet).

```rust
use esplora_rs::Client;

#[tokio::main]
async fn main() {
    // enterprise creds in env (see above)
    let client = Client::new("https://enterprise.blockstream.info/testnet/api/").unwrap();
    let descriptor = "wpkh([00000000/84h/1h/0h]tpub.../<0;1>/*)#checksum".to_string();
    // scan derivation indices 0..=50, paging until exhausted
    let history = client.get_waterfalls_all(descriptor, 50).await.unwrap();
    println!("tip {}, {} branch(es)", history.tip, history.txs_seen.len());
}
```

See the [waterfalls server](https://github.com/Blockstream/waterfalls) for the
protocol and the age-encrypted-descriptor option.

### Bitcoin Endpoints

#### Get Block

This example shows how to get a block by its hash.

**Request:**
```rust
use esplora_rs::Client;

#[tokio::main]
async fn main() {
    let client = Client::new_public("https://blockstream.info/testnet/api/").unwrap();
    let block_hash = "00000000000000000005930aa4894de96644480436473138535038e9e4933eb9";
    match client.get_block(block_hash).await {
        Ok(block) => println!("{:#?}", block),
        Err(e) => eprintln!("Error: {}", e),
    }
}
```

**Response:**
```json
{
    "id": "00000000000000000005930aa4894de96644480436473138535038e9e4933eb9",
    "height": 600000,
    "version": 536870912,
    "timestamp": 1573135017,
    "tx_count": 2369,
    "size": 1369324,
    "weight": 3991660,
    "merkle_root": "a2e53369b54d4e9dd1472598336341f53a5e8f49fa6911c43f145a38535038e9",
    "previousblockhash": "00000000000000000002f2f3f93a02e97b85353e3fb059424e8d2e85e4933eb9",
    "nonce": 0,
    "bits": 402690119
}
```

#### Get Transaction

This example shows how to get a transaction by its ID.

**Request:**
```rust
use esplora_rs::Client;

#[tokio::main]
async fn main() {
    let client = Client::new_public("https://blockstream.info/testnet/api/").unwrap();
    let txid = "f5de79f0312d803666e3a83f12423cc5825227ee055c56f2d2b58a1d741f8713";
    match client.get_tx(txid).await {
        Ok(tx) => println!("{:#?}", tx),
        Err(e) => eprintln!("Error: {}", e),
    }
}
```

**Response:**
```json
{
  "txid": "f5de79f0312d803666e3a83f12423cc5825227ee055c56f2d2b58a1d741f8713",
  "version": 2,
  "locktime": 0,
  "vin": [
    {
      "txid": "f4184fc596403b9d638783cf57adfe4c75c605f6356fbc91338530e9831e9e16",
      "vout": 0,
      "prevout": {
        "scriptpubkey": "0014f6b5212642a8b9e83693e5b382d6a6c561763c0a",
        "scriptpubkey_asm": "OP_0 OP_PUSHBYTES_20 f6b5212642a8b9e83693e5b382d6a6c561763c0a",
        "scriptpubkey_type": "v0_p2wpkh",
        "scriptpubkey_address": "bc1q8q2h3lg5h2z59w2z59w2z59w2z59w2z59w2z5",
        "value": 100000
      },
      "scriptsig": "",
      "scriptsig_asm": "",
      "witness": [
        "30440220202020202020202020202020202020202020202020202020202020202020202002202020202020202020202020202020202020202020202020202020202020202020",
        "03a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3"
      ],
      "is_coinbase": false,
      "sequence": 4294967295
    }
  ],
  "vout": [
    {
      "scriptpubkey": "0014f6b5212642a8b9e83693e5b382d6a6c561763c0a",
      "scriptpubkey_asm": "OP_0 OP_PUSHBYTES_20 f6b5212642a8b9e83693e5b382d6a6c561763c0a",
      "scriptpubkey_type": "v0_p2wpkh",
      "scriptpubkey_address": "bc1q8q2h3lg5h2z59w2z59w2z59w2z59w2z59w2z5",
      "value": 90000
    }
  ],
  "size": 223,
  "weight": 556,
  "fee": 10000,
  "status": {
    "confirmed": true,
    "block_height": 2000000,
    "block_hash": "0000000000000034a3646d53e345e8284835d88e07c875104a371343f76d3ba0",
    "block_time": 1609459200
  }
}
```

#### Get Address Info

This example shows how to get information about a Bitcoin address.

**Request:**
```rust
use esplora_rs::Client;

#[tokio::main]
async fn main() {
    let client = Client::new_public("https://blockstream.info/testnet/api/").unwrap();
    let address = "tb1qxdjp5w4y7449cm5qensttdeauzlxquqtr289ql";
    match client.get_address_info(address).await {
        Ok(info) => println!("{:#?}", info),
        Err(e) => eprintln!("Error: {}", e),
    }
}
```

**Response:**
```json
{
  "address": "tb1qxdjp5w4y7449cm5qensttdeauzlxquqtr289ql",
  "chain_stats": {
    "tx_count": 10,
    "funded_txo_count": 5,
    "funded_txo_sum": 500000,
    "spent_txo_count": 3,
    "spent_txo_sum": 300000
  },
  "mempool_stats": {
    "tx_count": 1,
    "funded_txo_count": 1,
    "funded_txo_sum": 100000,
    "spent_txo_count": 0,
    "spent_txo_sum": 0
  }
}
```

#### Get Fee Estimates

This example shows how to get fee estimates.

**Request:**
```rust
use esplora_rs::Client;

#[tokio::main]
async fn main() {
    let client = Client::new_public("https://blockstream.info/api/").unwrap();
    match client.get_fee_estimates().await {
        Ok(estimates) => println!("{:#?}", estimates),
        Err(e) => eprintln!("Error: {}", e),
    }
}
```

**Response:**
```json
{
  "1": 87.882,
  "2": 87.882,
  "3": 87.882,
  "4": 87.882,
  "5": 81.129,
  "6": 68.285,
  "144": 1.027,
  "504": 1.027,
  "1008": 1.027
}
```

### Liquid Endpoints

The Liquid api offers the same endpoints as are illustrated for Bitcoin above, plus asset endpoints.

#### Get Asset Info

This example shows how to get information about a Liquid asset.

**Request:**
```rust
use esplora_rs::Client;

#[tokio::main]
async fn main() {
    let client = Client::new_public("https://blockstream.info/liquid/api/").unwrap();
    let asset_id = "d8a317ce2c14241192cbb3ebdb9696250ca1251a58ba6251c29fcfe126c9ca1f";
    match client.get_asset_info(asset_id).await {
        Ok(asset_info) => println!("{:#?}", asset_info),
        Err(e) => eprintln!("Error: {}", e),
    }
}
```

**Response:**
```json
{
  "asset_id": "d8a317ce2c14241192cbb3ebdb9696250ca1251a58ba6251c29fcfe126c9ca1f",
  "issuance_txin": {
    "txid": "39affca34bd51ed080f89f1e7a5c7a49d6d9e4779c84424ae50df67dd60dcaf7",
    "vin": 0
  },
  "issuance_prevout": {
    "txid": "0cdd74c540af637d5a3874ce8500891fd8e94ec8e3d5d436d86e87b6759a7674",
    "vout": 0
  },
  "reissuance_token": "eb8b210d42566699796dbf78649120fd5c9d9b04cabc8f480856e04bd5e9fc22",
  "contract_hash": "025d983cc774da665f412ccc6ccf51cb017671c2cb0d3c32d10d50ffdf0a57de",
  "status": {
    "confirmed": true,
    "block_height": 105,
    "block_hash": "7bf84f2aea30b02981a220943f543a6d6e7ac646d59ef76cff27dca8d27b2b67",
    "block_time": 1586248729
  },
  "chain_stats": {
    "tx_count": 1,
    "issuance_count": 1,
    "issued_amount": 0,
    "burned_amount": 0,
    "has_blinded_issuances": true,
    "reissuance_tokens": 0,
    "burned_reissuance_tokens": 0
  },
  "mempool_stats": {
    "tx_count": 0,
    "issuance_count": 0,
    "issued_amount": 0,
    "burned_amount": 0,
    "has_blinded_issuances": false,
    "reissuance_tokens": null,
    "burned_reissuance_tokens": 0
  },
  "ticker": "TEST",
  "name": "Test Asset",
  "precision": 8,
  "entity": {
    "domain": "test.com"
  }
}
```

## Running the Tests

The test suite includes both mocked tests and live tests that interact with the Blockstream API.

- The mocked tests can be run with `cargo test -- --all-targets`.
- The live tests for the public API can be run by setting the `ESPLORA_TEST_LIVE` environment variable to `live`:

```bash
export ESPLORA_TEST_LIVE=live
cargo test
```

Running the enterprise API tests requires a valid set of credentials and is not recommended at this time.
