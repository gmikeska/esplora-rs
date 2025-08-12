use serde::Deserialize;

/// Represents a Bitcoin block.
#[derive(Debug, Clone, Deserialize)]
pub struct Block {
    /// The block hash.
    pub id: String,
    /// The block height.
    pub height: u64,
    /// The block version.
    pub version: u32,
    /// The block timestamp.
    pub timestamp: u64,
    /// The number of transactions in the block.
    pub tx_count: u64,
    /// The block size in bytes.
    pub size: u64,
    /// The block weight in weight units.
    pub weight: u64,
    /// The Merkle root of the block's transactions.
    pub merkle_root: String,
    /// The hash of the previous block.
    pub previousblockhash: Option<String>,
    /// The block nonce.
    pub nonce: u32,
    /// The block bits.
    pub bits: u32,
}

/// Represents the confirmation status of a block.
#[derive(Debug, Clone, Deserialize)]
pub struct BlockStatus {
    /// Whether the block is in the best chain.
    pub in_best_chain: bool,
    /// The hash of the next block in the best chain.
    pub next_best: Option<String>,
}

/// Represents the confirmation status of a transaction.
#[derive(Debug, Clone, Deserialize)]
pub struct TxStatus {
    /// Whether the transaction is confirmed.
    pub confirmed: bool,
    /// The height of the block that contains the transaction.
    pub block_height: Option<u64>,
    /// The hash of the block that contains the transaction.
    pub block_hash: Option<String>,
    /// The timestamp of the block that contains the transaction.
    pub block_time: Option<u64>,
}

/// Represents a previous output of a transaction input.
#[derive(Debug, Clone, Deserialize)]
pub struct Prevout {
    /// The script public key.
    pub scriptpubkey: String,
    /// The assembly representation of the script public key.
    pub scriptpubkey_asm: String,
    /// The type of the script public key.
    pub scriptpubkey_type: String,
    /// The address associated with the script public key.
    pub scriptpubkey_address: Option<String>,
    /// The value of the output in satoshis.
    pub value: u64,
}

/// Represents a transaction input.
#[derive(Debug, Clone, Deserialize)]
pub struct Vin {
    /// The ID of the transaction that contains the input.
    pub txid: String,
    /// The index of the input in the transaction.
    pub vout: u32,
    /// The previous output being spent.
    pub prevout: Option<Prevout>,
    /// The script signature.
    pub scriptsig: String,
    /// The assembly representation of the script signature.
    pub scriptsig_asm: String,
    /// The witness data for the input.
    pub witness: Option<Vec<String>>,
    /// Whether the input is a coinbase input.
    pub is_coinbase: bool,
    /// The sequence number of the input.
    pub sequence: u32,
}

/// Represents a transaction output.
#[derive(Debug, Clone, Deserialize)]
pub struct Vout {
    /// The script public key.
    pub scriptpubkey: String,
    /// The assembly representation of the script public key.
    pub scriptpubkey_asm: String,
    /// The type of the script public key.
    pub scriptpubkey_type: String,
    /// The address associated with the script public key.
    pub scriptpubkey_address: Option<String>,
    /// The value of the output in satoshis.
    pub value: u64,
}

/// Represents a Bitcoin transaction.
#[derive(Debug, Clone, Deserialize)]
pub struct Transaction {
    /// The transaction ID.
    pub txid: String,
    /// The transaction version.
    pub version: u32,
    /// The transaction locktime.
    pub locktime: u32,
    /// The list of transaction inputs.
    pub vin: Vec<Vin>,
    /// The list of transaction outputs.
    pub vout: Vec<Vout>,
    /// The transaction size in bytes.
    pub size: u64,
    /// The transaction weight in weight units.
    pub weight: u64,
    /// The transaction fee in satoshis.
    pub fee: u64,
    /// The confirmation status of the transaction.
    pub status: TxStatus,
}

/// Represents the spending status of a transaction output.
#[derive(Debug, Clone, Deserialize)]
pub struct Outspend {
    /// Whether the output is spent.
    pub spent: bool,
    /// The ID of the transaction that spent the output.
    pub txid: Option<String>,
    /// The index of the input that spent the output.
    pub vin: Option<u32>,
    /// The confirmation status of the spending transaction.
    pub status: Option<TxStatus>,
}

/// Represents statistics for an address.
#[derive(Debug, Clone, Deserialize)]
pub struct Stats {
    /// The total number of transactions for the address.
    pub tx_count: u64,
    /// The number of funded transaction outputs.
    pub funded_txo_count: u64,
    /// The sum of all funded transaction outputs.
    pub funded_txo_sum: Option<u64>,
    /// The number of spent transaction outputs.
    pub spent_txo_count: u64,
    /// The sum of all spent transaction outputs.
    pub spent_txo_sum: Option<u64>,
}

