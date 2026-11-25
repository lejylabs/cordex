# Migrating from Serenity & Twilight

`cordex` combines the **modular, zero-overhead architecture** of Twilight with an **ergonomic batteries-included layer (`Bot`)** similar to Serenity/Poise — without proc-macro magic or hidden background caching.

## Automated First Pass (`cordex-migrate`)

Before editing files by hand, run the zero-dependency `cordex-migrate` CLI on your repository:

```bash
# 1. Inspect Cargo.toml + src/**/*.rs and get the exact recommended cordex features
cargo run -p cordex-migrate -- analyze path/to/your-bot

# 2. Preview safe mechanical rewrites & inline TODO notes (dry-run by default)
cargo run -p cordex-migrate -- apply path/to/your-bot

# 3. Write changes in place (idempotent — safe to re-run)
cargo run -p cordex-migrate -- apply path/to/your-bot --write
```

`cordex-migrate` performs only deterministic 1:1 rewrites (such as `twilight_*` module imports and `GatewayIntents::` -> `Intents::`) and inserts `// TODO(cordex-migrate): ...` guidance comments above structural patterns (`#[poise::command]`, `impl EventHandler`, `impl TypeMapKey`, Serenity builders, Twilight shard/cache setup) that link directly to the recipes below.

---

## Architectural Comparison

| Aspect | Serenity (+ Poise) | Twilight | `cordex` |
|---|---|---|---|
| **Crate structure** | Monolithic crate | Separate crates (`twilight-*`) | Modular `cordex-*` crates + unified `cordex` facade |
| **Simple bot layer** | `EventHandler` / Poise macros | Manual event loop | Built-in `cordex::bot::Bot` (plain async closures, no proc-macros) |
| **Caching** | On by default when `cache` feature is enabled | `twilight-cache-inmemory` (`ResourceType`) | Opt-in `cordex-cache` with `Off` / `Lazy` / `Eager` + `CacheParts` field stripping |
| **Gateway compression** | `zlib-stream` | `zlib-stream` / `zstd-stream` | Pure-Rust `zlib-stream` default + optional `zstd-stream` |
| **HTTP/2** | via `reqwest` | HTTP/1.1 or HTTP/2 | HTTP/2 via ALPN by default (disable `http2` feature for smaller binaries) |
| **Voice** | via `songbird` | via `songbird` / `twilight-lavalink` | Opt-in pure-Rust `cordex-voice` & `cordex-lavalink` (no C/FFI audio transport) |
| **Unsafe code** | Some in deps | Minimal | `#![forbid(unsafe_code)]` across the workspace |

---

## Crate Mapping

| Serenity / Twilight Crate | `cordex` Equivalent | Feature on `cordex` |
|---|---|---|
| `serenity` (REST + Gateway) | `cordex` (or `cordex-http` + `cordex-gateway`) | default (`http`, `gateway`, `interactions`, `bot`) |
| `poise` | `cordex::bot` (`Bot`, `Command`, `CommandContext`) | default (`bot`) |
| `twilight-model` | `cordex-model` (`cordex::model`) | always enabled |
| `twilight-http` | `cordex-http` (`cordex::http`) | `http` (default) |
| `twilight-gateway` | `cordex-gateway` (`cordex::gateway`) | `gateway` (default) |
| `twilight-gateway-queue` | `cordex-gateway::identify` (`IdentifyQueue`, `LocalIdentifyQueue`) | `gateway` (default) |
| `twilight-standby` | `cordex-gateway::Standby` (`ctx.wait_for_*`) | `gateway` / `bot` (default) |
| `twilight-cache-inmemory` | `cordex-cache` (`cordex::cache`) | `cache` (opt-in) |
| `twilight-mention` | `cordex-model::mention` (`Mentionable`, `parse_mentions`) | always enabled |
| `twilight-util` | `cordex-model` (`permission_calc`, `cdn`, `parse_message_link`, `parse_webhook_url`, `parse_invite_url`) + `cordex-interactions` | always / `interactions` |
| `twilight-lavalink` | `cordex-lavalink` (`cordex::lavalink`) | `lavalink` (opt-in, enables `voice`) |
| `songbird` | `cordex-voice` (signaling only) / `cordex-lavalink` (no C/FFI Opus/UDP audio transport) | `voice` / `lavalink` (opt-in) |

