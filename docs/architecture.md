# Architecture

`cordex` is a modular Discord v10 bot API library on Tokio. Every piece is a
separate crate that works on its own; `cordex` (in `crates/facade`) re-exports
them behind feature flags and adds the simple layer (`cordex::bot`).

## Crate dependency graph

```text
cordex-model  <──  cordex-http
      ^                  ^
cordex-gateway     cordex-interactions
      ^                  ^
cordex-cache (model only)
      ^
cordex (facade, optional features)
```

Design boundaries (enforced by `cargo xtask layout-check`):

- **`cordex-model`** has no async runtime and no networking dependencies. It
  contains snowflakes (`Id<T>`), bitflags (`Permissions`, `Intents`, ...),
  typed REST/Gateway models, Components V2 builders, CDN URLs, mention
  helpers, and the pure permission calculator.
- **`cordex-http`** depends only on `cordex-model`. It implements the Hyper
  1.x REST client (`rustls` + `aws-lc-rs` + platform verifier), per-route and
  global rate limiting, the Cloudflare invalid-request guard, retries,
  multipart uploads, local validation, and all 235 typed endpoints.
- **`cordex-gateway`** depends on `cordex-model` and talks to `cordex-http`
  only through a small trait (`GatewayBotSource`) for `GET /gateway/bot`. It
  implements shards, `zlib-stream` decompression, heartbeats, resume/reconnect,
  backpressure, panic isolation, `ShardManager`, `IdentifyQueue`, `Spawner`,
  and `Standby`.
- **`cordex-interactions`** depends on `cordex-model` and `cordex-http`. It
  provides command/modal builders with local validation, typed option access,
  `InteractionHandle` (auto-defer, double-response guard), Ed25519 signature
  verification (`verify` feature), and a standalone Hyper HTTP endpoint
  (`server` feature).
- **`cordex-cache`** depends only on `cordex-model`. It has no network access
  and is completely opt-in: every resource (`ResourceType`, with `CachePolicy`
  `Off` / `Lazy` / `Eager`), every optional field group (`CacheParts`), and
  every limit is its own switch, changeable at runtime via `set_config`.
- **`cordex`** (facade) re-exports the crates behind features (`http`,
  `gateway`, `interactions`, `bot`, `cache`, `interactions-verify`,
  `interactions-server`), provides a single `Error` / `Result`, a `prelude`,
  `cordex::bot` (simple layer), and `cordex::backfill`.

## Two layers, one code path

- **Simple layer (`cordex::bot`):** `Bot::new(token)` derives intents and the
  gateway event filter from your registered handlers, shards automatically,
  reshards live when Discord recommends more shards, registers commands on
  start (only when changed), auto-defers slow interactions, and isolates
  handler errors and panics.
- **Full layer:** Direct access to `Shard`, `ShardManager`, `Client`,
  `Route::raw`, `InMemoryCache`, `Standby`, and `InteractionEndpoint`. The
  simple layer is built directly on top of the full layer.

## Core invariants

- **No `unsafe`:** `unsafe_code = "forbid"` in every workspace crate.
- **No panics on network input:** `unwrap_used` and `expect_used` are denied
  in library code; shard runs are supervised behind a panic guard.
- **Unknown data survives:** No `#[serde(deny_unknown_fields)]`; enums have
  `Unknown(...)` variants; unknown gateway events become `Event::Unknown`;
  bitflags use `from_bits_retain`.
- **No hidden caching:** Only `cordex-cache` stores Discord objects. Gateway
  keeps only protocol state (session ID, sequence, resume URL, pending member
  chunk requests); HTTP keeps only rate-limit buckets and invalid-request
  timestamps.
- **No global allocator:** Library crates never set a global allocator;
  applications and examples may use `mimalloc`.
