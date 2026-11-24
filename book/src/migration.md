# Migrating from Serenity & Twilight

`cordex` combines the **modular, zero-overhead architecture** of Twilight with an **ergonomic batteries-included layer (`Bot`)** similar to Serenity/Poise.

## Architectural Comparison

| Aspect | Serenity (+ Poise) | Twilight | `cordex` |
|---|---|---|---|
| **Crate structure** | Monolithic crate | Modular crates | Modular crates (`cordex-*`) + unified `cordex` facade |
| **Simple bot layer** | `EventHandler` / Poise macros | Manual event loop | Built-in `cordex::bot::Bot` (no proc-macro magic) |
| **Caching** | On by default (when `cache` feature is enabled) | Separate `twilight-cache-inmemory` (`ResourceType` flags) | Opt-in `cordex-cache` with `Off` / `Lazy` / `Eager` per resource + `CacheParts` field stripping |
| **Gateway compression** | `zlib-stream` | `zlib-stream` / `zstd-stream` | Pure-Rust `zlib-stream` default + optional `zstd-stream` |
| **HTTP/2** | via `reqwest` | HTTP/1.1 or HTTP/2 | HTTP/2 via ALPN by default |
| **Voice** | via `songbird` | via `songbird` / `twilight-lavalink` | Opt-in pure-Rust `cordex-voice` & `cordex-lavalink` (no C/FFI audio transport) |
| **Unsafe code** | Some in deps | Minimal | `#![forbid(unsafe_code)]` across the workspace |

## Coming from Serenity / Poise

### Command Handler

In **Poise**, commands use proc-macros (`#[poise::command(slash_command)]`). In **`cordex`**, commands are plain Rust builders and async closures — no macro expansion errors, full IDE autocomplete:

```rust,no_run
use cordex::prelude::*;

Bot::new(token)
    .command(
        Command::slash("greet", "Greet a user")
            .required_user("user", "User to greet"),
        |ctx: CommandContext| async move {
            let user = ctx.user_option("user")?;
            ctx.reply(format!("Hello {}!", user.mention())).await
        },
    )
    .run_until_ctrl_c()
    .await?;
```

### Shared State (`TypeMap` → `Bot::data`)

Instead of Serenity's `ctx.data.read().await` with `TypeMapKey`, attach any `Send + Sync + 'static` type directly:

```rust,no_run
// Registration
Bot::new(token).data(db_pool)

// Inside any handler (no .await or lock needed)
let pool = ctx.data::<DbPool>().expect("DbPool registered");
```

### Auto-Defer

`Bot` automatically defers interactions after 2.5 seconds — you only need manual `ctx.defer()` / `ctx.defer_ephemeral()` if you want an immediate "thinking…" state or an ephemeral deferred response.

## Coming from Twilight

If you prefer Twilight's explicit low-level control, every `cordex-*` crate can be used directly without `Bot`:

- `twilight-model` → `cordex-model` (uses `Box<[T]>` for read-only lists to minimize RAM)
- `twilight-http` → `cordex-http`
- `twilight-gateway` → `cordex-gateway` (with `Shard`, `ShardManager`, `EventTypeFlags`, `Standby`)
- `twilight-cache-inmemory` → `cordex-cache` (`cache.update(&event)` + `CachePolicy::Lazy` + `CacheParts` + `wanted_events()`)
- `twilight-standby` → built into `cordex-gateway::Standby`
