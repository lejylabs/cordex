# cordex

[![crates.io][crates badge]][crates link] [![docs.rs][docs badge]][docs link] [![CI][ci badge]][ci link] [![discord][discord badge]][discord link] [![license][license badge]][license link] ![rust 1.95+][rust badge]

cordex is an independent, fast Discord bot API library for Rust on Tokio.
It covers the whole official bot API — gateway, sharding, REST, interactions,
an optional cache — and adds a simple layer, so a working bot takes a few
lines. Low-level access stays available, but you never need it.

> **Beta.** The API can still change between `0.x` releases (see the
> [changelog]). `0.1.0` is the first regular release. Coverage of the
> official docs is measured, not estimated: [`docs/coverage.md`][coverage]
> (currently 100 % of all endpoints, events, opcodes, enum values, object
> fields and flags).

Questions, feedback or help: join the [cordex Discord server][discord link].

## Installation

```toml
[dependencies]
cordex = "0.1.0-beta.4"
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

Rust 1.95 or newer. No voice support, by design.

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

`Bot` derives the intents and the gateway event filter from your handlers,
shards automatically, registers the commands on start (only when they
changed), defers slow interactions on its own and logs handler errors and
panics instead of crashing.

### Buttons, embeds and waiting for a click

```rust,no_run
use std::time::Duration;

use cordex::prelude::*;

