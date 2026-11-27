# cordex

[![crates.io][crates badge]][crates link] [![docs.rs][docs badge]][docs link] [![guide][guide badge]][guide] [![CI][ci badge]][ci link] [![discord][discord badge]][discord link] [![license][license badge]][license link] ![rust 1.95+][rust badge]

cordex is an independent, fast Discord bot API library for Rust on Tokio.
It covers the whole official bot API — gateway, sharding, REST, interactions,
an optional cache — and adds a simple layer, so a working bot takes a few
lines. Low-level access stays available, but you never need it.

Read the full [cordex Guide][guide] (quickstart, commands, caching, migration from Serenity/Twilight, FAQ) or join the [cordex Discord server][discord link] for questions and feedback.

## Installation

```toml
[dependencies]
cordex = "0.2.2"
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

Or scaffold a ready-to-run bot with [`cargo-generate`][template]:

```bash
cargo generate lejylabs/cordex-template
```

Rust 1.95 or newer. Pure-Rust voice signaling (`voice`) and Lavalink v4 (`lavalink`) are available as opt-in features (no C/FFI audio transport). See the [cordex Guide][guide] for walkthroughs and deployment notes.

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

## Examples

One small file per task in [`crates/facade/examples`][examples]:

| Example | Shows |
|---|---|
| [`bot.rs`][ex-bot] | Commands, options, autocomplete, buttons, modals, text commands |
| [`buttons_and_waiting.rs`][ex-wait] | `wait_for_button` (confirm dialog) and `button_clicks` (a poll) |
| [`components_v2.rs`][ex-v2] | Components V2 layout: container, section, thumbnail, gallery |
| [`embeds_and_modals.rs`][ex-embeds] | Embed builder, CDN avatar, relative timestamps, modal with radio group and checkbox |
| [`cache_and_permissions.rs`][ex-cache] | Opt-in cache and permissions without a REST call (feature `cache`) |
| [`manual_sharding.rs`][ex-shard] | Low-level shard range for multi-process bots, raw events, event filter |
| [`thread_per_core.rs`][ex-tpc] | Pin shards onto dedicated `current_thread` Tokio runtimes (`RuntimeHandles`) |
| [`http_only.rs`][ex-http] | REST without a gateway, safe default mentions, webhook URLs |
| [`interactions_http.rs`][ex-ihttp] | Interactions over HTTP with signature check (feature `interactions-server`) |
| [`voice_lavalink.rs`][ex-voice] | Voice signaling with `Bot::voice()` and Lavalink v4 payload (feature `voice`) |

## What is included

- **Simple layer** (`cordex::bot`): `Bot` (and `Bot::from_env()`) with plain
  async closures for commands, autocomplete, buttons, select menus, modals,
  messages and any event; modular bot composition (`BotModule` / `Bot::mount`),
  prefix and `{param}` pattern component/modal routing, command-level cooldowns
  (`command_with_cooldown`), typed shared state (`Bot::data` /
  `ctx.data::<T>()`), `on_error` and `before_command` hooks, graceful shutdown
  (`run_until_ctrl_c()` / `run_until()`), `ctx.paginate(...)`,
  `ctx.confirm(...)` / `ctx.select(...)` / `ctx.prompt(...)` /
  `ctx.ask_modal(...)`, `ctx.send(...)` / `ctx.dm(...)`,
  `ctx.wait_for_button` / `button_clicks` (with `.idle_timeout()`, `.max()`,
  `.filter()`, `.end_reason()`) / `wait_for_message` / `wait_for_reaction`,
  interaction guards (`require_user_perm` / `require_bot_perm` /
  `require_guild` / `require_nsfw` / `require_role` / `require_entitlement` /
  `require_owner`), and automatic resharding while running when Discord
  recommends more shards.
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
- **Components**: builders for every component, including the Components V2
  layout (container, section, text display, thumbnail, media gallery, file,
  separator); the `IS_COMPONENTS_V2` flag is set automatically.
- **Helpers**: `MessageBuilder` with Markdown escaping, embed builder with
  Discord's limits (`Embed::success`/`error`/`info`/`warn` + `colors`), short
  ID aliases (`GuildId`, `ChannelId`, `UserId`, `RoleId`, `MessageId`, …), CDN
  URLs (avatars, icons, emojis, stickers), mentions and timestamp markdown,
  mention parsing, webhook/invite/message URL parsing, permission calculator,
  default allowed mentions.

## Crates

Most bots only need `cordex`. The parts work on their own too:

| Crate | Content |
|---|---|
| [`cordex`][crates link] | Facade: simple layer `Bot` & `BotModule`, re-exports behind features, `prelude`, `backfill` |
| [`cordex-model`][model] | Models, snowflakes & ID aliases, flags, gateway payloads, CDN URLs, mentions, permission calculator (no async) |
| [`cordex-http`][http] | REST client, rate limiter, invalid request guard |
| [`cordex-gateway`][gateway] | Shards, shard manager, identify queue, `Standby` |
| [`cordex-interactions`][interactions] | Command builders, responses, modals, HTTP endpoint |
| [`cordex-cache`][cache] | Optional in-memory cache, every resource and part switchable |
| [`cordex-voice`][voice] | Optional pure-Rust voice signaling & session coordination (Lavalink v4 & custom backends) |
| [`cordex-lavalink`][lavalink] | Optional pure-Rust Lavalink v4 client (WebSocket + REST + `cordex-voice` bridge) |
| [`cordex-migrate`][migrate] | Zero-dependency CLI to analyze and mechanically migrate Serenity/Poise & Twilight bots to `cordex` |
| [`cordex-test`][test] | Offline testing harness (`TestBot`, mock HTTP recorder, deterministic time, `.jsonl` gateway replay) |
| [`cordex-macros`][macros] | Optional derive macros (`#[derive(CommandArgs)]`, `#[derive(CommandChoice)]`) |
| [`cordex-axum`][axum] | Axum integration (`VerifiedInteraction` Ed25519 extractor, `interactions_router`, `/healthz` & `/readyz`) |