/// Represents information about an address.
#[derive(Debug, Clone, Deserialize)]
pub struct AddressInfo {
    /// The address.
    pub address: String,
    /// The chain-level statistics for the address.
    pub chain_stats: Stats,
    /// The mempool-level statistics for the address.
    pub mempool_stats: Stats,
}

/// Represents an unspent transaction output (UTXO).
#[derive(Debug, Clone, Deserialize)]
pub struct Utxo {
    /// The transaction ID of the UTXO.
    pub txid: String,
    /// The output index of the UTXO.
    pub vout: u32,
    /// The confirmation status of the UTXO.
    pub status: TxStatus,
    /// The value of the UTXO in satoshis.
    pub value: u64,
    /// The asset ID of the UTXO (for Elements-based chains).
    pub asset: Option<String>,
}

/// Represents information about the mempool.
#[derive(Debug, Clone, Deserialize)]
pub struct Mempool {
    /// The number of transactions in the mempool.
    pub count: u64,
    /// The total size of the mempool in virtual bytes.
    pub vsize: u64,
    /// The total fee of all transactions in the mempool.
    pub total_fee: u64,
    /// The fee histogram of the mempool.
    pub fee_histogram: Vec<(f64, f64)>,
}

/// Represents a recent transaction in the mempool.
#[derive(Debug, Clone, Deserialize)]
pub struct RecentTx {
    /// The transaction ID.
    pub txid: String,
    /// The transaction fee in satoshis.
    pub fee: u64,
    /// The transaction size in virtual bytes.
    pub vsize: u64,
    /// The transaction value in satoshis.
    pub value: u64,
}

/// A map of confirmation targets to fee estimates.
pub type FeeEstimates = std::collections::HashMap<String, f64>;

/// Represents statistics for an asset.
#[derive(Debug, Clone, Deserialize)]
pub struct AssetStats {
    /// The number of transactions for the asset.
    pub tx_count: u64,
    // Native asset stats
    /// The number of peg-in transactions.
    pub peg_in_count: Option<u64>,
    /// The total amount of pegged-in assets.
    pub peg_in_amount: Option<u64>,
    /// The number of peg-out transactions.
    pub peg_out_count: Option<u64>,
    /// The total amount of pegged-out assets.
    pub peg_out_amount: Option<u64>,
    /// The number of burn transactions.
    pub burn_count: Option<u64>,
    /// The total amount of burned assets.
    pub burned_amount: Option<u64>,
    // User-issued asset stats
    /// The number of issuance transactions.
    pub issuance_count: Option<u64>,
    /// The total amount of issued assets.
    pub issued_amount: Option<u64>,
    /// Whether the asset has had any blinded issuances.
    pub has_blinded_issuances: Option<bool>,
    /// The number of reissuance tokens.
    pub reissuance_tokens: Option<u64>,
    /// The number of burned reissuance tokens.
    pub burned_reissuance_tokens: Option<u64>,
}

/// Represents the transaction input of an asset issuance.
#[derive(Debug, Clone, Deserialize)]
pub struct AssetIssuanceTxin {
    /// The transaction ID of the issuance.
    pub txid: String,
    /// The input index of the issuance.
    pub vin: u32,
}

/// Represents the previous output of an asset issuance.
#[derive(Debug, Clone, Deserialize)]
pub struct AssetIssuancePrevout {
    /// The transaction ID of the previous output.
    pub txid: String,
    /// The output index of the previous output.
    pub vout: u32,
}

/// Represents the entity associated with an asset.
#[derive(Debug, Clone, Deserialize)]
pub struct AssetEntity {
    /// The domain of the entity.
    pub domain: String,
}

/// Represents information about an asset.
#[derive(Debug, Clone, Deserialize)]
pub struct AssetInfo {
    /// The asset ID.
    pub asset_id: String,
    /// The transaction input of the issuance.
    pub issuance_txin: Option<AssetIssuanceTxin>,
    /// The previous output of the issuance.
    pub issuance_prevout: Option<AssetIssuancePrevout>,
    /// The reissuance token for the asset.
    pub reissuance_token: Option<String>,
    /// The contract hash for the asset.
    pub contract_hash: Option<String>,
    /// The confirmation status of the asset issuance.
    pub status: Option<TxStatus>,
    /// The chain-level statistics for the asset.
    pub chain_stats: AssetStats,
    /// The mempool-level statistics for the asset.
    pub mempool_stats: AssetStats,
    // From asset registry
    /// The ticker symbol for the asset.
    pub ticker: Option<String>,
    /// The name of the asset.
    pub name: Option<String>,
    /// The precision of the asset.
    pub precision: Option<u8>,
    /// The entity associated with the asset.
    pub entity: Option<AssetEntity>,
}
