# Phase 4 — Waterfalls descriptor-scan endpoint (esplora-rs)

Add a `/v2/waterfalls` client to esplora-rs: **one descriptor query returns a
wallet's entire per-index history**, collapsing the ~80 per-address requests of
an address-based scan to ~1. This is the low-query-count chain source for the
**dev/staging** pipeline.

## Scope & rationale (2026-07-12)

- **Production uses the node** (privacy for clients). Plain Esplora is out of prod
  too; **waterfalls is out of prod** as well — it sends the wallet **descriptor**
  to the server, leaking the whole future address space (the same reason we drop
  it for groupvault prod).
- **But dev/staging still need a nodeless chain source**, and there the
  descriptor-leak is acceptable (our own server, test data). Waterfalls is the
  best fit *precisely because of the reduced query count* — it removes the
  render-path latency and 429/rate pressure that motivated incremental sync +
  preload in the first place.
- **This phase = esplora-rs client only.** The bespoke emvault-core sync
  (`esplora_waterfalls_sync`) and the pkcs11 wiring are **Phase 5** (below).
  Preload/readiness is deferred and restored later.

## The endpoint (confirmed live against Blockstream QuickSync, 2026-07-12)

```
GET https://enterprise.blockstream.info/<chain>/api/waterfalls/v2/waterfalls?descriptor=<url-encoded>&to_index=<n>&page=<n>
Authorization: Bearer <oauth-token>
```
`<chain>`: `testnet`, `liquid`, `liquidtestnet`; **mainnet omits the segment**.
The route sits **under `/api`** (`…/api/waterfalls/v2/waterfalls`), *not*
`…/api/v2/waterfalls` — with an esplora base of `…/<chain>/api/`, the relative
path is **`waterfalls/v2/waterfalls`** (resolves the base-URL gotcha).

- **✅ Host reality (probed live 2026-07-12) — the account is paid & works:**
  - `enterprise.blockstream.info/testnet` and `/api` (mainnet), authed → **200**,
    `X-Credits-Remaining` decrements (Basic tier: QuickSync 10 credits/call,
    500k credits). A `?addresses=…` call returned a valid v2 body (see fixture).
  - `enterprise.blockstream.info/signet`, authed → **402 Payment Required** — and
    a plain `signet/api/blocks/tip/height` also 402s. **Signet is simply not
    offered** on Blockstream QuickSync (their docs list mainnet/testnet/liquid/
    liquidtestnet — no signet), *not* a billing problem.
  - `blockstream.info/*` (public) → **404** `endpoint does not exist` (public host
    never serves waterfalls; standard esplora works there).
  → **Decision needed — the app targets signet, which QuickSync doesn't cover:**
  1. **Run the waterfalls dev path on `testnet`** (works now, zero infra, cheap
     credits). Nodeless, so no testnet node required. *Recommended.*
  2. **Self-host `Blockstream/waterfalls` for signet** (open source, `new_public`)
     to keep signet parity with the local node — infra to run.
  The esplora-rs client is the same either way; only base URL + whether a Bearer
  is sent differ.
- **Server:** Blockstream QuickSync, built on the open-source
  [`Blockstream/waterfalls`](https://github.com/Blockstream/waterfalls) server.
- **Privacy:** the descriptor goes to Blockstream's server, so this is
  **dev/staging** (fine to leak test descriptors) — **unless** the descriptor is
  **age-encrypted with the server's public key** (a documented option: the server
  scans without seeing the plaintext descriptor). Encryption is the future lever
  if we ever want waterfalls closer to prod; **out of scope for the dev impl** —
  start with the plaintext descriptor.
- **Version:** target **v2** — it returns `block_hash` + `block_timestamp` per
  sighting, so the bespoke sync builds BDK anchors **without** a follow-up per-tx
  status call.

### Request params (from the QuickSync docs)
- **`descriptor`** (string) — the wallet's **public multipath** descriptor with
  wildcard, e.g. `wsh(sortedmulti(2,[fp/48h/1h/0h/2h]xpub…/<0;1>/*,…))#cksum` —
  exactly what emvault's `to_multipath_string` produces. **Must be
  percent-encoded** (contains `#`, `(`, `)`, `[`, `]`, `/`, `*`, `<`, `;`, `>`).
  Mutually exclusive with `addresses`. Network-validated (mainnet xpub rejected
  on testnet, etc.).
- **`addresses`** (string) — comma-separated addresses; alternative to
  `descriptor`. Not needed for our wallet sync (we have the descriptor).