---

## Type & Method Cheatsheet

### IDs & Core Types

| Serenity / Poise | Twilight | `cordex` |
|---|---|---|
| `GuildId`, `ChannelId`, `UserId`, `RoleId`, `MessageId` | `Id<GuildMarker>`, `Id<ChannelMarker>`, `Id<UserMarker>`, … | `Id<GuildMarker>`, `Id<ChannelMarker>`, `Id<UserMarker>`, … |
| `serenity::all::*` / `serenity::prelude::*` | individual crate imports | `use cordex::prelude::*;` |
| `Client` (`serenity::Client`) | `Shard` + `twilight_http::Client` | `Bot` (high-level) or `ShardManager` + `cordex::http::Client` (low-level) |
| `Context` / `poise::Context<'_, Data, Err>` | `Arc<Client>` + `Arc<InMemoryCache>` | `CommandContext`, `ComponentContext`, `ModalContext`, `MessageContext`, `Context` |
| `TypeMap` + `impl TypeMapKey` | custom `Arc<State>` struct | `Bot::data(value)` + `ctx.data::<T>()` (lock-free, no `.await`) |

### Intents & Permissions

| Serenity | Twilight | `cordex` |
|---|---|---|
| `GatewayIntents::GUILD_MESSAGES \| GatewayIntents::MESSAGE_CONTENT` | `Intents::GUILD_MESSAGES \| Intents::MESSAGE_CONTENT` | `Bot::new(token).message_content()` or `.intents(Intents::...)` |
| `Permissions::KICK_MEMBERS` | `Permissions::KICK_MEMBERS` | `Permissions::KICK_MEMBERS` |
| `guild.user_permissions_in(channel, member)` | `twilight_util::permission_calculator::PermissionCalculator` | `ctx.user_can(perm)` / `ctx.bot_can(perm)` (interactions) or `cache.permissions(guild, user, channel)` |

### Interactions, Messages & Components

| Serenity / Poise | Twilight | `cordex` |
|---|---|---|
| `ctx.say("Hi").await` | `http.interaction(app).create_response(...)` | `ctx.reply("Hi").await` |
| `ctx.send(CreateReply::default().ephemeral(true).content("Hi"))` | `InteractionResponseData { flags: Some(EPHEMERAL), .. }` | `ctx.reply_ephemeral("Hi").await` |
| `ctx.defer().await` / `ctx.defer_ephemeral().await` | `InteractionResponseType::DeferredChannelMessageWithSource` | Automatic after 2.5 s; manual `ctx.defer().await` / `ctx.defer_ephemeral().await` |
| `CreateEmbed::new().title("Hi").color(0x5865F2)` | `EmbedBuilder::new().title("Hi").color(0x5865F2).build()` | `Embed::new().title("Hi")?.color(0x0058_65F2)` |
| `CreateButton::new("yes").label("Yes").style(Success)` | `Button { custom_id: Some("yes"), style: Success, .. }` | `Button::success("yes", "Yes")` inside `ActionRow::new([...])` |
| `CreateModal::new("fb", "Feedback")` | `InteractionResponseData { custom_id, title, components }` | `Modal::new("fb", "Feedback").text_input("k", "Label")` + `ctx.modal(modal).await` |
| `channel_id.say(&ctx.http, "Hi").await` | `http.create_message(channel_id).content("Hi").await` | `ctx.send(channel_id, "Hi").await` |
| `user.direct_message(&ctx.http, ...)` | `create_private_channel` + `create_message` | `ctx.dm(user_id, "Hi").await` |

---

## Side-by-Side Migration Recipes

### Recipe 1: Minimal Bot & Graceful Shutdown

**Before (Serenity):**

```rust,ignore
use serenity::async_trait;
use serenity::model::channel::Message;
use serenity::prelude::*;

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, ctx: Context, msg: Message) {
        if msg.content == "!ping" {
            let _ = msg.channel_id.say(&ctx.http, "Pong!").await;
        }
    }
}

#[tokio::main]
async fn main() {
    let token = std::env::var("DISCORD_TOKEN").unwrap();
    let intents = GatewayIntents::GUILD_MESSAGES | GatewayIntents::MESSAGE_CONTENT;
    let mut client = Client::builder(&token, intents).event_handler(Handler).await.unwrap();
    client.start().await.unwrap();
}
```

