# Phase 1 — Ergonomics & Robustness Fixes (esplora-rs 0.2.0)

Land the extant-issue fixes from the [ergonomics audit](../DESIGN.md#ergonomics-audit-extant-issues-2026-07-12).
The headline is a **structured error type** (so `429` rate-limits and `401` auth
failures are matchable), plus timeouts/retry-backoff and a **first-class,
tested enterprise-auth path** (Greg is providing real Blockstream Enterprise
creds — the production chain source). This is a **breaking** `Error` change, so
it's a `0.2.0`.

## Scope (in priority order)

### 1. Structured HTTP errors (E1 — the big one)
Replace the catch-all `Error::Api(String)` on non-2xx with a structured variant
that preserves status + context:
```rust
pub enum Error {
    // …existing…
    /// Non-2xx HTTP response from the Esplora server.
    #[error("HTTP {status} for {url}: {body}")]
    Http { status: u16, url: String, body: String },
    // optionally a convenience:
    /// Rate limited (HTTP 429). Carries any `Retry-After` seconds.
    #[error("rate limited (retry after {retry_after:?}s)")]
    RateLimited { retry_after: Option<u64> },
}
```
- In `get` / `get_plain` / `get_raw` / `broadcast_tx`, on `!status.is_success()`
  return `Error::Http { status, url, body }` (map `429` → `RateLimited` with
  parsed `Retry-After`), instead of `Error::Api(format!("HTTP {status}: {body}"))`.
- Keep `Error::Api(String)` for genuinely non-HTTP internal cases.
- **Downstream ripple:** `emvault-core`'s `EsploraSyncError::Http(#[from]
  esplora_rs::Error)` keeps working; but emvault can now *match* on
  `RateLimited`/`Http.status` to distinguish rate limits from real failures in
  logs and (future) retries.

### 2. Request timeout (E2)
Add a default timeout to the reqwest builder (e.g. 30 s, `const`), keep it
overridable via a client-config path. A stalled endpoint must not hang a server
request thread indefinitely.

### 3. Retry with backoff on 429/5xx (E3)
Bounded, opt-outable retry (e.g. 3 attempts, exponential backoff, honor
`Retry-After`). Applies to idempotent GETs; **do not** auto-retry `broadcast_tx`
(non-idempotent — return the error and let the caller decide). Config knob:
`max_retries` (default small).

### 4. Explicit-credentials constructor (E4)
Make the `from_parts` logic public and ergonomic:
```rust
pub fn with_credentials(base_url: &str, client_id: impl Into<String>,
                        client_secret: impl Into<String>) -> Result<Self, Error>;
pub fn with_credentials_and_token_url(/* + custom token_url */) -> Result<Self, Error>;
```
Keep `new()` (env-var convenience) but document that it reads
`ESPLORA_CLIENT_ID/SECRET`.

**Validate the enterprise-auth path end-to-end** against real Blockstream creds
(Greg providing): OAuth token fetch, `Authorization: Bearer` on requests,
expiry-aware refresh, and `401`/`403` handling (matchable via the structured
error). Enterprise auth is currently **untested** (DESIGN "Testing" gap) — add a
gated live auth smoke test. This is the production chain source, so it must be
solid.

### 5. Client config surface (supports 2–3)
Introduce a small `ClientConfig`/builder (timeout, max_retries, user_agent) or
`Client::builder()` — without breaking the existing `new_public`/`new`
constructors (they delegate to defaults). Set a `User-Agent` (E9).

### 6. Docs, lints, README (E7, E8)
- Add `# Errors` sections to public fns.
- Adopt the pedantic clippy gate in CI: `cargo clippy --all-targets -- -D
  warnings -W clippy::pedantic -W rust-2018-idioms`.
- Update README: real repo URL, `esplora-rs = "0.1"` (→ `0.2`), and a
  public/enterprise usage matrix. Remove the `github.com/example/…`
  placeholder.

### Deferred (optional, non-blocking)
- E5 typed `bitcoin`-feature conveniences (`Txid`/`Address`/`Transaction`,
  typed `broadcast`) — only if we want them; must be **non-default feature**.
- E6 auto-paging `get_address_txs_all` — optional convenience.

## Tests
- Extend the httpmock suite: assert `Error::Http { status: 429, .. }` /
  `RateLimited` is produced on a mocked 429; assert `Retry-After` parsing;
  assert retry-then-success and retry-exhaustion behavior; assert timeout error.
- Keep the 16 existing fixture tests green.

## Acceptance
- `cargo test` green; pedantic clippy + fmt clean.
- A downstream (emvault) can `match` on a rate-limit vs a 404.
- Version bumped to **0.2.0**; CHANGELOG/commit notes the breaking `Error` change.
- Publish order: cut **esplora-rs 0.2.0**, then emvault repins `esplora-rs =
  "0.2"` and drops the dev `[patch.crates-io]`.
