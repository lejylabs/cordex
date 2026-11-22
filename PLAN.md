# <lib> – Rust Discord Library (working title, name not decided)
## Project Plan & Technical Specification

| | |
|---|---|
| **Project** | Independent Discord bot API library for Rust |
| **Primary consumer** | ZenyBot (the library itself stays free of ZenyBot logic) |
| **Language / Runtime** | Rust (stable), Tokio |
| **Discord API version** | v10 (REST and Gateway) |
| **Voice** | Explicitly out of scope |
| **Plan status** | Spec for v0.1.0; open items at the end of §24 |

The words **MUST**, **MUST NOT**, **SHOULD** and **MAY** are used as in RFC 2119. Every MUST in this document has to be covered by a test or a review checklist item before v0.1.0 ships (see §22).

---

# 0. Fixed Decisions

These decisions are final for v0.1.0. Changing one requires updating this table and the affected sections.

| # | Topic | Decision | Reason |
|---|---|---|---|
| D1 | Project / crate name | **Open.** `<lib>` is a placeholder in this plan; all crates will be prefixed `<lib>-`. Must be decided in Phase 0 before any crate is created. Constraint: no "Discord" in the name (Discord brand guidelines). | One name across repo, crates and docs. |
| D2 | API version | v10 only, constant in `<lib>-model` | One version keeps models and tests manageable. |
| D3 | HTTP stack | `hyper` 1.x + `hyper-util` + `hyper-rustls` + `http-body-util` | Full control, no hidden middleware. |
| D4 | TLS | `rustls` with the `aws-lc-rs` provider by default, `ring` as an opt-in feature; root certificates from `webpki-roots` by default, `rustls-native-certs` as a feature | Reproducible builds and no OpenSSL. |
| D5 | WebSocket | `tokio-websockets` with rustls features | Small, fast and Tokio-native. |
| D6 | Gateway encoding | JSON only (no ETF) | serde ecosystem; ETF is not worth it for v0.1. |
| D7 | Gateway compression | Transport `zlib-stream` by default (`flate2`, `zlib-rs` backend), `zstd-stream` behind feature `zstd` | Big bandwidth saving with one inflate context per connection. |
| D8 | Event delivery | Pull-based async stream per shard (`Shard::next_event()`) plus an optional fan-in stream in the manager; no callback traits in the core | Explicit backpressure, no hidden task spawning (see §10). |
| D9 | Unknown data | Every event/model keeps an escape hatch (`Event::Unknown`, unknown enum variants, optional raw JSON) | Discord changes faster than releases. |
| D10 | Cache | Separate crate, behind feature `cache` in the facade, off by default | Zero cost when unused. |
| D11 | Toolchain / MSRV | Toolchain pinned in `rust-toolchain.toml` (`1.100.0`, `profile = "minimal"`, `rustfmt` + `clippy`), `rust-version = "1.100"`; same as `zeny/backend-rs` | One toolchain locally and on the server. |
| D12 | License | `MIT OR Apache-2.0` | Rust ecosystem standard. |
| D13 | `unsafe` | `unsafe_code = "forbid"` without exception, not even in an isolated module. `unsafe` inside maintained dependencies (tokio, hyper, rustls, mimalloc, …) is allowed if they pass `cargo deny`. | Same decision as `zeny/backend-rs` (2026-11-22). |
| D14 | Panics | Library code MUST NOT panic on any input coming from the network. `panic = "unwind"`; long-running library tasks (shard tasks) run behind a panic guard, so a panic ends one shard run, not the process (§9.5). | Same model as the backend supervisor. |
| D15 | Supply chain | `.cargo/config.toml` with `[registry] global-min-publish-age = "3 days"`; dependencies always at the newest version this allows (no pre-releases); `Cargo.lock` committed; all builds with `--locked`; new dependencies only after asking | Taken over from `zeny/backend-rs`. |
| D16 | Lints | Lint set taken over 1:1 from `zeny/backend-rs` (§18.2). Lints are never weakened; a single `#[allow(...)]` needs a reason in a comment. | Proven setup. |
| D17 | Global allocator | The library does **not** set a global allocator. Applications (ZenyBot, examples, benchmarks) use `mimalloc`, as the backend does. | A library must not decide the allocator for its users. |

---

# 1. Goals

### Primary goals

- Own Gateway and REST implementation without Twilight, Serenity, Poise or any other Discord library.
- Manual and automatic sharding, including multi-process deployments.
- Correct rate limiting that never puts the bot's IP at risk of a Cloudflare ban.
- Low memory use, low latency and high throughput, measured rather than assumed (§16).
- Modular crates that work on their own, with a convenient facade crate.
- Raw access to everything the typed API does not cover yet.

### Non-goals

- Voice connections, audio, Opus, UDP voice transport, DAVE, Songbird.
- User-account automation (self-bots) and unofficial or undocumented API use.
- ETF encoding (v0.1).
- A command framework or macros (may come later as a separate crate).

---

# 2. Scope Tiers

Discord's API is too large to cover fully in one release. Coverage is split into tiers. **v0.1.0 = all of P0 + raw fallbacks for everything else.**

### Phase 0 deliverable: ZenyBot inventory

Before implementation starts, produce `docs/zenybot-inventory.md` listing:

- every REST endpoint ZenyBot calls,
- every Gateway event ZenyBot handles,
- the intents it needs (privileged ones marked),
- the interaction features it uses (commands, components, modals, autocomplete),
- expected guild count and shard count over 12 months.

That inventory decides what goes into P0. The list below is the minimum P0, which is needed by practically every bot.

**Note:** ZenyBot today is C++ (`zeny/bot`, D++ v10.1.6, callbacks, no coroutines). The inventory is therefore taken from the C++ sources (D++ calls and event handlers). How ZenyBot will consume this library (a Rust port of the bot, or a new Rust bot next to it) is an open decision (§0 open items) and does not change the library itself.

### P0 – required for v0.1.0

| Area | Content |
|---|---|
| Gateway | Full connection lifecycle, all opcodes (§8.3), all close codes (§8.6), resume, compression, outbound rate limiting, presence updates, member chunking |
| Sharding | Manual + automatic, identify queue (local + trait for distributed), shard manager |
| Events | Typed: Ready, Resumed, Guild*, Channel*, Thread*, Message*, Reaction*, Member*, Role*, InteractionCreate, plus everything in the ZenyBot inventory. Everything else: `Event::Unknown` with raw JSON |
| REST core | Client, auth, rate limiter, invalid-request guard, multipart, errors, raw requests |
| REST endpoints | Users (@me), Channels, Messages, Reactions, Threads, Guilds (core), Members, Roles, Application Commands, Interaction responses/follow-ups, Webhooks (execute/edit) and ZenyBot inventory endpoints |
| Interactions | Slash/user/message commands, autocomplete, all component types incl. Components V2, modals, all response types, Gateway and HTTP delivery incl. Ed25519 verification |
| Models | All models used by the P0 endpoints and events |

### P1 – v0.2.x

Audit logs, Auto Moderation, Scheduled Events, Emojis (guild + application), Stickers, Invites, Polls, Stage Instances, Guild Templates, Onboarding, Welcome Screen, Integrations, the remaining typed Gateway events, and the cache crate if ZenyBot does not need it for v0.1.

### P2 – later

Entitlements, SKUs, Subscriptions (monetization), Soundboard, Application Role Connections, Lobbies, ETF encoding, the automated API diff workflow (§19).

**Rule:** a resource is "supported" only if its endpoints have typed methods, its models have deserialization tests against documented examples, and it is listed in `docs/coverage.md`. Anything else counts as reachable through raw requests and is not claimed as covered.

---

# 3. Technology Stack