**After (`cordex`):**

```rust,no_run
use cordex::prelude::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    Bot::new(std::env::var("DISCORD_TOKEN")?)
        .message_content()
        .on_message(|ctx: MessageContext| async move {
            if ctx.content() == "!ping" {
                ctx.reply("Pong!").await?;
            }
            Ok(())
        })
        .run_until_ctrl_c()
        .await?;
    Ok(())
}
```

---

### Recipe 2: Slash Command with Options & Subcommands

**Before (Poise):**

```rust,ignore
#[poise::command(slash_command, subcommands("add", "remove"))]
async fn role(_ctx: Context<'_>) -> Result<(), Error> { Ok(()) }

#[poise::command(slash_command)]
async fn add(
    ctx: Context<'_>,
    #[description = "Target user"] user: serenity::User,
    #[description = "Optional note"] note: Option<String>,
) -> Result<(), Error> {
    ctx.say(format!("Updated {} ({})", user.mention(), note.as_deref().unwrap_or("-"))).await?;
    Ok(())
}
```

**After (`cordex`):**

```rust,no_run
use cordex::prelude::*;

fn build_bot(token: String) -> Bot {
    let role_cmd = Command::slash("role", "Manage member roles")
        .option(
            CommandOption::subcommand("add", "Add a role")
                .option(CommandOption::user("user", "Target user").required())
                .option(CommandOption::string("note", "Optional note")),
        )
        .option(
            CommandOption::subcommand("remove", "Remove a role")
                .option(CommandOption::user("user", "Target user").required()),
        );

    Bot::new(token).command(role_cmd, |ctx: CommandContext| async move {
        let user = ctx.user_option("user")?;
        match ctx.subcommand().as_slice() {
            ["add"] => {
                let note = ctx.string_option_opt("note")?.unwrap_or("-");
                ctx.reply(format!("Added role for {} ({note})", user.mention()))
                    .await
            }
            ["remove"] => {
                ctx.reply(format!("Removed role from {}", user.mention()))
                    .await
            }
            _ => Ok(()),
        }
    })
}
```

---

### Recipe 3: Autocomplete

**Before (Poise):**

```rust,ignore
async fn autocomplete_color<'a>(_ctx: Context<'_>, partial: &'a str) -> impl Iterator<Item = String> + 'a {
    ["red", "green", "blue"].into_iter().filter(move |c| c.starts_with(partial)).map(str::to_owned)
}
```

**After (`cordex`):**

```rust,no_run
use cordex::prelude::*;

const COLORS: [&str; 3] = ["red", "green", "blue"];

fn add_color_command(bot: Bot) -> Bot {
    bot.command(
        Command::slash("color", "Pick a color").option(
            CommandOption::string("query", "Start typing")
                .required()
                .autocomplete(),
        ),
        |ctx: CommandContext| async move {
            ctx.reply(format!("You picked {}", ctx.string_option("query")?))
                .await
        },
    )
    .autocomplete("color", |ctx: AutocompleteContext| async move {
        let typed = ctx.focused_value().to_lowercase();
        let hits = COLORS
            .iter()
            .filter(|c| c.starts_with(&typed))
            .map(|c| Choice::string(*c, *c));
        ctx.suggest(hits).await
    })
}
```

---

### Recipe 4: Buttons & Inline Waiting

In Serenity you typically call `msg.await_component_interaction(&ctx)`; in Twilight you wire `twilight_standby::Standby`. In `cordex`, `Standby` is built directly into `CommandContext`, and `on_button("prefix", ...)` also splits `custom_id` on `:` (`vote:yes` -> `ctx.custom_id_arg() == "yes"`):