## Features

| Feature | Default | Content |
|---|---|---|
| `http` | yes | REST client (`cordex-http`) |
| `http2` | yes | Negotiate HTTP/2 via ALPN on the REST client (disable for smaller binaries) |
| `gateway` | yes | Shards and shard manager (`cordex-gateway`) |
| `interactions` | yes | Command builders and responses (`cordex-interactions`) |
| `bot` | yes | Simple layer: `Bot` and handler contexts |
| `cache` | no | In-memory cache (`cordex-cache`) and `cordex::backfill` |
| `voice` | no | Pure-Rust voice signaling (`cordex-voice`, `Bot::voice()`, `ctx.voice()`) |
| `lavalink` | no | Pure-Rust Lavalink v4 client (`cordex-lavalink`, enables `voice`) |
| `interactions-verify` | no | `verify_signature()` for HTTP interactions in any web framework |
| `interactions-server` | no | Standalone hyper endpoint for HTTP interactions |
| `oauth2` | no | Discord `OAuth2` client in `cordex::http::oauth2` (PKCE, token exchange/refresh/revoke, Bearer endpoints) |
| `macros` | no | Optional derive macros (`#[derive(CommandArgs)]`, `#[derive(CommandChoice)]`) |
| `postgres-types` | no | Direct PostgreSQL `ToSql` / `FromSql` for `Id<T>`, `Timestamp`, `ImageHash`, `Locale`, `u64` bitflags, `u8` enums, and `JSONB`/`JSON` (`Embed`, `Component`, `AllowedMentions`) |
| `rusqlite` | no | Direct SQLite `ToSql` / `FromSql` for `Id<T>`, `Timestamp`, `ImageHash`, `Locale`, `u64` bitflags, and `u8` enums |
| `redis` | no | Direct Redis `ToRedisArgs` / `FromRedisValue` for `Id<T>`, `Timestamp`, `ImageHash`, `Locale`, `u64` bitflags, `u8` enums, plus `RedisSessionStore` in `cordex-gateway` |
| `mysql` | no | Direct MySQL/MariaDB (`mysql_common`) value conversions for `Id<T>`, `Timestamp`, `ImageHash`, `Locale`, `u64` bitflags, and `u8` enums |
| `simd-json` | no | Opt-in SIMD-accelerated JSON deserialization (`cordex_model::simd_json::from_slice`) |
| `metrics` | no | Export HTTP and gateway counters through the `metrics` crate |
| `zstd-stream` | no | Use `zstd-stream` gateway compression instead of pure-Rust `zlib-stream` (requires a C compiler) |

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
# Optimize the serde-heavy model crate for size without slowing down gateway/http/cache:
[profile.release.package.cordex-model]
opt-level = "s"
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
[examples]: https://github.com/lejylabs/cordex/tree/master/crates/facade/examples
[ex-bot]: https://github.com/lejylabs/cordex/blob/master/crates/facade/examples/bot.rs
[ex-wait]: https://github.com/lejylabs/cordex/blob/master/crates/facade/examples/buttons_and_waiting.rs
[ex-v2]: https://github.com/lejylabs/cordex/blob/master/crates/facade/examples/components_v2.rs
[ex-embeds]: https://github.com/lejylabs/cordex/blob/master/crates/facade/examples/embeds_and_modals.rs
[ex-cache]: https://github.com/lejylabs/cordex/blob/master/crates/facade/examples/cache_and_permissions.rs
[ex-shard]: https://github.com/lejylabs/cordex/blob/master/crates/facade/examples/manual_sharding.rs
[ex-tpc]: https://github.com/lejylabs/cordex/blob/master/crates/facade/examples/thread_per_core.rs
[ex-http]: https://github.com/lejylabs/cordex/blob/master/crates/facade/examples/http_only.rs
[ex-ihttp]: https://github.com/lejylabs/cordex/blob/master/crates/facade/examples/interactions_http.rs
[ex-voice]: https://github.com/lejylabs/cordex/blob/master/crates/facade/examples/voice_lavalink.rs
[template]: https://github.com/lejylabs/cordex-template
[guide]: https://lejylabs.github.io/cordex/
[guide badge]: https://img.shields.io/badge/guide-cordex_docs-1f6feb.svg?style=for-the-badge
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
[voice]: https://crates.io/crates/cordex-voice
[lavalink]: https://crates.io/crates/cordex-lavalink
[migrate]: https://crates.io/crates/cordex-migrate
[test]: https://crates.io/crates/cordex-test
[macros]: https://crates.io/crates/cordex-macros
[axum]: https://crates.io/crates/cordex-axum
