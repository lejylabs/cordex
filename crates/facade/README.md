# cordex

Independent, fast Discord bot API library for Rust on Tokio. No voice.

> **Beta.** The API can still change between `0.x` releases. See
> `CHANGELOG.md` for what is in each release.

- **Gateway**: zlib-stream compression, heartbeat with zombie detection,
  resume/reconnect, close-code handling, outbound rate limit, event filter,
  backpressure, panic isolation per shard.
- **Sharding**: automatic or manual shard ranges, identify queue with
  `max_concurrency`, session start limit handling, thread-per-core placement.
- **REST**: per-route and global rate limiter, invalid request guard,
  retries, multipart uploads, local validation before sending, raw requests
  for anything not typed yet.
- **Interactions**: slash/user/message command builders with local
  validation, typed option access, `InteractionHandle` with auto-defer and
  double-response protection, HTTP endpoint with Ed25519 verification.
- **Models**: typed Discord objects and events, unknown values and events are
  kept instead of failing.

## Quick start

```toml
[dependencies]
cordex = "0.1"
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

```rust,no_run
use cordex::gateway::{ManagerEvent, ShardManager};
use cordex::model::interaction::InteractionData;
use cordex::prelude::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let token = std::env::var("DISCORD_TOKEN")?;
    let http = Client::new(&token)?;

    let mut manager = ShardManager::builder(&token, Intents::GUILDS)
        .http(http.clone())
        .build()
        .await?;
    let mut events = manager.events();
    manager.start().await?;

    while let Some(event) = events.recv().await {
        let ManagerEvent::Dispatch { event, .. } = event else { continue };
        match event {
            Event::Ready(ready) => {
                // Register `/ping` in every guild (shows up instantly).
                for guild in &ready.guilds {
                    http.bulk_overwrite_guild_commands(
                        ready.application.id,
                        guild.id,
                        [Command::slash("ping", "Replies with pong")],
                    )
                    .await?;
                }
            }
            Event::InteractionCreate(interaction) => {
                let handle = InteractionHandle::new(http.clone(), &interaction);
                if let Some(InteractionData::Command(data)) = &interaction.data {
                    if &*data.name == "ping" {
                        handle.respond(Reply::new("pong").ephemeral()).await?;
                    }
                }
            }
            _ => {}
        }
    }
    Ok(())
}
```

A fuller example with text commands, autocomplete, auto-defer and follow-ups
is in [`crates/facade/examples/testbot.rs`](https://github.com/lejylabs/cordex/blob/master/crates/facade/examples/testbot.rs).

## Features

| Feature | Default | Content |
|---|---|---|
| `http` | yes | REST client (`cordex-http`) |
| `gateway` | yes | shards and shard manager (`cordex-gateway`) |
| `interactions` | yes | command builders, responses (`cordex-interactions`) |
| `interactions-verify` | no | `verify_signature()` for HTTP interactions in any web framework |
| `interactions-server` | no | standalone hyper endpoint for HTTP interactions |

The library sets no global allocator; applications may use `mimalloc`.

## Crates

| Crate | Content |
|---|---|
| `cordex` | facade, re-exports behind features, `prelude` |
| `cordex-model` | models, snowflakes, flags, gateway payloads (no async) |
| `cordex-http` | REST client, rate limiter |
| `cordex-gateway` | shards, shard manager, identify queue |
| `cordex-interactions` | commands, responses, HTTP endpoint |

## Requirements

Rust 1.100 or newer (edition 2024), Tokio. TLS uses rustls with aws-lc-rs
and the platform certificate verifier.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option. Unless you explicitly state
otherwise, any contribution intentionally submitted for inclusion in this
crate by you, as defined in the Apache-2.0 license, shall be dual licensed as
above, without any additional terms or conditions.

## Develop

Toolchain pinned in `rust-toolchain.toml`; dependencies need 3 days of publish
age (`.cargo/config.toml`); builds use `--locked`.

```bash
cargo fmt --all
cargo xtask check          # fmt, clippy, tests, docs, cargo-deny, layout-check
cargo run -p cordex --example testbot   # live test bot, reads DISCORD_TOKEN from .env
```
