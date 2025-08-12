use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Block {
    pub id: String,
    pub height: u64,
    pub version: u32,
    pub timestamp: u64,
    pub tx_count: u64,
    pub size: u64,
    pub weight: u64,
    pub merkle_root: String,
    pub previousblockhash: Option<String>,
    pub nonce: u32,
    pub bits: u32,
    // Note: difficulty is not in the API doc format section, but is in the example.
    // It's better to add it as optional or check if it's always present.
    // For now, I will omit it and can add it later if needed.
}

#[derive(Debug, Clone, Deserialize)]
pub struct BlockStatus {
    pub in_best_chain: bool,
    pub next_best: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TxStatus {
    pub confirmed: bool,
    pub block_height: Option<u64>,
    pub block_hash: Option<String>,
    pub block_time: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Prevout {
    pub scriptpubkey: String,
    pub scriptpubkey_asm: String,
    pub scriptpubkey_type: String,
    pub scriptpubkey_address: Option<String>,
    pub value: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Vin {
    pub txid: String,
    pub vout: u32,
    pub prevout: Option<Prevout>,
    pub scriptsig: String,
    pub scriptsig_asm: String,
    pub witness: Option<Vec<String>>,
    pub is_coinbase: bool,
    pub sequence: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Vout {
    pub scriptpubkey: String,
    pub scriptpubkey_asm: String,
    pub scriptpubkey_type: String,
    pub scriptpubkey_address: Option<String>,
    pub value: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Transaction {
    pub txid: String,
    pub version: u32,
    pub locktime: u32,
    pub vin: Vec<Vin>,
    pub vout: Vec<Vout>,
    pub size: u64,
    pub weight: u64,
    pub fee: u64,
    pub status: TxStatus,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Outspend {
    pub spent: bool,
    pub txid: Option<String>,
    pub vin: Option<u32>,
    pub status: Option<TxStatus>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Stats {
    pub tx_count: u64,
    pub funded_txo_count: u64,
    pub funded_txo_sum: Option<u64>,
    pub spent_txo_count: u64,
    pub spent_txo_sum: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AddressInfo {
    pub address: String,
    pub chain_stats: Stats,
    pub mempool_stats: Stats,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Utxo {
    pub txid: String,
    pub vout: u32,
    pub status: TxStatus,
    pub value: u64,
    // Elements-specific fields
    pub asset: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Mempool {
    pub count: u64,
    pub vsize: u64,
    pub total_fee: u64,
    pub fee_histogram: Vec<(f64, f64)>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RecentTx {
    pub txid: String,
    pub fee: u64,
    pub vsize: u64,
    pub value: u64,
}

pub type FeeEstimates = std::collections::HashMap<String, f64>;

#[derive(Debug, Clone, Deserialize)]
pub struct AssetStats {
    pub tx_count: u64,
    // Native asset stats
    pub peg_in_count: Option<u64>,
    pub peg_in_amount: Option<u64>,
    pub peg_out_count: Option<u64>,
    pub peg_out_amount: Option<u64>,
    pub burn_count: Option<u64>,
    pub burned_amount: Option<u64>,
    // User-issued asset stats
    pub issuance_count: Option<u64>,
    pub issued_amount: Option<u64>,
    pub has_blinded_issuances: Option<bool>,
    pub reissuance_tokens: Option<u64>,
    pub burned_reissuance_tokens: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AssetIssuanceTxin {
    pub txid: String,
    pub vin: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AssetIssuancePrevout {
    pub txid: String,
    pub vout: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AssetEntity {
    pub domain: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AssetInfo {
    pub asset_id: String,
    pub issuance_txin: Option<AssetIssuanceTxin>,
    pub issuance_prevout: Option<AssetIssuancePrevout>,
    pub reissuance_token: Option<String>,
    pub contract_hash: Option<String>,
    pub status: Option<TxStatus>,
    pub chain_stats: AssetStats,
    pub mempool_stats: AssetStats,
    // From asset registry
    pub ticker: Option<String>,
    pub name: Option<String>,
    pub precision: Option<u8>,
    pub entity: Option<AssetEntity>,
}
