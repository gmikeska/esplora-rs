# Phase 3 — Preload & Readiness Gate (test-app-pkcs11)

**Philosophy (Greg, 2026-07-12):** *"I don't want the app to work until it's
ready to show data. I don't mind a bit of warm-up time as long as the app is
ready to use by the time the UI renders."*

Move the chain sync **off the render path** and **onto boot**: every wallet is
synced during startup, behind a readiness gate that serves a "warming up" splash
until data is ready. The first real page load is then instant — no 20-second
scan sitting between a click and a page.

This lives in **test-app-pkcs11** (not esplora-rs), but is tracked here with the
rest of the nodeless workstream. It builds on Phase-A incremental sync
(`emvault-core` `esplora_sync`: full-scan first, revealed-range after).

## Design decisions (settled)
- **Gate on the *scan*, not the HSMs.** Viewing needs the descriptor + chain
  sync; *signing* needs the HSMs. The descriptor is persisted after first boot
  (or derived from the HSMs on a fresh DB), so the sync gates readiness and the
  HSM connector finishes in the background — up long before anyone builds a
  spend.
- **All-wallets-ready granularity.** At 2–3 users, flip to `Ready` only when all
  seeded wallets have synced (simplest; revisit if the user count grows).
- **Login/splash exempt; data routes gated.** A user can reach the splash and the
  login page while the last wallet finishes; wallet/data routes 503 until `Ready`.
- **Backend-agnostic.** The warm-up calls `UserWallet::sync()`, which already
  dispatches rpc vs esplora via `APP_CHAIN_BACKEND`. Swapping to the node in prod
  needs no change here.
- **Thundering herd = non-issue at this scale** (2–3 users, paid 50k-query/mo
  enterprise tier). Still bound warm-up concurrency with a small semaphore as
  cheap insurance; not a hard requirement.

## Readiness model

```rust
// state.rs
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Readiness {
    Booting,                 // pre-seed
    Syncing { done: usize, total: usize },
    Ready,
    Degraded { failed: Vec<String> }, // wallet labels that never synced
}

pub struct AppState {
    // ...existing...
    pub readiness: tokio::sync::watch::Sender<Readiness>,
}
```

- `watch` because middleware/handlers can *read the latest* without polling, and
  a handler *could* `.await` the transition if we ever want request-hold instead
  of a splash.
- Per-wallet status tracked in a `Mutex<HashMap<WalletId, SyncStatus>>`
  (`Pending | Syncing | Synced | Failed{err}`) for an ops/debug view; the
  aggregate `Readiness` is derived from it.

## Work items

### P1 — Readiness state in `AppState`
- Add the `watch::Sender<Readiness>` (+ per-wallet status map) to `AppState`
  (`state.rs`). Seed it `Booting`.

### P2 — Boot warm-up task (`main.rs`)
- After `seed_test_wallets` (which already builds each `UserWallet` — the
  expensive cryptoki derivation), **before** `axum::serve`:
  1. Set `Readiness::Syncing { done: 0, total }`.
  2. For each seeded wallet, run `uw.sync().await` behind a bounded semaphore
     (`SCAN_CONCURRENCY` ~= 4), with **retry/backoff** (3 tries, exp backoff,
     honoring the structured `RateLimited { retry_after }` once esplora-rs 0.2.0
     lands — until then, fixed backoff on any error).
  3. Update per-wallet status + bump `done`.
  4. When all succeed → `Ready`. If any wallet exhausts retries → `Degraded`
     with the failed labels (do **not** flip `Ready` with missing data).
- The Elements ingest task already runs in the background; leave it as-is (it's
  its own push loop, not on the render path).
- **Fresh-DB note:** seeding derives descriptors from the HSMs first, so on a
  fresh DB the warm-up naturally sequences after HSM derivation. On a warm boot
  the descriptors load from the DB and the sync runs concurrently with the HSM
  connector — exactly the intended overlap.

### P3 — Tower readiness gate (splash)
- A small `axum::middleware::from_fn_with_state` layer wrapping the **data**
  routes:
  - If `*readiness.borrow() != Ready` → return `503` + a lightweight HTML splash
    ("Warming up wallets…", meta-refresh ~1s). On `Degraded`, show the failed
    wallet labels instead of an infinite spinner.
  - Exempt: `/healthz` (liveness — always 200), the splash asset, `/login`,
    static assets.
- Add `/readyz` returning 200 only when `Ready` (for Shuttle/other readiness
  probes later).

### P4 — Take sync off the render path
- Remove the per-request `uw.sync().await?` from the 5 handler sites
  (`handlers/wallet.rs:292, 374, 431, 467, 550`). Handlers read straight from
  persisted BDK state (instant).
- Replace with **freshness elsewhere**:
  - A background `tokio::interval` re-sync loop (e.g. every 30–60s) per wallet,
    same semaphore + retry as warm-up. (This is the steady-state analogue of the
    boot warm-up.)
  - Optional explicit **"Refresh"** button → on-demand `sync()` (the only
    user-triggered sync), plus the future **"Rescan"** button → `esplora_rescan`.
- Net: the render path never blocks on the network; warm-up + interval keep data
  fresh; manual refresh/rescan cover the edge cases.

### P5 — Status/observability
- Log warm-up start/finish per wallet + total elapsed.
- Surface `Readiness`/per-wallet status on an ops page or `/readyz` body so a
  failed background sync is **visible**, never silently swallowed.

## Acceptance
- Cold start: hitting any wallet route during warm-up shows the splash, never a
  half-synced/empty wallet; once `Ready`, the same route renders instantly with
  correct balances.
- Repeat loads never issue a render-path sync (verify: no esplora request in the
  logs on page load; only the interval loop + explicit refresh do).
- A deliberately-failed backend (bad URL) ends in `Degraded` with the wallet
  labelled — boot does not hang and does not falsely report `Ready`.
- `cargo fmt` + `cargo clippy --all-targets -- -D warnings -W clippy::pedantic
  -W rust-2018-idioms` clean; existing handler tests updated for the no-sync
  render path.

## Not in scope
- esplora-rs changes (none needed; this is pure app wiring).
- Per-user lazy readiness / partial gating — deferred until the user count makes
  all-wallets-ready too coarse.
- groupvault: the same gate carries over when groupvault gets its axum warm-up
  (workstream doc Phase 2d / Shuttle); mirror this design there.

## Cross-references
- Incremental sync it builds on: `emvault-core/src/esplora_sync.rs`
  (`esplora_sync` dispatcher → `esplora_rescan` / `esplora_incremental`).
- Structured `RateLimited` error that upgrades the retry/backoff:
  [`01-ergonomics-fixes.md`](01-ergonomics-fixes.md) (E1/E3).
- Broader workstream: `groupvault/docs/plans/esplora-shuttle-workstream.md`.
