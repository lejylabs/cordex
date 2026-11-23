# cordex

Independent, fast Discord bot API library for Rust on Tokio. No voice.

> **Beta.** The API can still change between `0.x` releases. See
> `CHANGELOG.md` for what is in each release.

Questions, feedback or help: join the [cordex Discord server](https://discord.gg/Xj9nRKyKNV).

- **Gateway**: zlib-stream compression, heartbeat with zombie detection,
  resume/reconnect, close-code handling, outbound rate limit, event filter,
  backpressure, panic isolation per shard.
- **Sharding**: automatic or manual shard ranges, identify queue with
  `max_concurrency`, session start limit handling, thread-per-core placement.
- **REST**: per-route and global rate limiter, invalid request guard,
  retries, multipart uploads, local validation before sending, raw requests
  for anything not typed yet.
- **Simple layer**: `Bot` with async closures as handlers for commands,
  autocomplete, buttons, modals and messages; a working bot in ~20 lines.
- **Interactions**: slash/user/message command builders with local
  validation, typed option access, `InteractionHandle` with auto-defer and
  double-response protection, HTTP endpoint with Ed25519 verification.
- **Models**: typed Discord objects and events, unknown values and events are
  kept instead of failing.

## Quick start

```toml
[dependencies]
cordex = "0.1.0-beta.2"
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

```rust,no_run
use cordex::prelude::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    Bot::new(std::env::var("DISCORD_TOKEN")?)
        .command(
            Command::slash("ping", "Replies with pong"),
            |ctx: CommandContext| async move { ctx.reply("Pong!").await },
        )
        .command(
            Command::slash("greet", "Greets a user")
                .option(CommandOption::user("user", "Who to greet").required()),
            |ctx: CommandContext| async move {
                let user = ctx.user_option("user")?;
                ctx.reply(format!("Hello {}!", user.mention())).await
            },
        )
        .on_message(|ctx: MessageContext| async move {
            if ctx.content() == "!hi" {
                ctx.reply("Hi!").await?;
            }
            Ok(())
        })
        .run()
        .await?;
    Ok(())
}
```

`Bot` derives the intents from your handlers, shards automatically,
registers the commands on start, defers slow interactions on its own and
logs handler errors instead of crashing. Buttons, select menus, modals and
autocomplete work the same way (`on_button`, `on_modal`, `autocomplete`):
see [`examples/bot.rs`](https://github.com/lejylabs/cordex/blob/master/crates/facade/examples/bot.rs).

Everything below the simple layer stays available: `ShardManager`, the REST
`Client`, `InteractionHandle` and raw events. The live test bot in
[`examples/testbot.rs`](https://github.com/lejylabs/cordex/blob/master/crates/facade/examples/testbot.rs)
uses those directly.

## Features

| Feature | Default | Content |
|---|---|---|
| `http` | yes | REST client (`cordex-http`) |
| `gateway` | yes | shards and shard manager (`cordex-gateway`) |
| `interactions` | yes | command builders, responses (`cordex-interactions`) |
| `bot` | yes | simple layer: `Bot` and handler contexts (needs `http`, `gateway`, `interactions`) |
| `interactions-verify` | no | `verify_signature()` for HTTP interactions in any web framework |
| `interactions-server` | no | standalone hyper endpoint for HTTP interactions |
| `cache` | no | in-memory cache (`cordex-cache`); every resource inside is off until enabled |

The library sets no global allocator; applications may use `mimalloc`.

## Crates

| Crate | Content |
|---|---|
| `cordex` | facade, simple layer (`Bot`), re-exports behind features, `prelude` |
| `cordex-model` | models, snowflakes, flags, gateway payloads (no async) |
| `cordex-http` | REST client, rate limiter |
| `cordex-gateway` | shards, shard manager, identify queue |
| `cordex-interactions` | commands, responses, HTTP endpoint |
| `cordex-cache` | optional in-memory cache, every resource and part switchable |

## Requirements

Rust 1.95 or newer (MSRV, edition 2024), Tokio. Raising the MSRV counts as a
minor (`0.x`) change and is noted in the changelog. TLS uses rustls with aws-lc-rs
and the platform certificate verifier.

## Community

- [Discord server](https://discord.gg/Xj9nRKyKNV) for questions, help and showing what you built.
- Bugs and feature requests: [GitHub issues](https://github.com/lejylabs/cordex/issues).
- Contributions: see [CONTRIBUTING.md](https://github.com/lejylabs/cordex/blob/master/CONTRIBUTING.md).

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
cargo run -p cordex --example testbot --features cache   # live test bot, reads DISCORD_TOKEN from .env
```
