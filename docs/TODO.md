# esplora-rs — TODO

Actionable checklist for the enterprise-Esplora initiative. Full rationale in
[`docs/DESIGN.md`](DESIGN.md) and [`docs/plans/`](plans/). Sequence:
ergonomics/robustness (**0.2.0**) → downstream integration
(emvault → pkcs11 → groupvault → Shuttle).

> **Deferred breaking changes (need the 0.2.0 version bump).** A non-breaking
> docs pass has landed (crate-level `//!` docs, README refresh, `# Errors`
> sections). The remaining **API-breaking** work is held for the version bump:
> - **E1 — structured `Error::Http { status, url, body }`** (replaces
>   `Error::Api(String)`); see *Errors & HTTP robustness* below.
> - **E4 — explicit `with_credentials(url, id, secret)` constructor** so callers
>   inject creds instead of `Client::new` reading them from the environment; see
>   *Auth / enterprise* below.
> Ship these together as **0.2.0** with the migration note.

### Done (docs pass, 2026-07-12)
- [x] Crate-level `//!` docs (overview, public/enterprise/waterfalls, env-var
      behavior + the app-vs-library split).
- [x] README refresh: real repo URL, corrected enterprise section, **Environment
      variables** + **Waterfalls/QuickSync** sections (E7).
- [x] `# Errors` doc sections on every public endpoint method (E8, docs half).

> **Auth note (confirmed 2026-07-12):** the enterprise OAuth flow — token URL
> `login.blockstream.com/realms/blockstream-public/protocol/openid-connect/token`,
> `grant_type=client_credentials`, `scope=openid`, form-encoded, JSON response —
> **matches esplora-rs's `fetch_token` exactly**, so `Client::new(base_url)` with
> `ESPLORA_CLIENT_ID` + `ESPLORA_CLIENT_SECRET` should authenticate as-is. Still
> needs a live end-to-end verification.

## Phase 1 — Ergonomics & robustness → esplora-rs 0.2.0

### Errors & HTTP robustness
- [ ] **E1 — Structured HTTP errors.** Replace `Error::Api(String)` on non-2xx with
      `Error::Http { status, url, body }` (+ `RateLimited { retry_after }` for 429).
      Update `get` / `get_plain` / `get_raw` / `broadcast_tx`. **(BREAKING → 0.2.0.)**
- [ ] **E2 — Request timeout.** Default reqwest timeout (~30s), configurable.
- [ ] **E3 — Retry/backoff** on 429/5xx (honor `Retry-After`); idempotent GETs only,
      **not** `broadcast_tx`.

### Auth / enterprise
- [ ] **E4 — Explicit-credentials constructor.** Public `with_credentials(url, id, secret)`
      (+ `with_credentials_and_token_url`) exposing the private `from_parts` logic;
      keep `new()` as the env convenience.
- [ ] **Validate enterprise auth end-to-end** against real creds (token fetch → `Bearer`
      on requests → expiry-aware refresh → 401/403 handling, all matchable via E1).
      Expected to work as-is (see auth note above); confirm on signet.
- [ ] Add a gated live auth smoke test.

### Smart auto-detecting constructor (new)
- [ ] **Auto-detecting constructor** — one entry point that picks enterprise vs
      public. Lives on emvault-core's `EsploraBackend::new(url, network)` (the
      `EsploraBackend::new_*` methods this replaces are there); **mirror** at the
      esplora-rs level as `Client::from_env(url)` / `Client::connect(url)` since
      `Client::new` is already the enterprise ctor. Rules:
      - Reads `ESPLORA_CLIENT_ID` / `ESPLORA_CLIENT_SECRET` (**treat empty string
        as absent**) plus a mode override `ESPLORA_AUTH_MODE = auto | public |
        enterprise` (default `auto`).
      - `auto`: creds present → enterprise, else public.
      - `public`: always public (even if creds are present).
      - `enterprise`: always enterprise; **error if creds are missing**
        (fail-closed until creds are inserted properly).
      - Implement as a convenience that **delegates to the explicit primitives**
        (`new_public` / `with_credentials`) so those stay pure and testable —
        keeps the env magic opt-in, consistent with **E4**.
- [ ] **Deprecation timing:** ship the new constructor for **≥ 1 release** before
      deprecating `new_public` / `new_enterprise` (don't deprecate same-version as
      the new API).

### Config / client surface
- [ ] **E5 — `ClientConfig` / `Client::builder()`** (timeout, max_retries, user_agent)
      without breaking `new_public` / `new`.
- [ ] **E9 — Set a `User-Agent`** for server-side identification/debugging.

### Docs / lints / release
- [ ] **E7 — Update README** (real repo URL, `esplora-rs = "0.2"`, public/enterprise
      usage matrix; remove the `github.com/example/…` placeholder).
- [ ] **E8 — `# Errors` doc sections** + adopt the pedantic clippy gate
      (`cargo clippy --all-targets -- -D warnings -W clippy::pedantic -W rust-2018-idioms`).
- [ ] Bump to **0.2.0**, note the breaking `Error` change, publish to crates.io.

### Deferred / optional (keep the crate `bitcoin`-dependency-free!)
- [ ] Typed `bitcoin`-feature conveniences (`Txid`/`Address`/`Transaction`, typed
      `broadcast`) — **non-default feature only**.
- [ ] E6 auto-paging `get_address_txs_all` convenience.

## Phase 2 — Integration (emvault → pkcs11 → groupvault → Shuttle)
- [ ] **emvault-core:** point `esplora_sync` / `esplora_broadcast` at enterprise Esplora;
      repin `esplora-rs = "0.2"`, drop the dev `[patch.crates-io]`; publish emvault 0.3.0.
- [ ] **test-app-pkcs11:** enterprise auto-detect wired (creds present → `new_enterprise`);
      run the full app on signet with enterprise creds — no 429s, receive + broadcast
      fully nodeless. Closes the pkcs11 validation.
- [ ] (optional) incremental sync (revealed SPKs only) to cut request volume/latency.
- [ ] **groupvault:** `ChainBackend` enum (rpc | esplora); enterprise creds via secrets.
- [ ] **Shuttle.dev signet deploy:** single same-origin service (axum `/api/*` + Dioxus
      WASM), `shuttle-shared-db` Postgres, enterprise URL + creds as Shuttle secrets.

## Done (2026-07-12)
- [x] Fix `broadcast_tx` (published 0.1.0 used the JSON `post()` helper on the
      plain-text txid → "invalid type: integer, expected a string").
- [x] Normalize base URL (`ensure_base_slash`) — `Url::join` trailing-slash trap
      (`…/api` + `tx` → `…/tx`).
- [x] Relicense GPL-2.0 → MIT OR Apache-2.0; add `repository`; bump 0.1.0 → **0.1.1**.
- [x] Tidy: `#[allow(dead_code)]` on the now-unused `post`; drop unused `warn` import.
- [x] Crate design doc + phased plans under `docs/`.