```rust,no_run
use std::time::Duration;
use cordex::prelude::*;

async fn confirm_command(ctx: CommandContext) -> cordex::Result<()> {
    let row = ActionRow::new([
        Button::success("confirm:yes", "Yes"),
        Button::danger("confirm:no", "No"),
    ]);
    ctx.defer().await?;
    let msg = ctx
        .edit(Reply::new("Are you sure?").components(vec![row.into()]))
        .await?;

    match ctx.wait_for_button(msg.id, Duration::from_secs(30)).await {
        Some(click) => click.update(format!("Picked `{}`", click.custom_id_arg())).await,
        None => ctx.edit("Timed out.").await.map(|_| ()),
    }
}
```

For multi-page responses, replace custom button loops with `ctx.paginate(pages, Duration::from_secs(60)).await`.

---

### Recipe 5: Modals

**After (`cordex`):**

```rust,no_run
use cordex::prelude::*;

fn add_feedback(bot: Bot) -> Bot {
    bot.command(
        Command::slash("feedback", "Send feedback"),
        |ctx: CommandContext| async move {
            let modal = Modal::new("feedback", "Feedback")
                .text_input("subject", "Subject")
                .paragraph("body", "Details");
            ctx.modal(modal).await
        },
    )
    .on_modal("feedback", |ctx: ModalContext| async move {
        let subject = ctx.value("subject").unwrap_or_default();
        let body = ctx.value("body").unwrap_or_default();
        ctx.reply_ephemeral(format!("Thanks! **{subject}**: {body}"))
            .await
    })
}
```

---

### Recipe 6: Shared State (`TypeMap` -> `Bot::data`)

**Before (Serenity):**

```rust,ignore
struct DbKey;
impl TypeMapKey for DbKey { type Value = Arc<DbPool>; }

// Inside handler:
let pool = {
    let data = ctx.data.read().await;
    data.get::<DbKey>().unwrap().clone()
};
```

**After (`cordex`):**

```rust,no_run
use cordex::prelude::*;

struct AppState {
    prefix: String,
}

fn build_with_state(token: String) -> Bot {
    Bot::new(token)
        .data(AppState {
            prefix: "!".to_owned(),
        })
        .command(
            Command::slash("prefix", "Show configured prefix"),
            |ctx: CommandContext| async move {
                let prefix = ctx
                    .data::<AppState>()
                    .map_or("!", |state| state.prefix.as_str());
                ctx.reply(format!("Current prefix: `{prefix}`")).await
            },
        )
}
```

No `TypeMapKey`, no `RwLock`, and no `.await` to read shared state.

---

### Recipe 7: Permissions, Moderation & Audit Log Reason

`CommandContext` exposes `ctx.user_can(...)` and `ctx.bot_can(...)` directly from Discord's interaction payload (no cache or REST lookup needed), while `cache.can_moderate(...)` checks role hierarchy when `cache` is enabled:

```rust,no_run
use cordex::prelude::*;

async fn kick_member(ctx: CommandContext) -> cordex::Result<()> {
    let Some(guild_id) = ctx.guild_id() else {
        return ctx.reply_ephemeral("Use this command in a server.").await;
    };
    if !ctx.user_can(Permissions::KICK_MEMBERS) || !ctx.bot_can(Permissions::KICK_MEMBERS) {
        return ctx.reply_ephemeral("Missing `KICK_MEMBERS` permission.").await;
    }

    let target = ctx.user_option("user")?;
    let reason = ctx.string_option_opt("reason")?.unwrap_or("No reason");

    ctx.http()
        .remove_guild_member(guild_id, target.id)
        .reason(reason)?
        .await?;

    ctx.reply_ephemeral(format!("Kicked **{}** ({reason}).", target.username))
        .await
}
```

---

### Recipe 8: Opt-in Cache & REST Fallback

**Before (Twilight):**

```rust,ignore
let cache = InMemoryCache::builder()
    .resource_types(ResourceType::GUILD | ResourceType::MEMBER | ResourceType::ROLE)
    .build();
```

**After (`cordex`, with `features = ["cache"]`):**

