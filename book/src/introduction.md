# Introduction

`cordex` is an independent, fast Discord v10 bot API library for Rust on Tokio.

- **100 % measured bot API coverage** against `discord-api-docs` (`docs/coverage.md`).
- **Simple layer (`cordex::bot`)** for everyday bots in ~20 lines, built directly on top of the low-level crates.
- **Modular crates:** `cordex-model`, `cordex-http`, `cordex-gateway`, `cordex-interactions`, `cordex-cache`, `cordex-voice`, `cordex-lavalink`, `cordex-migrate` (CLI), and the `cordex` facade.
- **No `unsafe`**, no hidden caching, and pure-Rust opt-in voice signaling & Lavalink v4 (no C/FFI audio transport).

## Installation

Add `cordex` and `tokio` to your `Cargo.toml` (Rust 1.95 or newer):

```toml
[dependencies]
cordex = "0.2.1"
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

Or start from the official [`cargo-generate` template](https://github.com/lejylabs/cordex-template):

```bash
cargo generate lejylabs/cordex-template
```

## Feature flags

| Feature | Default | Enables |
|---|---|---|
| `http` | yes | REST client (`cordex-http`) |
| `http2` | yes | Negotiate HTTP/2 via ALPN on the REST client (disable for smaller binaries) |
| `gateway` | yes | Shards, manager, `Standby` (`cordex-gateway`) |
| `interactions` | yes | Command/modal builders, `InteractionHandle` |
| `bot` | yes | Simple layer (`cordex::bot::Bot`) |
| `cache` | no | Opt-in in-memory cache (`cordex-cache`) + `cordex::backfill` |
| `voice` | no | Opt-in pure-Rust voice signaling (`cordex-voice`, `Bot::voice()`, `ctx.voice()`) |
| `lavalink` | no | Opt-in pure-Rust Lavalink v4 client (`cordex-lavalink`, enables `voice`) |
| `interactions-verify` | no | Ed25519 `verify_signature()` for any web framework |
| `interactions-server` | no | Standalone Hyper HTTP interaction endpoint |
| `oauth2` | no | Discord `OAuth2` client in `cordex::http::oauth2` (PKCE, token exchange/refresh/revoke, Bearer endpoints) |
| `macros` | no | Optional derive macros (`#[derive(CommandArgs)]`, `#[derive(CommandChoice)]`) |
| `postgres-types` | no | Direct PostgreSQL `ToSql` / `FromSql` for `Id<T>`, `Permissions`, `Timestamp`, `ImageHash` |
| `rusqlite` | no | Direct SQLite `ToSql` / `FromSql` for `Id<T>`, `Permissions`, `Timestamp`, `ImageHash` |
| `redis` | no | Direct Redis `ToRedisArgs` / `FromRedisValue` for `Id<T>`, `Permissions`, `Timestamp`, `ImageHash` |
| `mysql` | no | Direct MySQL/MariaDB (`mysql_common`) value conversions for `Id<T>`, `Permissions`, `Timestamp`, `ImageHash` |
| `simd-json` | no | Opt-in SIMD-accelerated JSON deserialization (`cordex_model::simd_json::from_slice`) |
| `metrics` | no | Export HTTP and gateway counters through the `metrics` crate |
| `zstd-stream` | no | Use `zstd-stream` gateway compression instead of pure-Rust `zlib-stream` (requires a C compiler) |

## Quick start

```rust,no_run
use cordex::prelude::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    Bot::new(std::env::var("DISCORD_TOKEN")?)
        .command(
            Command::slash("ping", "Replies with pong"),
            |ctx: CommandContext| async move { ctx.reply("Pong!").await },
        )
        .run()
        .await?;
    Ok(())
}
```
