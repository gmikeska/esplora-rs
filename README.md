# Esplora Client

A Rust client for the Blockstream Esplora API.

This client provides asynchronous access to the Esplora API. It supports both the public, unauthenticated API and the enterprise API, which requires authentication.

## Adding to Your Project

To use this client in your Rust project, add the following to your `Cargo.toml` file:

```toml
[dependencies]
esplora-rs = { git = "https://github.com/example/esplora-rs" } # Replace with the actual git repository URL
```

## Usage

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

Support for the enterprise API is currently under development. In the future, you will be able to create a client for the enterprise API like this:

```rust
// This is not yet fully supported and may change.
use esplora_rs::Client;

#[tokio::main]
async fn main() {
    // Set up the environment variables before running this
    // export ESPLORA_CLIENT_ID="your_client_id"
    // export ESPLORA_CLIENT_SECRET="your_client_secret"

    let client = Client::new("https://enterprise.blockstream.info/testnet/api/").unwrap();
    match client.get_tip_height().await {
        Ok(height) => println!("Current tip height: {}", height),
        Err(e) => eprintln!("Error: {}", e),
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