```rust,no_run
use cordex::cache::{CacheConfig, CacheParts, ResourceType};
use cordex::prelude::*;

fn build_cached_bot(token: String) -> Bot {
    let cache = CacheConfig::new()
        .resources(ResourceType::GUILDS | ResourceType::MEMBERS | ResourceType::ROLES)
        .parts(CacheParts::MEMBER_ROLES);

    Bot::new(token)
        .intents(Intents::GUILD_MEMBERS)
        .cache(cache)
        .command(
            Command::slash("server", "Show server name"),
            |ctx: CommandContext| async move {
                let Some(guild_id) = ctx.guild_id() else {
                    return ctx.reply_ephemeral("Server only.").await;
                };
                // Reads from cache first, falls back to REST on a miss.
                let guild = ctx.get_guild(guild_id).await?;
                ctx.reply(format!("Server: **{}**", guild.name)).await
            },
        )
}
```

---

### Recipe 9: Error Handling & Command Guards (`before_command`, `on_error`)

**After (`cordex`):**

```rust,no_run
use std::time::Duration;
use cordex::prelude::*;

fn build_guarded_bot(token: String) -> Bot {
    Bot::new(token)
        .data(Cooldowns::<(u64, String)>::new())
        .before_command(|ctx: CommandContext| async move {
            if let (Some(cooldowns), Some(user)) =
                (ctx.data::<Cooldowns<(u64, String)>>(), ctx.user())
                && let Some(wait) =
                    cooldowns.check((user.id.get(), ctx.name().to_owned()), Duration::from_secs(5))
            {
                ctx.reply_ephemeral(format!("Slow down! Try again in {}s.", wait.as_secs()))
                    .await?;
                return Ok(false);
            }
            Ok(true)
        })
        .on_error(|_ctx, err| async move {
            eprintln!("command failed: {err}");
        })
}
```

---

### Recipe 10: Low-Level Sharding & Event Loop (Twilight)

If you prefer Twilight's explicit shard loop without `Bot`, use `ShardManager` directly — it handles automatic or ranged sharding, identify queues, reconnects, and pre-parse event filtering:

```rust,no_run
use cordex::gateway::{ManagerEvent, ShardManager};
use cordex::prelude::*;

async fn run_shards(token: String) -> Result<(), Box<dyn std::error::Error>> {
    let http = Client::new(&token)?;
    let mut manager = ShardManager::builder(&token, Intents::GUILDS | Intents::GUILD_MESSAGES)
        .http(http)
        .event_types(EventTypeFlags::READY | EventTypeFlags::MESSAGE_CREATE)
        .build()
        .await?;

    let mut events = manager.events();
    manager.start().await?;

    while let Some(item) = events.recv().await {
        if let ManagerEvent::Dispatch { shard, event } = item
            && let Event::MessageCreate(msg) = event
        {
            println!("[shard {}] {}", shard.number(), msg.content);
        }
    }
    Ok(())
}
```

---

## Gotchas Checklist

1. **Read-only lists use `Box<[T]>` and `Box<str>`:** Models in `cordex-model` use boxed slices instead of `Vec<T>`/`String` to save 8 bytes per field. Use `.as_ref()` or `&*field` when passing to APIs expecting `&str` or `&[T]`.
2. **Cache is off by default:** Nothing is cached until you enable the `cache` feature and pass `.cache(CacheConfig::new().resources(...))`. Unneeded gateway events are skipped before JSON deserialization.
3. **Privileged intents are never enabled implicitly:** `Bot` derives standard intents from your handlers, but `MESSAGE_CONTENT`, `GUILD_MEMBERS`, and `GUILD_PRESENCES` always require explicit opt-in (`.message_content()` or `.intents(...)`) plus enabling them in the Discord Developer Portal.
4. **Auto-defer after 2.5 seconds:** Slow command handlers do not fail Discord's 3-second timeout. Call `ctx.defer_ephemeral().await?` manually only when you want an immediate ephemeral "thinking…" state.
5. **Button `:` prefix routing:** Registering `.on_button("vote", ...)` matches both `"vote"` and `"vote:<arg>"`, exposing `<arg>` via `ctx.custom_id_arg()`.
6. **Voice transport:** `cordex-voice` and `cordex-lavalink` are pure Rust (voice signaling + Lavalink v4). If your Serenity/Twilight bot uses Songbird for in-process Opus/UDP audio streaming, keep Songbird or switch playback to Lavalink v4.