Versions are pinned in `[workspace.dependencies]`; `Cargo.lock` is committed. Where `zeny/backend-rs` already uses a crate, the **same major version and feature style** is used, so both projects stay in step (reference versions from its `Cargo.lock`, 2026-11-22, in brackets; at Phase 0 always take the newest version the 3-day rule allows).

| Purpose | Crate | Notes |
|---|---|---|
| Runtime | `tokio` [1.54] | features `rt`, `net`, `time`, `sync`, `macros` (tests only). The library MUST work on both `current_thread` and multi-thread runtimes (§16.4). |
| Cancellation | `tokio-util` | `CancellationToken` for shutdown |
| HTTP | `hyper` [1.12], `hyper-util` [0.1.21], `http` [1.5], `http-body-util` | client + optional server for interactions; `default-features = false` as in the backend |
| TLS | `rustls` [0.23], `hyper-rustls`, `aws-lc-rs` provider [as in backend], `webpki-roots` / `rustls-platform-verifier` | see D4; the backend uses `rustls-platform-verifier` (system roots), offered here as feature `native-roots` |
| WebSocket | `tokio-websockets` [0.13] | `default-features = false`, `client`, `fastrand`, `sha1_smol` as in the backend, plus rustls features |
| Bytes | `bytes` [1.12] | buffer reuse |
| Serialization | `serde`, `serde_json` [1.0.154] | `serde_json` with `raw_value` (same as backend). `sonic-rs` only if a profile shows JSON as the bottleneck (backend rule). |
| Time | `time` [0.3] | no `chrono` (backend decision); `serde`, `parsing`, `formatting` features |
| Locks | `parking_lot` [0.13] | short synchronous critical sections (rate limiter buckets, cache shards); never held across `.await` |
| Compression | `flate2` (`zlib-rs`) [1.1], `zstd` (optional) | see D7 |
| Flags | `bitflags` 2.x | permissions, intents, flags |
| Errors | `thiserror` [2.0] | library errors only; no `anyhow` in library crates |
| Logging | `tracing` [0.1] | no `log` dependency; `tracing-subscriber` only in examples/tests |
| Crypto | `ed25519-dalek` | interaction signature verification only; `subtle` for constant-time compares where needed (as in the backend) |
| Randomness | `fastrand` [2.5] | heartbeat jitter, backoff jitter (already pulled in by tokio-websockets) |
| Encoding | `percent-encoding` | audit log reason, route parameters |
| Cache maps | `dashmap` or sharded `parking_lot::RwLock<HashMap>` (cache crate only) | decided by benchmark |
| Allocator (apps only) | `mimalloc` [0.1.52] | examples, benches and ZenyBot; never in library crates (D17) |
| Dev | `tokio-test`, `proptest`, `criterion`, `insta` (snapshots), `dhat` (allocation counting), `cargo-fuzz` | |

Dependency rules (same as `zeny/backend-rs`):

- Always the newest version that `global-min-publish-age = "3 days"` allows; no pre-releases (e.g. no `-rc`).
- New dependencies only when needed and only after asking; reason written in the plan or PR.
- Every new dependency must pass `cargo deny check` (licenses, advisories, duplicates, sources).
- Every crate sets `default-features = false` on its dependencies and enables only what it needs.
- No OpenSSL. Unix-only code behind `#[cfg(unix)]`; everything else builds on Windows and Debian alike.

---

# 4. Architecture

## 4.1 Repository layout

```text
<lib>/
├── Cargo.toml                 # virtual workspace manifest (no root package)
├── Cargo.lock
├── clippy.toml
├── deny.toml
├── rust-toolchain.toml        # pins the development toolchain
├── README.md
├── CHANGELOG.md
├── LICENSE-MIT / LICENSE-APACHE
├── .cargo/config.toml         # global-min-publish-age = "3 days", alias xtask (as in backend-rs)
├── .github/workflows/         # prepared for later; GitHub Actions is currently not usable (§18)
├── crates/
│   ├── facade/               # facade (package `<lib>`): re-exports, Client, prelude
│   ├── model/                # package `<lib>-model`: models, snowflakes, flags, gateway payloads
│   ├── http/                 # package `<lib>-http`: REST client, routes, rate limiter
│   ├── gateway/              # package `<lib>-gateway`: shard, compression, identify queue, shard manager
│   ├── cache/                # package `<lib>-cache`: optional in-memory cache
│   └── interactions/         # package `<lib>-interactions`: builders, HTTP interaction server, signature verification
├── xtask/                     # cargo xtask check | layout-check | bench | release (§18.1)
├── examples/
├── benches/
├── fuzz/
└── docs/
```

## 4.2 Crate dependency graph

```text
<lib>-model  ←  <lib>-http
      ↑               ↑
<lib>-gateway   <lib>-interactions
      ↑               ↑
<lib>-cache (model only)
      ↑
<lib> (facade, all optional via features)
```

Rules:

- `<lib>-model` has no async or network dependencies (serde, bitflags only).
- `<lib>-gateway` uses `<lib>-http` only for `GET /gateway/bot`, through a small trait, so it can be tested with a fake.
- `<lib>-cache` depends only on `<lib>-model` and consumes events, not connections.
- No dependency cycles; checked by `cargo xtask layout-check`.

## 4.3 Facade features

| Feature | Default | Enables |
|---|---|---|
| `gateway` | yes | `<lib>-gateway` |
| `http` | yes | `<lib>-http` |
| `interactions` | yes | `<lib>-interactions` (without the server) |
| `interactions-server` | no | HTTP interaction endpoint (hyper server) |
| `cache` | no | `<lib>-cache` |
| `zstd` | no | zstd-stream gateway compression |
| `native-roots` | no | system root certificates |
| `ring` | no | rustls `ring` provider instead of `aws-lc-rs` |

---

# 5. Models (`<lib>-model`)

## 5.1 Basic types

- **`Id<T>`**: a snowflake as `NonZeroU64` with a zero-sized marker type (`Id<GuildMarker>`, `Id<UserMarker>`, …), so different id kinds cannot be mixed up. It serializes as a string and deserializes from both string and integer. Helpers: `timestamp()`, `worker_id()`, `process_id()`, `increment()`.
- **`Timestamp`**: ISO 8601 parse and format without pulling in `chrono` (own small parser or `time` as an optional feature).
- **`Permissions`**: `bitflags` on `u64`, serialized as a **string**. Unknown bits MUST survive a round trip (`from_bits_retain`).
- **`Intents`**, **`MessageFlags`**, **`UserFlags`**, **`ChannelFlags`**, etc.: bitflags with `from_bits_retain`.
- **`ImageHash`**: compact storage (animated flag + 16 bytes) with a CDN URL builder.

## 5.2 Rules for all models

- Optional fields: distinguish **missing** from **null** where Discord gives them different meanings (`Option<Option<T>>` with a helper, mainly in update/patch bodies).
- Unknown JSON fields MUST be ignored; `#[serde(deny_unknown_fields)]` is forbidden.
- Integer enums: every enum has an `Unknown(u8/u16/…)` variant. A new Discord value MUST NOT fail deserialization.
- String enums: `Unknown(String)` variant.
- Partial objects (partial guild, partial member in interactions, …) get their own types instead of `Option`-everything.
- Large text/collection fields use `Box<str>` / `Box<[T]>` in read-only models where that measurably saves memory.
- Models implement `Clone`, `Debug`, `PartialEq`, `Eq` (where possible), `Serialize`, `Deserialize`.
- Request bodies are separate types (builders) from response models.

## 5.3 Components

Full component support, including **Components V2** (message flag `IS_COMPONENTS_V2`):

- Action Row, Button (all styles incl. Premium/SKU and Link), String/User/Role/Mentionable/Channel Select.
- Text Input.
- Section, Text Display, Thumbnail, Media Gallery, File, Separator, Container.
- Label and File Upload (modal components).
- Unknown component types deserialize into `Component::Unknown { type, raw }`.

