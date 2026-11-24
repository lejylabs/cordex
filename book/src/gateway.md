# Gateway, Intents & Events

`cordex-gateway` manages WebSocket connections, heartbeating, session resuming, transport compression, and event filtering.

## Gateway Intents

When using `Bot`, non-privileged intents are derived automatically from your registered handlers and cache config. **Privileged intents are never enabled implicitly** — you must enable them in the Discord Developer Portal *and* opt in explicitly:

```rust,no_run
use cordex::prelude::*;

let bot = Bot::new(token)
    .intents(Intents::GUILD_MEMBERS | Intents::MESSAGE_CONTENT);
```

| Privileged Intent | Needed for |
|---|---|
| `Intents::GUILD_MEMBERS` | `GUILD_MEMBER_ADD/UPDATE/REMOVE`, `GUILD_MEMBERS_CHUNK`, full member lists |
| `Intents::GUILD_PRESENCES` | `PRESENCE_UPDATE`, online status & activities |
| `Intents::MESSAGE_CONTENT` | Reading `message.content`, `embeds`, `attachments`, `components` in guild messages (DMs and mentions always work without it) |

## Event Filtering (`EventTypeFlags`)

When using `cordex-gateway` directly (without `Bot`), pass `EventTypeFlags` to `ShardBuilder` so unneeded dispatch events are skipped *before* JSON deserialization — saving significant CPU and allocations on busy shards (~0.36 µs per filtered event).

## Transport Compression (`zlib-stream` vs. `zstd-stream`)

- **Default (`zlib-stream`):** Pure-Rust via `flate2` (`zlib-rs`), zero C dependencies, zero-copy fast path for single-frame messages.
- **Optional (`zstd-stream`):** Enable the `zstd-stream` feature on `cordex` or `cordex-gateway` for lower bandwidth and faster decompression (requires a C compiler to build `libzstd`).

## Bot Presence & Status

Set the initial presence on `Bot` or update it dynamically from any handler:

```rust,no_run
use cordex::prelude::*;

Bot::new(token)
    .presence(UpdatePresence::online().activity(Activity::playing("with cordex")));
```

Inside a handler, call `ctx.set_presence(UpdatePresence::...)`.

## Decompressed Message Cap & Metrics

- **`max_message_bytes`**: Configured on `ShardBuilder` / `ShardManagerBuilder` (default 16 MiB) to protect against oversized compressed payloads.
- **`metrics` feature**: Enable `features = ["metrics"]` to export gateway counters (identifies, resumes, dispatches, dropped events, bytes received/decompressed) through the `metrics` crate, or read them directly from `ShardStatus::counters`.
