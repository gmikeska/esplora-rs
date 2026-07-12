# Phase 2 — Integration: emvault → pkcs11 → groupvault → Shuttle

Carry the nodeless Esplora backend to a live Shuttle.dev signet deployment,
using **Blockstream Enterprise credentials** as the chain source. This phase
spans the emvault ecosystem; the authoritative cross-repo tracker is
`groupvault/docs/plans/esplora-shuttle-workstream.md`.

## Chain source: enterprise Esplora
- The existing **address-based** `esplora_sync` (gap-limit SPK scan) is correct
  and already live-proven on signet. Its only problem is public-endpoint
  throttling. **Authenticated enterprise Esplora has real rate limits**, so the
  same scan works at scale — no protocol change, and the wallet **descriptor
  never leaves the app** (privacy preserved).
- esplora-rs already has the enterprise constructor (`Client::new`, and after
  Phase 1 `with_credentials`). Once Phase 1 validates the auth path against real
  creds, wiring it downstream is a config change.

## 2a. emvault-core
- `EsploraBackend::new_enterprise(url, network)` already exists (reads
  `ESPLORA_CLIENT_ID/SECRET`); after Phase 1, prefer the explicit-credentials
  form. `esplora_sync` / `esplora_broadcast` are unchanged — they just run
  against the authenticated client.
- **Optional optimization (not required):** an incremental sync that scans only
  *revealed* SPKs instead of a full gap-limit scan on every call — cuts request
  volume and latency. Nice-to-have; enterprise limits make it non-urgent.

## 2b. test-app-pkcs11: finish validation on signet
- Run the full app on signet with enterprise creds:
  `APP_CHAIN_BACKEND=esplora`, `APP_ESPLORA_URL=<enterprise signet endpoint>`,
  `ESPLORA_CLIENT_ID` / `ESPLORA_CLIENT_SECRET`, `BITCOIN_NETWORK=signet`.
- Confirm: multi-wallet startup sync **without `429`s**, deposit shows (receive),
  and a spend broadcasts via `esplora_broadcast` (send) — fully nodeless.
- This closes the "finish checking the pkcs11 crate" item.
- Config note: enterprise creds must reach the emvault backend. Decide the wiring
  (env pass-through vs an `EsploraBackend::new_enterprise` arm in
  `WalletManager::new`). Keep it explicit.

## 2c. groupvault integration
- Point groupvault's sync/broadcast at the emvault Esplora backend (workstream
  doc Phase 2). A `ChainBackend` enum (rpc | esplora); async sync fits
  groupvault's axum model. Enterprise creds via config/secrets.

## 2d. Shuttle.dev deploy (signet)
- Nodeless is the whole point: Shuttle has no local `bitcoind`, so the chain
  backend **must** be HTTP Esplora. Deploy groupvault as a single same-origin
  service (axum `/api/*` + Dioxus WASM), `shuttle-shared-db` Postgres, and the
  **enterprise Esplora URL + creds** as Shuttle secrets.
- With authenticated Esplora, a full wallet sync stays within rate limits (the
  public-endpoint `429` bug does not apply).

## Sequencing & publish
1. esplora-rs **0.2.0** (Phase 1: structured errors, timeouts/backoff, explicit
   creds, validated enterprise auth) → crates.io.
2. emvault-core/emvault **0.3.0** consuming `esplora-rs = "0.2"` (drop the dev
   `[patch.crates-io]`).
3. test-app-pkcs11 repin + signet validation with enterprise creds.
4. groupvault integration → Shuttle deploy.

## Dev vs prod endpoints
- **Prod:** Blockstream Enterprise (authenticated) — the real target.
- **Dev without creds:** `mempool.space/signet` is the more permissive public
  option; `blockstream.info` public throttles hard. Both are stopgaps — once
  enterprise creds exist, use them throughout.
