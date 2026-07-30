# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.3.0] - 2026-07-30

### Fixed

- **Waterfalls `v: -1` spend sentinel no longer 500s the `/v2` response.**
  Waterfalls emits `"v": -1` on spend/input sightings. `TxSeen.v` was typed
  `Option<u32>`, so the sentinel failed to deserialize (`invalid value: integer
  -1, expected u32`) and the **entire** `/v2` body was rejected for any address
  with a spent output. `TxSeen.v` is now `Option<i64>`. The client only reads
  `.txid` and the sighting's index position — never `.v` — so widening the field
  is zero blast radius.

### Changed

- **BREAKING:** `TxSeen.v` widened from `Option<u32>` to `Option<i64>`.
- **BREAKING:** `Block::version` and `Transaction::version` widened from `u32` to
  `i32`. Bitcoin's `nVersion` is a signed `int32` (rust-bitcoin's `Version` wraps
  `i32`), so Esplora can surface a negative version — the same bug class as the
  waterfalls sentinel. Nothing downstream reads `.version`.

### Added

- Regression test `test_waterfalls_spend_sighting_negative_v_parses`, backed by a
  live-captured `/v2` fixture (`src/testdata/waterfalls_v2_spend.json`) carrying
  the `v: -1` spend sighting alongside a confirmed funding sighting.
- Regression test `test_negative_version_parses`, decoding a `Block` and a
  `Transaction` with a negative `version`.

## [0.2.0] - 2026-07-13

### Added

- Waterfalls `/v2/waterfalls` descriptor-scan client, example, and docs.
- Comprehensive test suite and structured error types.

[0.3.0]: https://github.com/gmikeska/esplora-rs/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/gmikeska/esplora-rs/releases/tag/v0.2.0