Builder-level validation of documented limits (e.g. 5 action rows, 25 select options, label lengths, total component count) runs when building and returns an error instead of letting Discord reject the request.

## 5.4 Gateway payloads

- Envelope `{ op, d, s, t }` deserialized in two steps: first the envelope with `d` as `&RawValue`, then `d` only for events that are actually consumed (§10.3).
- An `Event` enum with one variant per typed event, plus `Event::Unknown { name: Box<str>, raw: Box<RawValue> }`.
- Send payloads (Identify, Resume, Heartbeat, Presence Update, Request Guild Members, Request Soundboard Sounds, Voice State Update) are serialize-only types.

## 5.5 Model sources

1. The official developer documentation is authoritative.
2. `discord-api-spec` (OpenAPI, public preview) MAY be used to generate drafts of REST models. Generated code MUST be reviewed by hand and tested against the documented JSON examples before it is merged.
3. Gateway models are written by hand (OpenAPI does not cover the gateway).

---

# 6. REST Client (`<lib>-http`)

## 6.1 Client

- A single `Client` (cheap to `Clone`, internally `Arc`) holding the connection pool, rate limiter and configuration.
- Base URL `https://discord.com/api/v10`, configurable (for proxies and mocks).
- Headers:
  - `Authorization: Bot <token>` (and `Bearer` for OAuth2 as an option).
  - `User-Agent: DiscordBot (<repo url>, <crate version>)`. This format is mandatory.
  - `Content-Type` according to the body.
  - `X-Audit-Log-Reason`, percent-encoded, max. 512 characters, set per request.