- **`to_index`** (int, optional) — **max derivation index the server scans.
  Default `0`** → *you must set this* (e.g. the wallet's revealed index + gap,
  or a fixed cap like 100) or the scan barely covers index 0.
- **`page`** (int, optional, default `0`) — pagination.
- **`utxo_only`** (bool, optional, default `false`) — balance-only mode (UTXOs,
  no full history). A future fast-balance path; the sync wants full history.
- Response format is JSON (default) or CBOR — we use JSON (`ACCEPT: application/json`).

### Response schema (confirmed)
```jsonc
{
  "txs_seen": {
    // one key PER SINGLE-PATH descriptor branch — a multipath `<0;1>` request
    // returns separate `…/0/*` and `…/1/*` keys, each: outer index = derivation
    // index, inner array = txs at that index.
    "wsh(sortedmulti(2,…/0/*))#cksum": [
      [ { "txid": "..", "height": 830000,
          "block_hash": "..", "block_timestamp": 1710000000, "v": 1 } ],
      [], ...
    ]
  },
  "page": 0,
  "tip": "<blockhash-hex>"
}
```
- `height == 0` (or absent block) = unconfirmed/mempool → no anchor, treat as seen-at.
- `v` appears in some responses (a per-sighting version marker) — model as optional.
- **Bitcoin-free DTOs only** (crate invariant): every field is a `String`/integer;
  no `bitcoin`/`bdk` types.

## esplora-rs additions

### `models.rs`
```rust
/// One `/v2/waterfalls` sighting of a tx at a derivation index.
#[derive(Debug, Clone, Deserialize)]
pub struct TxSeen {
    pub txid: String,
    /// Block height; `0` (or absent block) = unconfirmed.
    pub height: i64,
    #[serde(default)]
    pub block_hash: Option<String>,
    #[serde(default)]
    pub block_timestamp: Option<u64>,
    /// Per-sighting version marker present in some responses.
    #[serde(default)]
    pub v: Option<u32>,
}

/// `/v2/waterfalls` response: per-descriptor, per-index tx history in one call.
#[derive(Debug, Clone, Deserialize)]
pub struct WaterfallResponse {
    /// Keyed by (single-path) descriptor; outer Vec index = derivation index.
    pub txs_seen: std::collections::BTreeMap<String, Vec<Vec<TxSeen>>>,
    pub page: u16,
    /// Chain tip block hash at scan time (hex).
    pub tip: String,
}
```
Re-export both from `lib.rs` (`pub use models::{TxSeen, WaterfallResponse, …}`).

### `lib.rs` — a query-aware GET + the methods
`get()` takes a bare path and `Url::join`s it; a raw descriptor would corrupt the
URL (`#` → fragment, `?`/`&`/`+` mis-parsed). Add a query-encoding helper so the
`url` crate percent-encodes the value:

```rust
async fn get_query<T: DeserializeOwned>(
    &self, path: &str, params: &[(&str, &str)],
) -> Result<T, Error> {
    let mut url = self.base_url.join(path)?;
    url.query_pairs_mut().extend_pairs(params.iter().copied());
    // …identical token / ACCEPT json / send / status-check / json() as `get()`…
}
```

Public API (path is **`waterfalls/v2/waterfalls`** relative to the `…/<chain>/api/` base):
```rust
/// One page of `waterfalls/v2/waterfalls` for a descriptor, scanning derivation
/// indices `0..=to_index`.
pub async fn get_waterfalls(&self, descriptor: &str, to_index: u32, page: u16)
    -> Result<WaterfallResponse, Error> {
    self.get_query("waterfalls/v2/waterfalls", &[
        ("descriptor", descriptor),
        ("to_index", &to_index.to_string()),
        ("page", &page.to_string()),
    ]).await
}

/// Page through the scan and merge into one response
/// (loops until a page adds no new indices; hard cap on iterations).
pub async fn get_waterfalls_all(&self, descriptor: &str, to_index: u32)
    -> Result<WaterfallResponse, Error> { /* merge txs_seen across pages */ }
```

## Implementation gotchas
- **Base-URL alignment — RESOLVED.** Route is `…/<chain>/api/waterfalls/v2/waterfalls`,
  i.e. **under `/api`**. With base `…/<chain>/api/`, the relative path is
  `waterfalls/v2/waterfalls` — `Url::join` appends cleanly (given the
  `ensure_base_slash` trailing slash already in place). No separate waterfalls base.
- **`to_index` is required in practice.** Default `0` → the server barely scans.
  Pass the wallet's revealed index + a gap (or a fixed cap). The downstream sync
  decides the value.
- **Descriptor checksum.** Send the descriptor *with* its `#checksum` (some
  versions require it). `to_multipath_string` output already includes it.