async fn run(token: String) -> cordex::Result<()> {
    Bot::new(token)
        .command(
            Command::slash("profile", "Shows a user")
                .option(CommandOption::user("user", "Who").required()),
            |ctx: CommandContext| async move {
                let user = ctx.user_option("user")?;
                let embed = Embed::new()
                    .title(user.username.as_ref())?
                    .field(
                        "Account created",
                        user.id.created_at().markdown(TimestampStyle::Relative),
                        true,
                    )?
                    .thumbnail(user.avatar_url().size(256).to_string());
                ctx.reply(Reply::embed(embed)).await
            },
        )
        .command(
            Command::slash("confirm", "Asks before doing something"),
            |ctx: CommandContext| async move {
                let row = ActionRow::new([
                    Button::success("yes", "Yes"),
                    Button::danger("no", "No"),
                ]);
                ctx.defer().await?;
                let question = Reply::new("Are you sure?").components(vec![row.into()]);
                let message = ctx.edit(question).await?;
                // Wait right here for the click; no state maps needed.
                match ctx.wait_for_button(message.id, Duration::from_secs(30)).await {
                    Some(click) => {
                        click.update(format!("You chose {}", click.custom_id())).await
                    }
                    None => ctx.edit("Timed out.").await.map(|_| ()),
                }
            },
        )
        .run()
        .await
}
```

More in [`examples/bot.rs`][example]: autocomplete, select menus, modals and
text commands.

## What is included

- **Simple layer** (`cordex::bot`): `Bot` with plain async closures for
  commands, autocomplete, buttons, select menus, modals, messages and any
  event; `ctx.wait_for_button` / `wait_for_message` / `wait_for_reaction`.
- **Gateway**: zlib-stream compression, heartbeat with zombie detection,
  resume and reconnect, close codes with fix hints, outbound rate limit,
  event filter (unwanted events are not parsed), backpressure policies,
  panic isolation per shard.
- **Sharding**: automatic or manual shard ranges, identify queue with
  `max_concurrency`, session start limits, thread-per-core placement.
- **REST**: all 235 endpoints typed, per-route and global rate limiter,
  invalid request guard against Cloudflare bans, retries, multipart uploads,
  local validation before sending, raw requests for anything new.
- **Interactions**: command builders with local validation, typed option
  access, `InteractionHandle` with auto-defer and double-answer protection,
  modals, HTTP interaction endpoint with Ed25519 verification.
- **Models**: every Discord object and event typed; unknown fields, enum
  values and events are kept instead of failing.
- **Cache** (optional, off by default): every resource and every part is its
  own switch, limits per resource, switchable at runtime, permission
  calculation, backfill of old messages and members.
- **Helpers**: embed builder with Discord's limits, CDN URLs (avatars, icons,
  emojis, stickers), mentions and timestamp markdown, mention parsing,
  webhook URL parsing, permission calculator, default allowed mentions.

## Crates

Most bots only need `cordex`. The parts work on their own too:

| Crate | Content |
|---|---|
| [`cordex`][crates link] | Facade: simple layer `Bot`, re-exports behind features, `prelude`, `backfill` |
| [`cordex-model`][model] | Models, snowflakes, flags, gateway payloads, CDN URLs, mentions, permission calculator (no async) |
| [`cordex-http`][http] | REST client, rate limiter, invalid request guard |
| [`cordex-gateway`][gateway] | Shards, shard manager, identify queue, `Standby` |
| [`cordex-interactions`][interactions] | Command builders, responses, modals, HTTP endpoint |
| [`cordex-cache`][cache] | Optional in-memory cache, every resource and part switchable |

## Features

| Feature | Default | Content |
|---|---|---|
| `http` | yes | REST client (`cordex-http`) |
| `gateway` | yes | Shards and shard manager (`cordex-gateway`) |
| `interactions` | yes | Command builders and responses (`cordex-interactions`) |
| `bot` | yes | Simple layer: `Bot` and handler contexts |
| `cache` | no | In-memory cache (`cordex-cache`) and `cordex::backfill` |
| `interactions-verify` | no | `verify_signature()` for HTTP interactions in any web framework |
| `interactions-server` | no | Standalone hyper endpoint for HTTP interactions |

TLS uses rustls with aws-lc-rs and the platform certificate verifier. The
library sets no global allocator; applications may use `mimalloc`.

## Smaller and faster binaries

cordex is built for low memory use: unwanted gateway events are skipped
before parsing, nothing is cached unless you switch it on, and every queue
is bounded. For your own release builds:

```toml
[profile.release]
lto = "fat"          # whole-program optimization
codegen-units = 1
strip = "debuginfo"
# opt-level = "z"    # smallest binary, slightly slower
# panic = "abort"    # smaller, but a panic ends the process
#                    # (cordex then cannot keep other handlers running)
```

## Coverage and the docs watch

`cargo xtask coverage` compares the library with a pinned commit of the
official [Discord API docs][api docs] and writes [`docs/coverage.md`][coverage].
A daily workflow opens an issue with a checklist when Discord's docs change,
so new endpoints and fields land quickly.

## Community

- [Discord server][discord link] for questions, help and showing what you built.
- Bugs and feature requests: [GitHub issues][issues].
- Contributing: see [CONTRIBUTING.md][contributing]. Security issues: [SECURITY.md][security].

## License

Licensed under either of [Apache License, Version 2.0][apache] or
[MIT license][mit] at your option. Unless you explicitly state otherwise, any
contribution intentionally submitted for inclusion in this crate by you, as
defined in the Apache-2.0 license, shall be dual licensed as above, without
any additional terms or conditions.

[crates badge]: https://img.shields.io/crates/v/cordex.svg?style=for-the-badge&logo=rust
[crates link]: https://crates.io/crates/cordex
[docs badge]: https://img.shields.io/docsrs/cordex?style=for-the-badge&logo=docsdotrs
[docs link]: https://docs.rs/cordex
[ci badge]: https://img.shields.io/github/actions/workflow/status/lejylabs/cordex/ci.yml?branch=master&style=for-the-badge&logo=github
[ci link]: https://github.com/lejylabs/cordex/actions/workflows/ci.yml
[discord badge]: https://img.shields.io/badge/discord-join-5865F2.svg?style=for-the-badge&logo=discord&logoColor=white
[discord link]: https://discord.gg/Xj9nRKyKNV
[license badge]: https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg?style=for-the-badge
[license link]: https://github.com/lejylabs/cordex#license
[rust badge]: https://img.shields.io/badge/rust-1.95+-93450a.svg?style=for-the-badge&logo=rust
[changelog]: https://github.com/lejylabs/cordex/blob/master/CHANGELOG.md
[coverage]: https://github.com/lejylabs/cordex/blob/master/docs/coverage.md
[example]: https://github.com/lejylabs/cordex/blob/master/crates/facade/examples/bot.rs
[api docs]: https://github.com/discord/discord-api-docs
[issues]: https://github.com/lejylabs/cordex/issues
[contributing]: https://github.com/lejylabs/cordex/blob/master/CONTRIBUTING.md
[security]: https://github.com/lejylabs/cordex/blob/master/SECURITY.md
[apache]: https://github.com/lejylabs/cordex/blob/master/LICENSE-APACHE
[mit]: https://github.com/lejylabs/cordex/blob/master/LICENSE-MIT
[model]: https://crates.io/crates/cordex-model
[http]: https://crates.io/crates/cordex-http
[gateway]: https://crates.io/crates/cordex-gateway
[interactions]: https://crates.io/crates/cordex-interactions
[cache]: https://crates.io/crates/cordex-cache
