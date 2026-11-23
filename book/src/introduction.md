# Introduction

`cordex` is an independent, fast Discord v10 bot API library for Rust on Tokio.

- **100 % measured bot API coverage** against `discord-api-docs` (`docs/coverage.md`).
- **Simple layer (`cordex::bot`)** for everyday bots in ~20 lines, built directly on top of the low-level crates.
- **Modular crates:** `cordex-model`, `cordex-http`, `cordex-gateway`, `cordex-interactions`, `cordex-cache`, and the `cordex` facade.
- **No `unsafe`**, no hidden caching, no voice support by design.

## Installation

Add `cordex` and `tokio` to your `Cargo.toml` (Rust 1.95 or newer):

```toml
[dependencies]
cordex = "0.1.1"
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
| `gateway` | yes | Shards, manager, `Standby` (`cordex-gateway`) |
| `interactions` | yes | Command/modal builders, `InteractionHandle` |
| `bot` | yes | Simple layer (`cordex::bot::Bot`) |
| `cache` | no | Opt-in in-memory cache (`cordex-cache`) + `cordex::backfill` |
| `interactions-verify` | no | Ed25519 `verify_signature()` for any web framework |
| `interactions-server` | no | Standalone Hyper HTTP interaction endpoint |

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