- HTTP/1.1 keep-alive with a pool and HTTP/2 via ALPN (configurable; enabled only after testing).
- `TCP_NODELAY` on every connection; idle pooled connections are dropped after **30 s**, because Discord or NAT silently drop idle connections (lesson from the backend's Discord client).
- No DNS lookup per request: the pool reuses connections; hyper-util's resolver runs off the runtime threads.
- Timeouts: connect (default 10 s), whole request (default 15 s, the value proven in the backend), both configurable.
- Response bodies are read in full with a size limit (default 32 MiB) and then deserialized from `Bytes`.

## 6.2 Request API

```rust
// typed
let msg = http.create_message(channel_id)
    .content("hello")?
    .reply(message_id)
    .await?;

// raw
let value: serde_json::Value = http
    .request(Method::GET, Route::raw("/guilds/{guild_id}/onboarding", guild_id))
    .send()
    .await?
    .json()
    .await?;
```

- Every typed endpoint is defined by a `Route` (method + path template + major parameters), so the rate limiter and raw requests share one code path.
- Request builders validate documented limits (content length 2000, embeds ≤ 10, total embed text ≤ 6000, …) before sending.
- Multipart: `payload_json` + `files[n]` and the matching `attachments` array; streaming upload from `Bytes` or an `AsyncRead`.
- Responses: a typed future that yields `Response<T>` with access to status, headers and the raw body.

## 6.3 Retry policy

| Situation | Behavior |
|---|---|
| 429 | Wait `retry_after` (from the body, float seconds; fall back to `Retry-After`, else 1 s), then retry. Max. 3 retries per request, and only if the wait is ≤ a configurable cap (default 10 s, as in the backend). Longer locks return `HttpError::RateLimited` at once, so callers are not blocked for minutes. |
| 502/503/504, connection reset before the response | Retry only for `GET`, `HEAD`, `PUT`, `DELETE` (idempotent), with exponential backoff and jitter (100 ms → 1.6 s), max. 3 retries. |
| 500 | No retry. |
| `POST`, `PATCH` | Never retried automatically except on 429, because a 429 guarantees the request was not processed. |
| 401 | Not retried; marks the token as invalid and fails all following requests immediately (§6.5). |
| 403, 404, 400 | Not retried; returned as a typed error. |

## 6.4 Errors

```rust
pub enum HttpError {
    Discord { status: StatusCode, code: u32, message: Box<str>, errors: Option<Box<RawValue>> },
    RateLimited { retry_after: Duration, global: bool, scope: Option<RateLimitScope> },
    Unauthorized,          // token invalid, client poisoned
    Timeout,
    Transport(…),          // hyper/io/TLS
    Deserialize { body: Bytes, source: serde_json::Error },
    Validation(…),         // a builder limit was violated locally
}
```

- Discord JSON error codes (e.g. 10008 Unknown Message, 50013 Missing Permissions) are available as `DiscordErrorCode` enum with `Unknown(u32)`.
- Nested `errors` objects stay accessible as raw JSON plus a helper that flattens them into `field.path: message` pairs.

## 6.5 Invalid request guard (critical)

Discord blocks the IP address (Cloudflare) for a while if more than **10,000 invalid requests (status 401, 403 or 429, except shared-scope 429s) happen within 10 minutes**.

- The client counts these responses in a sliding 10-minute window.
- From 8,000 on it logs a `warn`; from 9,500 on it rejects new requests locally with `HttpError::InvalidRequestLimit` until the window has room again (threshold configurable).
- After a 401 the client goes into a poisoned state, so a revoked token cannot cause a request storm.

---

# 7. Rate Limiter

## 7.1 Discord's model

- Per-route buckets identified by `X-RateLimit-Bucket` (a hash). Several routes can share one bucket.
- Bucket key = bucket hash + **major parameters** (`channel_id`, `guild_id`, `webhook_id` + `webhook_token`).
- Headers: `X-RateLimit-Limit`, `-Remaining`, `-Reset`, `-Reset-After`, `-Bucket`, `-Global`, `-Scope` (`user` / `global` / `shared`).
- Global limit: **50 requests per second** per bot (larger bots may have a higher limit). Interaction endpoints (`/interactions/{id}/{token}/callback` and webhook follow-ups with the interaction token) are not subject to the global limit.
- A 429 with `X-RateLimit-Global: true` or `global: true` in the body blocks all requests.
- A 429 with scope `shared` does not count towards the invalid request limit.
- Emoji routes and some others have special limits that are not in the headers. That is acceptable; the limiter learns from 429s.

## 7.2 Implementation

1. **Route → bucket mapping:** `HashMap<RouteKey, BucketHash>`, learned from response headers. `RouteKey` = method + path template + major parameters. Until a hash is known the route is treated as its own bucket with concurrency 1 (no parallel "probe" requests to an unknown bucket).
2. **Bucket state:** `remaining`, `limit`, `reset_at: Instant`, plus a FIFO queue of waiters. Requests per bucket are executed in order (a `tokio::sync::Semaphore` with permits = `remaining`, refilled at `reset_at`).
3. **Global limiter:** a token bucket at 50/s (configurable) for everything except interaction routes, plus a global lock that is set by a global 429 until `retry_after`.
4. **Ordering:** a request waits for the global limiter → its bucket → is sent → its headers update the bucket. If no headers come back (e.g. 5xx), one permit is returned.
5. **Clock:** only `Instant` plus `Reset-After` are used, never local wall-clock time compared to `X-RateLimit-Reset` (avoids clock drift problems).
6. **Cleanup:** buckets that have been unused for 10 minutes are removed (prevents unbounded growth with many channels).
7. **Locking:** a sharded map for buckets (`DashMap` or N mutex shards); no global mutex on the hot path.
8. **Pluggable:** `trait RateLimiter` so multi-process deployments can use a shared limiter (e.g. a Redis-based implementation in a separate crate later). In-process is the default.

## 7.3 Tests

- Unit tests with a fake clock (`tokio::time::pause`) for: bucket discovery, shared buckets, major parameters, global 429, `Reset-After`, missing headers, cleanup.
- An integration test against a mock server that enforces limits and fails the test on any 429 that a correct limiter should have avoided.
- A load test: 10,000 concurrent requests across 100 channels against the mock with **zero** avoidable 429s.

---

# 8. Gateway (`<lib>-gateway`)

## 8.1 Connection URL

- Fetch `GET /gateway/bot` once at startup (shard count, `session_start_limit`, URL); cache the URL.
- Connect to `<url>/?v=10&encoding=json&compress=zlib-stream` (or `zstd-stream`).
- For a resume, connect to the `resume_gateway_url` from READY (with the same query parameters), **never** to the original URL.

## 8.2 Connection state machine

```text
Disconnected → Connecting → AwaitingHello → Identifying ─┐
                                    │                    ├→ Active ⇄ (dispatch)
                                    └→ Resuming ─────────┘
Active → Reconnecting (resume) | Reconnecting (new session) | Disconnected (fatal) | ShuttingDown → Disconnected
```

- The state is observable (`Shard::state()`, plus a `ShardStateChanged` event in the manager).
- Every transition is logged with `tracing` (shard id, old state, new state, reason).

## 8.3 Opcodes

| Op | Name | Direction | Handling |
|---|---|---|---|
| 0 | Dispatch | receive | update `s`, dispatch the event |
| 1 | Heartbeat | both | if received: send a heartbeat immediately |
| 2 | Identify | send | §8.5 |
| 3 | Presence Update | send | public API, counted against the outbound limit |
| 4 | Voice State Update | send | public API (payload only; no voice connection) |
| 6 | Resume | send | §8.7 |
| 7 | Reconnect | receive | close with a resumable code and resume |
| 8 | Request Guild Members | send | public API with a `nonce`, chunk collection (§8.9) |
| 9 | Invalid Session | receive | `d=true`: resume; `d=false`: wait 1–5 s (random), new identify |
| 10 | Hello | receive | start the heartbeat |
| 11 | Heartbeat ACK | receive | mark the ACK and measure latency |
| 31 | Request Soundboard Sounds | send | public API |
| other | — | receive | log at `debug`, ignore; never crash |

## 8.4 Heartbeat

- After HELLO: the first heartbeat after `heartbeat_interval * random(0..1)`, then every `heartbeat_interval`.
- The heartbeat payload is the last sequence number (or `null`).
- If no ACK arrived since the previous heartbeat: the connection is a **zombie**. Close with a non-1000/1001 code (e.g. 4000) and resume.
- Latency = time from send to ACK; the last N values are kept and exposed per shard.
- The heartbeat runs in the shard's own task/select loop and is **never** blocked by slow event consumers (§10).

## 8.5 Identify

- Payload: token, intents, `properties { os, browser: "<lib>", device: "<lib>" }`, `compress: false` (transport compression is used instead), `large_threshold` (default 50, 50–250), `shard: [id, count]`, optional initial presence.
- An identify happens only after a permit from the **identify queue** (§9.3).
- `session_start_limit.remaining` is decreased for every identify; at 0 the shard waits until `reset_after` instead of identifying (logged at `warn`).

## 8.6 Close codes

| Code | Meaning | Action |
|---|---|---|
| 1000 / 1001 | Normal closure (sent by us) | session invalid → new identify on reconnect |
| 4000 | Unknown error | resume |
| 4001 | Unknown opcode | resume, log at `error` (our bug) |
| 4002 | Decode error | resume, log at `error` (our bug) |
| 4003 | Not authenticated | new identify |
| **4004** | **Authentication failed** | **fatal, no reconnect**, error to the user |
| 4005 | Already authenticated | resume, log at `error` |
| 4007 | Invalid sequence | new identify |
| 4008 | Rate limited | resume after a backoff, log at `warn` (outbound limiter failed) |
| 4009 | Session timed out | new identify |
| **4010** | **Invalid shard** | **fatal** |
| **4011** | **Sharding required** | **fatal** (the manager MAY restart with a new shard count, §9.4) |
| **4012** | **Invalid API version** | **fatal** |
| **4013** | **Invalid intents** | **fatal** |
| **4014** | **Disallowed intents** (privileged intent not enabled) | **fatal**, error message names the intent |
| other / no close frame (TCP drop) | — | resume |

Fatal codes MUST NOT lead to a reconnect loop. The shard goes to `Disconnected` and emits `ShardEvent::Fatal(reason)`.

## 8.7 Resume and reconnect

- Keep `session_id`, `resume_gateway_url` and the last sequence number.
- Resume payload: token, `session_id`, `seq`.
- Backoff for consecutive failed connection attempts: 1 s, 2 s, 4 s, … max. 60 s, each with ±20 % jitter. Reset after 60 s of stable `Active` state.
- After RESUMED, missed events are replayed by Discord as normal dispatches; nothing special is needed.
- Timeouts: no HELLO within 20 s after connecting, or no READY/RESUMED within 30 s after identify/resume → close and reconnect.

## 8.8 Outbound rate limit

- Discord allows **120 gateway sends per 60 seconds per connection**. Exceeding it closes the connection with 4008.
- A token bucket with 120 per 60 s, of which **a reserve is kept for heartbeats** (e.g. user payloads may use at most 110). Heartbeats always bypass the user queue.
- Send payloads larger than 4096 bytes are rejected locally.
- Presence updates and member requests from the user go through a bounded queue; when it is full, the caller gets an error or waits (configurable).

## 8.9 Request Guild Members

- `nonce` generated per request (max. 32 characters), chunks are collected until `chunk_index == chunk_count - 1`.
- API: `shard.request_members(guild_id, query).await -> Vec<Member>` plus a streaming variant.
- Requesting all members (`query: ""`, `limit: 0`) requires the `GUILD_MEMBERS` intent and is checked locally.
- Timeout per request (default 30 s).

## 8.10 Compression

- `zlib-stream`: **one** inflate context for the whole connection; a message is complete when the received binary data ends with `00 00 FF FF`. Frames are collected in a reused buffer.
- The decompression buffer is reused, its capacity is capped (shrunk after unusually large payloads, e.g. > 1 MiB), and a maximum decompressed message size protects against memory exhaustion (default 16 MiB).
- `zstd-stream` (feature): one streaming decoder per connection.
- A new connection (including a resume) always starts with a new context.

## 8.11 Graceful shutdown

- `shard.close(CloseMode::Resumable)`: close code 4000, session data is returned to the caller (`ResumeInfo`) so another process can resume (rolling deploys).
- `shard.close(CloseMode::Terminate)`: close code 1000, session ended.
- Shutdown waits at most 5 s for the close handshake, then drops the connection.
- `CancellationToken` support for the whole manager.

## 8.12 Intents

- All documented intents as bitflags, unknown bits retained.
- `Intents::non_privileged()`, `Intents::privileged()` helpers.
- `GUILD_MEMBERS`, `GUILD_PRESENCES` and `MESSAGE_CONTENT` are marked privileged in the docs, and 4014 errors name them.
- Debug-mode warning if typed events are requested whose intent is missing (e.g. message content without `MESSAGE_CONTENT`).

---

# 9. Sharding

## 9.1 Manual sharding

```rust
let mut shard = Shard::builder(token, intents)
    .shard(ShardId::new(0, 4)?)        // id 0 of 4, checked: id < count
    .identify_queue(queue.clone())
    .build();

while let Some(event) = shard.next_event().await {
    match event? { /* … */ }
}
```

- Each shard owns its connection, state, heartbeat and reconnect logic.
- Shards run in separate Tokio tasks or in separate processes; the library makes no assumption.
- `Shard::with_resume_info(...)` allows starting from a saved session (e.g. after a deploy).

## 9.2 Automatic sharding

```rust
let manager = ShardManager::builder(token, intents)
    .automatic()                       // or .shards(range, total)
    .build()
    .await?;                           // calls /gateway/bot

let mut events = manager.events();     // fan-in stream: (ShardId, Event)
manager.start().await?;
```

The manager:

1. Calls `GET /gateway/bot` → `shards`, `session_start_limit { total, remaining, reset_after, max_concurrency }`.
2. Checks that `remaining ≥ number of shards to start`; otherwise it starts the possible ones and waits for `reset_after` for the rest (logged at `warn`).
3. Creates the shards and passes them a shared identify queue.
4. Supports a **shard range** (`shards(10..20, 40)`) for clustering across processes, each with automatic count or a fixed count.

## 9.3 Identify queue

- Discord rule: identify bucket = `shard_id % max_concurrency`; **at most one identify per bucket every 5 seconds**.
- `trait IdentifyQueue: Send + Sync { async fn acquire(&self, shard_id: u32); }`.
- `LocalIdentifyQueue`: in-process, one queue per bucket, 5 s spacing plus a small safety margin (e.g. 250 ms).
- For multi-process: the trait can be implemented on top of Redis or a small coordination service (an example implementation in `examples/`, not in the core).
- Resumes do **not** go through the identify queue.

## 9.4 Shard manager API

- `start()`, `shutdown(mode)`, `restart_shard(id)`, `restart_all()` (rolling, respecting the identify queue).
- `status() -> Vec<ShardStatus>` (state, latency, last heartbeat ACK, guild count from READY, reconnect count).
- Per-shard event stream and the fan-in stream.
- **Resharding:** when the recommended shard count changes (or 4011), only a coordinated restart is supported: start the new shard set, wait until all are `Active`, then stop the old ones. Optional for v0.1 (documented as P1); v0.1 at least emits a warning event when `/gateway/bot` recommends more shards than are running.
- A fatal error on one shard does not stop the others; it is reported as `ShardEvent::Fatal`.

## 9.5 Panic guard and supervision

Taken over from the backend's job supervisor (`jobs/supervisor.rs`, `run_guarded`):

- Every shard run (one connection lifetime) is wrapped in `AssertUnwindSafe(..).catch_unwind()`. A panic becomes `ShardEvent::Error(Panic(msg))`, is logged at `error` with the shard id, and the shard reconnects with the normal backoff (resume info is kept if still valid).
- A panic never ends the manager, other shards or the process (`panic = "unwind"` is required; documented for users).
- Panics are counted per shard (metric). More than 5 panics in 10 minutes on one shard → the shard stops with `ShardEvent::Fatal` to avoid a hot loop.
- The panic guard is a safety net, not error handling: every panic found is a bug and gets a regression test.

---

# 10. Event Delivery and Backpressure

## 10.1 Model

- Each shard has its own read task, which parses frames and puts events into a **bounded** `tokio::sync::mpsc` channel (default capacity 1024, configurable).
- The user reads with `next_event()` / `Stream`.
- Heartbeats, ACKs, reconnects and the outbound queue are handled in the shard task and never wait on the user.

## 10.2 Slow consumer policy

Configurable per shard:

| Policy | Behavior | Default for |
|---|---|---|
| `Block` | The read task waits when the channel is full. The heartbeat keeps running (separate select branch), but Discord may close the connection if the socket is not read. | — |
| `DropOldest` / `DropNewest` | Drop events, increment a counter, log at `warn` every N drops | opt-in |
| `Disconnect` | Close resumable and resume later; Discord replays missed events | **default** |

The `Disconnect` default is safe: no event is lost and memory stays bounded.

## 10.3 Event filter

- `EventTypeFlags` decide which events are deserialized fully. Unwanted events are skipped after parsing the envelope (only `t` and `s` are read).
- `Event::Unknown` carries the raw JSON; `RawEvent` mode delivers every dispatch as raw JSON without typed parsing.

## 10.4 Ordering

- Events from one shard are delivered in sequence order.
- No ordering guarantee between shards (the fan-in stream interleaves).

---

# 11. Interactions (`<lib>-interactions`)

## 11.1 Commands

- Builders for slash, user and message commands, including options, choices, localizations, `default_member_permissions`, `nsfw`, `integration_types` (guild/user install) and `contexts` (guild, bot DM, private channel).
- Bulk overwrite (`PUT`) for global and guild commands, plus single create/edit/delete.
- Local validation of names (regex, length), description lengths, max. 25 options/choices and nesting depth.
- `CommandData` parsing helpers: typed access to options (`get_string("name")`, subcommand path), resolved data (users, members, roles, channels, attachments).

## 11.2 Responses

| Type | Name |
|---|---|
| 1 | PONG |
| 4 | CHANNEL_MESSAGE_WITH_SOURCE |
| 5 | DEFERRED_CHANNEL_MESSAGE_WITH_SOURCE |
| 6 | DEFERRED_UPDATE_MESSAGE |
| 7 | UPDATE_MESSAGE |
| 8 | APPLICATION_COMMAND_AUTOCOMPLETE_RESULT |
| 9 | MODAL |
| 10 | PREMIUM_REQUIRED (deprecated, supported only for deserialization) |
| 12 | LAUNCH_ACTIVITY |

- Deadlines are part of the API: the initial response must happen within **3 seconds**; the interaction token stays valid for follow-ups/edits for **15 minutes**.
- `InteractionHandle` helper: `defer()`, `respond()`, `edit_original()`, `delete_original()`, `followup()`, plus `ephemeral()`. It tracks whether a response was already sent and returns an error on a double response.
- `with_response=true` is supported to get the callback response object.
- Optional helper: auto-defer if no response was given after a configurable time (e.g. 2.5 s).

## 11.3 HTTP interaction endpoint (feature `interactions-server`)

- Verify every request with Ed25519 using `X-Signature-Ed25519` and `X-Signature-Timestamp` over `timestamp + body`, with the application's public key. Invalid → `401`.
- Signature verification works on the raw body **before** any JSON parsing.
- PING (type 1) is answered with PONG (type 1).
- Body size limit (e.g. 1 MiB) and request timeout.
- Provided as a `tower`-free hyper `Service` plus a standalone `verify_signature()` function, so it can also be used inside axum/actix/etc.

---

# 12. Cache (`<lib>-cache`)

- Completely optional (D10). It is fed with `cache.update(&event)`; no network access of its own.
- Resource types configurable individually (`ResourceType` bitflags): guilds, channels, threads, users, members, roles, emojis, stickers, voice states, presences, messages.
- Messages: a per-channel ring buffer with a configurable size (default 0 = off).
- Members: off by default (large bots); on when configured.
- References are stored by id (`Id<T>`), not duplicated. Users are stored once and referenced by members.
- Lookups return guards or cheap clones (`Arc`) and never hold a lock across an `.await`.
- Correctness rules: GUILD_DELETE with `unavailable: true` only marks the guild unavailable; with `unavailable` missing it removes the guild **and** all dependent entries (channels, members, roles, …).
- Metrics: entry count per resource type.
- Tests: an event sequence is replayed and the cache is compared to the expected snapshot (`insta`), plus concurrency tests (parallel read/update).

---

# 13. Error Handling (all crates)

- One error type per crate (`HttpError`, `GatewayError`, `ModelError`, …) with `thiserror`, `#[non_exhaustive]`, implementing `std::error::Error + Send + Sync + 'static`.
- The facade has an `Error` enum wrapping all of them.
- No `unwrap`/`expect`/`panic!`/indexing that can panic in library code (clippy-enforced; tests excluded via `clippy.toml`).
- Errors carry context (route, shard id, close code) without containing secrets.

---

# 14. Security

- **Token handling:** a `Token` newtype whose `Debug` and `Display` print `Token(***)`. The token never appears in logs, errors, panics or `tracing` spans. A test verifies this.
- **TLS:** certificate verification is always on; there is no option to disable it.
- **Input limits:** maximum sizes for WebSocket frames, decompressed messages, HTTP response bodies and interaction request bodies (all configurable, with safe defaults).
- **Interaction endpoint:** signature verification is mandatory and cannot be turned off in the server.
- **Dependencies:** `cargo deny check` (advisories, licenses, sources) in `cargo xtask check`; 3-day publish age; manual update pass at least monthly.
- **Test secrets:** the test bot token lives only in a local, git-ignored `.env` (mode 600 on Linux, like the backend), is never committed and never printed.
- **Fuzzing:** gateway frame parsing, decompression and model deserialization (§17.4).
- `SECURITY.md` with a reporting channel.

---

# 15. Observability

- `tracing` spans: `shard{id=…}` for all gateway work, `http.request{method, route}` for REST (with the route template, not the full path with tokens).
- Levels: `error` = action needed (fatal close code, 401), `warn` = degraded (rate limit, reconnect, dropped events), `info` = lifecycle (connect, READY, RESUMED, shutdown), `debug` = per-request/per-event, `trace` = payloads (**without the token**; webhook/interaction tokens in paths are redacted).
- Metrics are exposed through simple getters (no metrics crate dependency in the core): per shard latency, reconnects, resumes, identifies, events received/dropped, bytes received (compressed/uncompressed); per client requests, 429s by scope, invalid-request window count, active buckets.
- An optional `metrics` feature that reports the same values through the `metrics` crate.

---

# 16. Performance

The approach is the one that worked for `zeny/backend-rs` (see `zeny/docs/plans/Backend Rust Rewrite.md`, "Performance: voll getunt"): fully tuned, but **without own `unsafe`**, measured against an own baseline, with regressions caught by `cargo xtask bench`.

## 16.1 Targets

First values are estimates. After the first full measurement (Phase 7) the measured numbers become the **binding baseline** in `benches/baseline.json` (same process as the backend: first measurement = baseline, then only regression checks).

| Metric | Initial target |
|---|---|
| Gateway dispatch (receive frame → event in channel) for MESSAGE_CREATE | p99 < 50 µs (excluding the network) |
| Skipped (filtered) event | < 5 µs |
| Memory per idle shard (no cache) | < 1 MiB |
| RSS of a test bot (1 shard, no cache) idle / under replayed load / after 1 h | < 15 MB / < 40 MB / plateau |
| REST overhead (client side, excluding the network) | p99 < 100 µs |
| Cache lookup by id | < 200 ns |
| Allocations per filtered event | 0 after warm-up (measured with `dhat`) |
| Allocations per typed MESSAGE_CREATE | ≤ 15 (measured with `dhat`, then fixed as budget) |
| Allocations per REST request (client side) | ≤ 20 |
| Regression | Warning at > 10 % worse than `benches/baseline.json` (throughput, p99 or allocations) |

Benchmarks run on the production KVM (8 cores, AMD EPYC 7543) for the official numbers, like the backend; local numbers are only for comparisons within one machine. Single runs on the shared KVM vary a lot, so a regression must be confirmed by a second run.

## 16.2 Rules

**Allocations and memory** (backend rules)
- Allocation budgets per hot path (table above), measured with `dhat` as a dev feature.
- `&str` / `Cow` instead of `String` in transient data, `Box<str>` / `Box<[T]>` in long-lived models; small fixed values without heap (snowflakes as `u64`, image hashes as 16 bytes).
- Buffers reused per connection: WebSocket receive buffer, inflate output buffer, JSON send buffer (serialize into a reused `Vec<u8>` / `BytesMut`).
- Pre-sized buffers instead of `format!` on hot paths (route keys, URLs); the clippy perf lints (§18.2) enforce most of this.
- No unbounded queue, map or cache anywhere; every structure has an upper limit (rate limiter buckets are cleaned, event channels are bounded, caches have caps).

**Latency**
- No blocking on runtime threads. CPU-heavy work (e.g. parsing a huge READY/GUILD_CREATE) stays in the shard task, which is fine because each shard is independent; no `spawn_blocking` in the library.
- No serialize → deserialize round trips; two-step envelope parsing (§5.4).
- No global locks on the hot path (sharded rate limiter and cache); `parking_lot` locks only for short synchronous sections, never across `.await`.
- `TCP_NODELAY` on gateway and REST connections.
- Independent async work is run concurrently (`tokio::join!`), e.g. fetching `/gateway/bot` and application info at startup (same pattern as the backend's parallel queries).

**Process**
- Optimizations only with a benchmark or profile showing the effect; results go into `docs/performance.md`.
- `serde_json` is the default; `sonic-rs` or similar only if a profile shows JSON as the bottleneck **and** the output is identical.

## 16.3 Benchmarks and tooling

- **Micro benchmarks** (`criterion`, `benches/`): JSON deserialization of real payloads (READY of a large guild, GUILD_CREATE, MESSAGE_CREATE, INTERACTION_CREATE), envelope parsing, zlib-stream inflate, event dispatch, rate limiter acquire under contention, cache update/lookup.
- **Allocation benchmarks** (`dhat`): per hot path, compared against the budgets.
- **Load replay**: a recorded (anonymized) gateway event stream replayed through the mock gateway at a fixed rate; measures throughput, p99 dispatch latency, RSS.
- **`cargo xtask bench`**: runs all of the above and compares with `benches/baseline.json`; warning at > 10 % regression. Run before every larger merge.
- **Profiling**: `perf` + flamegraph on the KVM, `tokio-console` for scheduling, `dhat` for allocations (same tools as the backend).

## 16.4 Runtime model compatibility

The backend runs **thread-per-core** (one `current_thread` runtime per core, `SO_REUSEPORT`, no work stealing on the hot path) and found it clearly faster than one multi-thread runtime. The library MUST support both models:

- No hidden `tokio::spawn` on a runtime the user did not choose; a shard's tasks run on the runtime where the shard was started.
- Shards can be distributed over several `current_thread` runtimes (e.g. shard `i` on runtime `i % cores`); the shard manager accepts a spawner per shard (`ShardManager::spawn_with(...)`) for that.
- The REST client and the rate limiter are `Send + Sync` and can be shared between runtimes; their own background work (bucket cleanup) is lazy and does not need a dedicated task.
- An example `examples/thread_per_core.rs` and a benchmark comparing both models on the replayed load. The faster one becomes the recommendation in the docs.

## 16.5 Build profile for applications

Profiles taken over from the backend (already in the workspace `Cargo.toml`): `[profile.dev.package."*"] opt-level = 1`; release with `opt-level = 3`, `lto = "fat"`, `codegen-units = 1`, `panic = "unwind"`, `debug = "limited"`, `strip = "debuginfo"`; `release-fast` without LTO. Applications on the KVM build with `-C target-cpu=native` (opt-out like the backend's `ZENY_PORTABLE=1`). PGO/BOLT is deferred, as in the backend, until real traffic gives a representative profile.

---

# 17. Testing

## 17.1 Unit tests

Snowflakes, timestamps, permissions/flags (including unknown bits), every model against the JSON examples from the docs, unknown enum values, optional vs. null fields, envelope and opcode parsing, close code mapping, heartbeat scheduling (with `tokio::time::pause`), backoff, the identify queue (timing), the rate limiter (§7.3), the invalid request guard, builder validation, signature verification (official test vectors plus negative cases), token redaction.

## 17.2 Mock gateway

An in-repo mock gateway server (`tests/support/mock_gateway.rs`) that can be scripted:

- HELLO with a configurable interval, READY, dispatches, HEARTBEAT_ACK on/off (zombie test).
- Sending RECONNECT, INVALID_SESSION (true/false), any close code, dropping TCP without a close frame.
- zlib-stream compression of its output.
- Verifying what the client sends: identify payload, heartbeat timing, resume payload, outbound rate.

Required scenarios (each one test):

1. Connect → identify → READY → events.
2. Heartbeat timing including the first-heartbeat jitter.
3. Missing ACK → zombie → resume.
4. RECONNECT → resume to `resume_gateway_url`.
5. INVALID_SESSION true → resume; false → wait 1–5 s → identify.
6. Each close code in §8.6 → the right action; fatal codes → no reconnect.
7. TCP drop → resume.
8. Outbound burst of 200 presence updates → no more than 120 per 60 s, heartbeats still sent.
9. Slow consumer → policy behaves as configured, heartbeat not delayed.
10. Graceful shutdown with both modes.
11. Multiple shards with `max_concurrency` 1 and 16 → identify spacing respected.
12. `session_start_limit.remaining` too low → waits for `reset_after`.
13. Unknown opcode / unknown event / malformed JSON → no crash, logged.
14. Request guild members with multiple chunks.

## 17.3 Mock REST

A mock HTTP server for: authentication headers, User-Agent, rate limit headers and 429s (route/global/shared), 5xx retry rules, multipart bodies, error JSON parsing, timeouts, the invalid request guard.

## 17.4 Property and fuzz tests

- `proptest`: snowflake and flag round trips, model serialize → deserialize round trips.
- `cargo-fuzz` targets: envelope parsing, event deserialization, zlib-stream inflate, interaction signature verification. Run before each release for a fixed time (e.g. 10 min per target; needs Linux/nightly, so on the KVM or WSL when available).

## 17.5 Live tests

- A dedicated test application and test guild; tests are `#[ignore]` and run with `cargo test -- --ignored` when `LIB_TEST_TOKEN` is set.
- Run manually before releases and after gateway/sharding changes. Without a token they are skipped visibly (same pattern as the backend's `ZENY_PG_TEST_URL` tests).
- Scenarios: connect + READY, slash command registration and response, message create/edit/delete, multipart upload, reaction, 2 shards with a forced reconnect, member chunk request.

## 17.6 Soak test

Before every release: ZenyBot (or a test bot) runs on the release candidate for **at least 72 hours** without restarts. Exit criteria: no panics, no unbounded memory growth (RSS after 72 h within 10 % of the 6 h value), all disconnects recovered by resume or re-identify, zero avoidable 429s.

## 17.7 Coverage

Line coverage measured with `cargo-llvm-cov`; target ≥ 80 % for `<lib>-gateway` and `<lib>-http`, ≥ 70 % overall. Coverage is a guideline; the scenario list above is the requirement.

---

# 18. Quality Gate, Versioning and Release

## 18.1 Local checks with `cargo xtask` (the quality gate)

GitHub Actions is currently **not usable** (quota used up, same situation as the backend). The quality gate is therefore `cargo xtask check`, run locally after every change; a step counts as done only when it is green. A workflow file that calls the same `xtask` commands is kept in `.github/workflows/` for later.

| Command | Content |
|---|---|
| `cargo xtask check` | `cargo fmt --all --check` → `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` → `cargo test --workspace --all-features --locked` → `cargo deny check` → `layout-check` → docs (`RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --all-features`) |
| `cargo xtask layout-check` | Enforces the crate dependency rules of §4.2 (like the backend's layer check): `<lib>-model` has no async/network dependencies, no cycles, `cache` depends only on `model`, no other Discord library anywhere in `cargo tree` |
| `cargo xtask features` | `cargo hack check --feature-powerset --depth 2` (every feature combination compiles) |
| `cargo xtask bench` | §16.3, compared with `benches/baseline.json` |
| `cargo xtask release <version>` | §18.4 |

Before a release, additionally: `cargo semver-checks` (after 0.1), `cargo llvm-cov` report, fuzzing, live tests, and a build + test run on Debian 13 (acceptance platform, like the backend).

## 18.2 Lints

Taken over 1:1 from `zeny/backend-rs` and already in the workspace `Cargo.toml`:

- `unsafe_code = "forbid"`.
- `clippy::perf` = deny, `clippy::pedantic` = warn (with `-D warnings` effectively mandatory). Exceptions: `module_name_repetitions`, `must_use_candidate`, `missing_errors_doc`, `missing_panics_doc`, `too_many_arguments`.
- Performance lints outside `perf` = deny: `needless_pass_by_value`, `large_types_passed_by_value`, `large_futures`, `trivially_copy_pass_by_ref`, `implicit_clone`, `unnecessary_to_owned`, `inefficient_to_string`, `format_push_string`, `format_collect`, `needless_collect`, `or_fun_call`, `manual_memcpy`, `rc_buffer`, `mutex_atomic`, `string_lit_as_bytes`, `redundant_clone`.
- Hygiene = deny: `enum_glob_use`, `dbg_macro`, `todo`, `unwrap_used`, `expect_used` (tests excluded via `clippy.toml` `allow-unwrap-in-tests` / `allow-expect-in-tests`).
- Additionally for a library: `missing_docs` = warn (public API must be documented), `unreachable_pub` = warn.
- Lints are never weakened. A single `#[allow(...)]` needs a reason in a comment.

## 18.3 Versioning

- SemVer. Before 1.0, minor versions may break the API; every break is listed in `CHANGELOG.md`.
- All public enums and structs that Discord may extend are `#[non_exhaustive]`.
- Raising the pinned toolchain / `rust-version` counts as a minor version bump.
- All workspace crates are released with the same version.

## 18.4 Release process

1. All DoD items (§22) are checked off.
2. `CHANGELOG.md` updated (Keep a Changelog format).
3. Soak test (§17.6) passed.
4. Version bump via `cargo xtask release <version>`, git tag `v<version>`.
5. Publishing to crates.io is decided separately; until then `publish = false`.

---

# 19. API Maintenance

Sources (in this order of authority):

1. Official developer documentation: https://discord.com/developers/docs
2. https://github.com/discord/discord-api-docs (docs source and changelog)
3. https://github.com/discord/discord-api-spec (OpenAPI, public preview, not authoritative)

### v0.1 (manual)

- Watch the `discord-api-docs` changelog (GitHub watch/RSS).
- Every Discord change that affects a supported resource gets an issue with the label `api-change`.

### After v0.1 (automated, P2)

A scheduled job (`cargo xtask api-diff`, run manually or by a cron job until CI is available) that:

1. Fetches the latest `discord-api-spec` and `discord-api-docs`.
2. Diffs against the commit recorded in `docs/api-baseline.toml`.
3. Reports new/changed/removed endpoints, fields, enum values and gateway events.
4. Opens an issue with the report (it does **not** generate code automatically).

When the docs and the spec disagree, the docs win.

---

# 20. Roadmap

Every phase has exit criteria. A phase is done only when its criteria are met.

## Phase 0 – Setup (blocker for everything else)

- [ ] **Decide the project name (D1)** and replace `<lib>` everywhere. Working name in code until then: `drs` (one search-and-replace).
- [x] Rename `cargo.toml` → `Cargo.toml`, turn it into a virtual workspace manifest (`members = ["crates/*", "xtask"]`), remove the root `[package]`; keep lints and profiles (already 1:1 from the backend).
- [x] Copy from `zeny/backend-rs` and adapt: `rust-toolchain.toml`, `clippy.toml`, `deny.toml` (for a library additionally drop `[licenses.private]` once the crates get a license field), `.cargo/config.toml` (3-day rule, `xtask` alias).
- [x] Create all crates, `xtask` (`check`, `layout-check`; `features` and `bench` follow with the phases that need them), `[workspace.dependencies]` with the newest versions the 3-day rule allows.
- [ ] Licenses, `SECURITY.md`, README, `.github/workflows/` prepared for later.
- [ ] ZenyBot inventory from the C++ bot (§2).

**Exit:** `cargo xtask check` green on an empty workspace on Windows; the inventory exists.

## Phase 1 – Models

- [ ] `Id<T>`, `Timestamp`, `ImageHash`, bitflags.
- [ ] P0 models including components (Components V2).
- [ ] Gateway envelope, `Event` enum, send payloads.
- [ ] Tests against all documented JSON examples for P0 models.

**Exit:** all P0 models round-trip; unknown fields/enum values tested; `docs/coverage.md` started.

## Phase 2 – REST

- [ ] Client, auth, User-Agent, timeouts, pool.
- [ ] Route system, raw requests.
- [ ] Rate limiter, invalid request guard, retry policy.
- [ ] Multipart.
- [ ] P0 endpoints.

**Exit:** §7.3 load test with zero avoidable 429s; mock REST tests (§17.3) green.

## Phase 3 – Gateway

- [ ] Transport, compression, state machine.
- [ ] Heartbeat, identify, resume, reconnect, close codes.
- [ ] Outbound limiter, member chunking, presence.
- [ ] Event filter, backpressure policies.

**Exit:** all mock gateway scenarios 1–10 and 13–14 (§17.2) green; live connect test green.

## Phase 4 – Sharding

- [ ] Identify queue (local + trait).
- [ ] Shard manager, shard ranges, status, restart.
- [ ] Session start limit handling.

**Exit:** scenarios 11–12 green; live test with 2 shards incl. forced reconnect green.

## Phase 5 – Interactions

- [ ] Command builders + registration.
- [ ] Responses, follow-ups, `InteractionHandle`.
- [ ] HTTP endpoint + signature verification.

**Exit:** live slash command + component + modal round trip; signature test vectors green.

## Phase 6 – Cache (only if required by the ZenyBot inventory; otherwise P1)

- [ ] Resource types, update logic, message ring buffer.
- [ ] Snapshot and concurrency tests.

**Exit:** replayed event sequences produce the expected snapshots.

## Phase 7 – Hardening and release

- [ ] Fuzzing, benchmarks, compare to §16.1, record results in `docs/performance.md`, write `benches/baseline.json`.
- [ ] Thread-per-core vs. multi-thread comparison (§16.4).
- [ ] Examples: `basic_bot`, `manual_sharding`, `automatic_sharding`, `multi_process_sharding`, `thread_per_core`, `interactions_gateway`, `interactions_http`, `raw_requests` (all with `mimalloc`).
- [ ] Docs: rustdoc for every public item, `docs/architecture.md`, `gateway.md`, `sharding.md`, `rate-limits.md`, `performance.md`, `coverage.md`.
- [ ] ZenyBot integration on a staging bot.
- [ ] 72 h soak test.
- [ ] Release 0.1.0.

**Exit:** §22 fully checked.

---

# 21. Timeline

Realistic estimate for one developer working full-time with AI assistance:

| Phase | Effort |
|---|---|
| 0 – Setup + inventory | 1 day |
| 1 – Models | 2–3 days |
| 2 – REST + rate limiter | 3–4 days |
| 3 – Gateway | 4–5 days |
| 4 – Sharding | 2 days |
| 5 – Interactions | 2–3 days |
| 6 – Cache (if needed) | 2 days |
| 7 – Hardening, docs, integration | 3–4 days |
| Soak test | 3 days (runs in parallel with docs work) |
| **Total** | **≈ 4–5 weeks** |

The original 5–10 day target is enough for a working prototype (Phases 0–4 without full hardening), not for production quality. Correctness of the gateway, sharding and rate limiting has priority over endpoint count.

---

# 22. Definition of Done – v0.1.0

Each item must be verifiable by a test, an `xtask` command or a document.

**Build and quality**
- [ ] `cargo xtask check` and `cargo xtask features` green on Windows **and** Debian 13 (acceptance platform).
- [ ] No dependency on any other Discord library (`layout-check`).
- [ ] No voice implementation.
- [ ] `unsafe_code = "forbid"` in every crate; no `unwrap`/`expect` in library code; no unexplained `#[allow]`.
- [ ] `cargo deny check` clean; all builds `--locked`.

**Functionality**
- [ ] All P0 items from §2 implemented and listed in `docs/coverage.md`.
- [ ] Raw REST requests and raw/unknown events work (tested).
- [ ] Rate limiter: zero avoidable 429s in the load test; invalid request guard tested.
- [ ] Gateway: all mock scenarios in §17.2 green.
- [ ] Manual and automatic sharding, identify queue, session start limit: tested.
- [ ] Interactions via Gateway and HTTP, signature verification: tested.
- [ ] Cache (if in scope): snapshot and concurrency tests green.

**Operations**
- [ ] Token never appears in logs/errors (tested).
- [ ] All input size limits in place with defaults (§14).
- [ ] Metrics getters available (§15).
- [ ] 72 h soak test passed (§17.6) with ZenyBot or a test bot.
- [ ] Benchmarks run on the KVM and recorded; `benches/baseline.json` written; deviations from §16.1 explained; allocation budgets met.
- [ ] Panic guard tested (a panicking shard run reconnects, others keep running).

**Documentation**
- [ ] rustdoc for every public item (`missing_docs`, `-D warnings`).
- [ ] README with quick start, feature table, toolchain, license.
- [ ] Examples compile (`--all-targets` in `xtask check`).
- [ ] CHANGELOG entry for 0.1.0.

---

# 23. Risks

| Risk | Impact | Mitigation |
|---|---|---|
| Discord changes payloads or adds events | Deserialization errors | Unknown variants, no `deny_unknown_fields`, raw fallback, changelog watch |
| Rate limiter bug | 429 storms, Cloudflare IP ban | Invalid request guard, load test, conservative defaults |
| Reconnect loop on fatal close codes | Token or IP flagged | Close code table, tests for each code |
| Identify limit exceeded | Session start limit exhausted, shards cannot start for up to 24 h | Identify queue, `remaining` check, waiting for `reset_after` |
| Slow consumer | Memory growth or gateway disconnect | Bounded channels, `Disconnect` policy |
| Scope creep (full API coverage) | Release never happens | Scope tiers, P0 from the ZenyBot inventory |
| Single maintainer | Bus factor | Docs in `docs/`, ADRs for decisions, high test coverage |
| Dependency vulnerabilities / supply chain | Security issues | `cargo-deny`, 3-day publish age, `--locked`, minimal dependencies |
| No CI available | Regressions slip through | `cargo xtask check` mandatory after every change; Debian run before releases |
| ZenyBot is C++ | Integration and soak test delayed | Soak test with a dedicated Rust test bot if the Rust bot is not ready (§17.6) |

---

# 24. Development Rules

1. No dependencies on existing Discord libraries.
2. No voice functionality.
3. No overengineering: implement what P0 needs, keep extension points (traits, raw access) for the rest.
4. Simple, efficient Rust; no macros in the public API unless they clearly pay off.
5. Crates stay independently usable (§4.2).
6. Raw API access always stays available.
7. Never assume Discord payloads stay unchanged.
8. Network input must never cause a panic or a crash.
9. Rate limits and gateway limits are respected without exception.
10. No coverage claim without tests (`docs/coverage.md` is the source of truth).
11. Never claim tests or benchmarks were run unless they actually were.
12. Correctness first, then measured optimization.
13. No ZenyBot-specific logic in the library.
14. Every architectural deviation from this plan is recorded in §0 or as an ADR in `docs/adr/`.
15. After every change: `cargo fmt --all && cargo xtask check`. A step is done only when it is green.
16. Lints are never weakened; single `#[allow(...)]` only with a reason in a comment.
17. New dependencies only when needed and after asking; always the newest version the 3-day rule allows.
18. Code, comments and user-visible texts in English.

## Open items

- [ ] Project name (D1).
- [ ] How ZenyBot consumes the library: Rust port of the C++ bot, or a new Rust bot (§2).
- [ ] Whether the cache is P0 (depends on the inventory).
- [ ] Shared rate limiter / identify queue over Redis (Dragonfly) for multi-process: needed for v0.1 or later.
