# Production Resilience, Scheduler & Observability

`cordex` `0.2.0` includes zero-dependency building blocks for long-running production bots.

## Zero-Downtime `RESUME` Persistence (`SessionStore`)

Persist active shard sessions across restarts with `FileSessionStore` (atomic temp-file + rename per shard):

```rust
use cordex::gateway::{FileSessionStore, SessionStore};

let store = FileSessionStore::new("./state/sessions");
```

On graceful shutdown, shards close with a non-`1000` close code (`4000`) so Discord keeps the session resumable.

## Priority-Aware Global Rate Limiter

Critical interaction responses (3-second window) automatically get `RequestPriority::Critical` ahead of `Normal` and `Background` traffic, with fair-scheduling aging so background jobs never starve:

```rust
use cordex::http::RequestPriority;

ctx.http()
    .create_message(log_channel)
    .content("Audit log entry")?
    .priority(RequestPriority::Background)
    .send()
    .await?;
```

## Bounded Cache Eviction & Write-Through

Cap memory usage in `CacheConfig` and optionally write REST fallbacks back into the cache:

```rust
let cache = CacheConfig::new()
    .guilds()
    .channels()
    .members()
    .max_messages_per_channel(50)
    .max_users(50_000)
    .member_ttl(Duration::from_secs(3600))
    .write_through(true);
```

## Health Snapshot (`/healthz` & `/readyz`)

`ctx.health_snapshot()` returns a serializable `HealthSnapshot` with shard status/latency, cache sizes, open `Standby` waiters, and HTTP request/rate-limit counters:

```rust
let snap = ctx.health_snapshot();
let status = if snap.is_ready() { 200 } else { 503 };
let json = snap.to_json();
```

## Lifecycle-Bound Scheduler & `around_command` Middleware

```rust
use std::time::Duration;
use cordex::prelude::*;

let bot = Bot::from_env()?
    .sync_dev_guild_from_env()
    .around_command(|ctx: CommandContext, next: Next| async move {
        let start = std::time::Instant::now();
        let res = next.run(ctx).await;
        tracing::info!(elapsed_ms = start.elapsed().as_millis(), ok = res.is_ok(), "command");
        res
    })
    .every(Duration::from_secs(60), |ctx: Context| async move {
        tracing::debug!(ready = ctx.health_snapshot().is_ready(), "heartbeat");
        Ok(())
    });
```
