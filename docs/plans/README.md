# esplora-rs — Work Plans

Phased plan for the ergonomics/robustness cleanup and the downstream nodeless
integration, using **Blockstream Enterprise credentials** end-to-end. Sequenced
so each phase lands green (fmt + pedantic clippy + tests) before the next.

## Direction (2026-07-12)

- **Enterprise Esplora, all the way through.** Greg is generating Blockstream
  Enterprise credentials. Authenticated Esplora has real (non-throttled) rate
  limits, so the existing **address-based** scan works at scale without the
  public-endpoint `429` problem.
- **Waterfalls — dev/staging only (revised 2026-07-12).** It leaks the wallet
  **descriptor** to the server, so it stays **out of production** (prod uses the
  **node**; plain Esplora is out of prod too). But dev/staging still need a
  nodeless chain source, and there the leak is acceptable (our server, test data)
  — and waterfalls' **one-query-per-descriptor** is the best fit precisely for the
  reduced query count. So it comes back, **scoped to dev/staging** (Phase 4).
- **Ergonomics is paramount** (per Greg): common calls are one-liners with clear,
  matchable errors — and the **enterprise-auth path must be first-class and
  tested** (it's currently untested end-to-end).

| Phase | Plan | Goal | Ships as |
|---|---|---|---|
| **1** | [`01-ergonomics-fixes.md`](01-ergonomics-fixes.md) | Fix extant ergonomics/robustness issues: **structured error type** exposing HTTP status (so `429`/`401` are matchable), timeouts, retry/backoff, an **explicit-credentials constructor**, and validating the **enterprise-auth path** against real creds. | esplora-rs **0.2.0** (breaking: `Error`) |
| **2** | [`02-integration-and-shuttle.md`](02-integration-and-shuttle.md) | Point `emvault-core`'s `esplora_sync` at **enterprise Esplora**, finish validating **test-app-pkcs11** on signet, then integrate **groupvault** and deploy to **Shuttle.dev** (signet). | emvault 0.3.0 → groupvault deploy |
| **4** | [`04-waterfalls-endpoint.md`](04-waterfalls-endpoint.md) | **Waterfalls client** (`/v2/waterfalls`): one descriptor query returns a wallet's full per-index history — the low-query-count **dev/staging** chain source. esplora-rs client + models only. | esplora-rs (additive) |

> **Preload / readiness-gate (dropped 2026-07-13).** An earlier Phase 3 proposed
> moving sync off the render path behind a boot-time readiness gate. Dropped at
> Greg's call — waterfalls makes the render-path sync cheap, and the Send/Federation
> tabs already skip their redundant sync. Not planned.

## Guiding constraints (all phases)
- **Keep the crate `bitcoin`-dependency-free.** Any typed conveniences go behind
  an optional, non-default feature. EmVault depends on this property.
- **Production chain source is the node** (privacy). Esplora — enterprise or
  waterfalls — is a **dev/staging** source. Among dev options: waterfalls
  (low query count, descriptor-based) is preferred; public address-based
  instances (blockstream.info / mempool.space) are rate-limited stopgaps.
- End each Rust cycle with `cargo fmt` + `cargo clippy --all-targets -- -D warnings
  -W clippy::pedantic -W rust-2018-idioms` (the emvault standard) + tests.

## Cross-references
- Crate design: [`../DESIGN.md`](../DESIGN.md)
- Broader workstream (emvault → groupvault → Shuttle):
  `groupvault/docs/plans/esplora-shuttle-workstream.md`
