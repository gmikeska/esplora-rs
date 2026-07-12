# esplora-rs — Crate Design

_Status: living document. Last updated 2026-07-12._

## Purpose

`esplora-rs` is an async Rust client for the **Blockstream Esplora HTTP API**
(the REST API documented at
<https://github.com/Blockstream/esplora/blob/master/API.md>). It targets three
deployment shapes with one client type:

1. **Public, unauthenticated** instances (e.g. `blockstream.info`, `mempool.space`).
2. **Blockstream Enterprise** (OAuth bearer tokens via client-credentials).
3. **Self-hosted** Esplora servers.

It is the chain-access layer for the EmVault custody stack: `emvault-core`'s
optional `esplora` feature wraps this client to sync BDK wallets and broadcast
transactions **without a local `bitcoind`** (see `emvault-core/src/esplora_sync.rs`).

## Design principles

1. **Bitcoin-dependency-free core.** The public API speaks `String`/`&str` and
   `serde` DTOs — it does **not** depend on the `bitcoin` crate. This is a
   deliberate, load-bearing choice: it lets any downstream (on any `bitcoin`
   major) graft the client with zero version-conflict risk. EmVault relies on
   exactly this. Typed conveniences, if added, must stay behind an **optional,
   non-default feature** so the dep-free property is preserved.
2. **One client, three auth modes.** `Client::new_public` (no auth),
   `Client::new` (enterprise, env creds), and (proposed) an explicit-credentials
   constructor. Auth is handled transparently per request via [`auth::Auth`].
3. **Thin, faithful endpoint mapping.** Each method maps to one documented
   Esplora endpoint, returning a typed DTO from `models.rs` or a primitive
   (`String`/`Bytes`).
4. **Ergonomics first for the common path.** The 90% call — "give me this
   address's history / this tx / broadcast this hex" — should be a one-liner
   with clear errors.

## Architecture

```
src/
  lib.rs      # Client: constructors, request helpers (get/get_plain/get_raw/post),
              #         and one method per Esplora endpoint.
  auth.rs     # Auth: public (no-op) or enterprise (OAuth client-credentials,
              #       token fetch + expiry-aware caching).
  error.rs    # Error: reqwest / url / serde / auth / env / generic API errors.
  models.rs   # serde DTOs: Block, Transaction, Vin/Vout/Prevout, TxStatus,
              #             Utxo, AddressInfo/Stats, Outspend, Mempool, FeeEstimates,
              #             asset (Elements) types, RecentTx, ...
  testdata/   # JSON fixtures used by the httpmock-based unit tests.
```

### Request pipeline
- `base_url: url::Url` + `Url::join(path)` per call. **Base URLs are normalized
  to a trailing slash** (`ensure_base_slash`) so `join` appends rather than
  replacing the last path segment (`…/api` + `tx` → `…/api/tx`, not `…/tx`).
- The HTTP client is built `.http1_only()` to avoid HTTP/2 POST quirks on some
  servers/CDNs.
- Private helpers: `get` (JSON), `get_plain` (text, e.g. hex/txid), `get_raw`
  (bytes), `post` (JSON body — currently unused; `broadcast_tx` posts raw text).

### Auth (enterprise)
- `Auth::new(client_id, client_secret, token_url)` fetches a bearer token via
  OAuth `client_credentials` (`grant_type=client_credentials`, `scope=openid`)
  and caches it with an expiry buffer. `get_token()` returns `Ok(None)` for the
  public (unauthenticated) client, so the request helpers add an
  `Authorization: Bearer` header only when authenticated.
- **Untested end-to-end** (no live enterprise creds yet) — flagged as a known
  risk; see plans.

## Public API surface (current)

Blocks: `get_block`, `get_block_header`, `get_block_status`, `get_block_txids`,
`get_block_txid_at_index`, `get_raw_block`, `get_block_hash_from_height`,
`get_blocks`, `get_tip_hash`, `get_tip_height`, `get_block_txs`.

Transactions: `get_tx`, `get_tx_status`, `get_tx_hex`, `get_raw_tx`,
`get_tx_merkle_block_proof`, `get_outspend`, `get_outspends`, **`broadcast_tx`**.

Addresses / scripts: `get_address_info`, `get_scripthash_info`,
`get_address_txs`, `get_address_txs_chain` (paginated), `get_address_mempool_txs`,
`get_address_utxos`, `search_addresses`.

Mempool: `get_mempool_info`, `get_mempool_txids`, `get_mempool_recent_txs`.

Fees: `get_fee_estimates`.

Elements/Liquid assets: `get_asset_info`, `get_asset_txs`, `get_asset_txs_chain`,
`get_asset_mempool_txs`, `get_asset_supply`, `get_asset_supply_decimal`.

All documented Esplora REST endpoints are covered. Scaling the address-based
scan to production is a matter of using **authenticated enterprise Esplora**
(real rate limits), not adding new endpoints — see
[`plans/01-ergonomics-fixes.md`](plans/01-ergonomics-fixes.md) for the
first-class, tested enterprise-auth path.

## Consumers

- **`emvault-core` (`esplora` feature)** → `EsploraBackend` + `esplora_sync` /
  `esplora_broadcast`. `esplora_sync` currently does a **per-address gap-limit
  full scan** (dozens of `get_address_info` + history + tx fetches per wallet).
  This is correct but **request-heavy**: public endpoints throttle it (observed
  2026-07-12: `blockstream.info` public signet returns `429` after ~10 sequential
  requests; `mempool.space` is more permissive for dev). The production fix is
  **authenticated enterprise Esplora** (real rate limits); an optional
  incremental sync (only revealed SPKs, vs full-scan-every-time) further reduces
  request volume.
- **`test-app-pkcs11`** selects the backend via `APP_CHAIN_BACKEND=rpc|esplora`.
- **groupvault** (planned) → Shuttle.dev signet deployment.

## Ergonomics audit (extant issues, 2026-07-12)

Ranked by impact. Actionable items are tracked in
[`plans/01-ergonomics-fixes.md`](plans/01-ergonomics-fixes.md).

- **E1 — Coarse error type hides HTTP status (HIGH).** Non-2xx responses collapse
  into `Error::Api(String)`. Callers cannot distinguish `429` (rate-limited) from
  `404`/`400` without scraping the message. This directly cost debugging time on
  the pkcs11 app (429s surfaced only as a generic "esplora HTTP request failed").
  → Add a structured `Error::Http { status, url, body }` (and/or an explicit
  `RateLimited`) so callers can match and back off.
- **E2 — No request timeout (MEDIUM).** The `reqwest` client has no default
  timeout; a stalled endpoint hangs forever. → Set a sane, configurable default.
- **E3 — No retry/backoff on 429/5xx (MEDIUM).** Public instances throttle;
  bounded retry-with-backoff honoring `Retry-After` would make the client robust
  by default. Depends on E1.
- **E4 — Enterprise `Client::new` reads env vars implicitly (MEDIUM).** Reading
  `ESPLORA_CLIENT_ID/SECRET` inside the constructor is surprising and hard to
  test. → Expose an explicit `Client::with_credentials(url, id, secret)` (the
  existing private `from_parts`), keep `new()` as the env convenience.
- **E5 — String-typed API pushes parsing onto callers (DESIGN TRADE-OFF).** This
  is what keeps the crate `bitcoin`-free (a feature, not a bug). → If we add
  typed helpers (`Txid`, `Address`, `Transaction`, typed `broadcast`), gate them
  behind an **optional `bitcoin` feature**, never the default.
- **E6 — Manual pagination (LOW).** `get_address_txs_chain` requires a caller
  loop. → Optional auto-paging convenience.
- **E7 — Stale README (LOW).** Placeholder git URL (`github.com/example/…`),
  predates crates.io. → Update with real repo, `esplora-rs = "0.1"`, and a
  public/enterprise matrix.
- **E8 — Docs + lint gate (LOW).** Public fns lack `# Errors` sections; no
  pedantic clippy gate. → Add `# Errors` docs + adopt the emvault clippy standard
  (`-D warnings -W clippy::pedantic -W rust-2018-idioms`).
- **E9 — No `User-Agent` (LOW).** Set a UA for server-side identification/debug.

## Testing

- Unit tests use `httpmock` against the `src/testdata/*.json` fixtures (16 tests
  currently green).
- **Gap:** no live-network integration tests and no enterprise-auth test. Live
  coverage today lives downstream in `emvault-core/tests/esplora_*_signet.rs`
  (gated on `ESPLORA_LIVE_TEST=1`). Consider mirroring a gated live smoke test
  here.

## Versioning / distribution

- Published on crates.io. Current: **0.1.1** (`MIT OR Apache-2.0`).
- SemVer discipline: the String-DTO surface is the public contract; adding
  new endpoints/DTOs is additive (minor bump). A structured `Error`
  change (E1) is **breaking** for any code matching on `Error::Api` — batch it
  into a `0.2.0` with the other ergonomics fixes.