- **Multipath key-splitting.** A `<0;1>` request returns **two** `txs_seen`
  keys (`…/0/*`, `…/1/*`); the sync must map each key back to its keychain.
- **Pagination termination.** "Done" = a page whose `txs_seen` windows are all
  empty (or `page` stops advancing); cap iterations against a misbehaving server.

## Auth
- **Enterprise host → `Client::new`** (OAuth Bearer). Confirmed live: the
  QuickSync endpoint requires the token; esplora-rs's existing token
  fetch/refresh (`ESPLORA_CLIENT_ID` / `ESPLORA_CLIENT_SECRET` →
  `login.blockstream.com/...`) works as-is. Each call decrements
  `X-Credits-Remaining` (surface it if we ever want budget visibility).
- Base URL: **`https://enterprise.blockstream.info/<chain>/api`** (e.g.
  `…/testnet/api`). ⚠️ `<chain>` must be a QuickSync-supported network
  (**testnet/mainnet/liquid/liquidtestnet — not signet**).
- A self-hosted `Blockstream/waterfalls` instance would instead be `new_public`
  (no Bearer); same client, only the base URL + token presence change.

## Errors / robustness
- Fold into the Phase 1 structured-error work: a `400` (bad descriptor) or `404`
  (server without waterfalls) must surface its status so the caller can tell
  "unsupported endpoint" from "bad input." Until E1 lands, `Error::Api("HTTP {status}: …")`
  is acceptable.

## Deps / feature gating
- **No new deps** (`reqwest` + `serde` + `url` already present), **no `bitcoin`
  types** → ships in the default surface, no feature flag.

## Tests
- **Offline (always-on):** deserialize a captured JSON fixture into
  `WaterfallResponse` — locks the schema, no network.
- **Gated live (`WATERFALLS_LIVE_TEST=1`):** hit the enterprise **testnet**
  endpoint (`https://enterprise.blockstream.info/testnet/api`, `Client::new` +
  `ESPLORA_CLIENT_ID/SECRET`) with a descriptor/address that has known testnet
  history; assert `txs_seen` parses and `get_waterfalls_all` terminates. Confirmed
  reachable 2026-07-12 (200, credits decrement).
- **Real sample (fixture seed)** — an `?addresses=…` call returned:
  ```json
  {"txs_seen":{"addresses":[[]]},"page":0,
   "tip":"00000000000000bcef2b46cddb481fd75a834f2272d58226c097892538660c36"}
  ```
  (empty history for the queried address; note the key is literally `addresses`
  for the address form, and the descriptor form keys by the single-path
  descriptor string.)
- End with `cargo fmt` + `cargo clippy --all-targets -- -D warnings
  -W clippy::pedantic -W rust-2018-idioms`.

## Downstream — Phase 5 (not this phase)
- **emvault-core `esplora_waterfalls_sync(wallet, backend)`** — the bespoke sync:
  pull the wallet's public multipath descriptor → `get_waterfalls_all` → from each
  `(index, TxSeen)` derive BDK `last_active_indices` + anchors (height/hash/time
  straight from v2, no extra calls) → fetch raw tx hex per **unique** txid
  (`get_tx_hex`) → assemble `TxUpdate` + chain update → `apply_update`. Net: **one
  descriptor query + K tx-hex fetches** vs ~80 address queries. This is the
  "bespoke sync run" — a third sibling to `esplora_sync` (address) and
  `emitter_sync` (node), behind the same `SyncResult` seam.
- **pkcs11:** add a `Waterfalls` chain-backend mode (env-selected) that routes
  `UserWallet::sync` to `esplora_waterfalls_sync`. With sync now a single fast
  query, the render-path sync is cheap again — **restore preload afterward** as a
  clean layer on top, not a prerequisite.

## Acceptance
- `get_waterfalls("<testnet descriptor>", to_index, 0)` returns parsed history in
  **one** request against `enterprise.blockstream.info/testnet/api`;
  `get_waterfalls_all` merges pages and terminates; offline fixture test green;
  fmt + pedantic clippy clean.

## References
- Blockstream QuickSync: <https://help.blockstream.com/blockstream-explorer-api/more/understanding-and-accessing-quicksync>
- Waterfalls server + API: <https://github.com/Blockstream/waterfalls> (`docs/API.md`)
- Enterprise auth (OAuth): <https://help.blockstream.com/blockstream-explorer-api/use-explorer-api/make-a-rest-api-request-with-your-api-keys>
