# Sharding & Resharding

## Automatic sharding & live resharding

`Bot` shards automatically based on `GET /gateway/bot`. Every 8 hours (configurable via `Bot::reshard_interval`), it checks whether Discord recommends more shards; if so and `session_start_limit.remaining` allows it, `Bot` starts the new shard set, waits for all shards to become `READY`, switches over, and closes the old set.

## Manual shard ranges (multi-process)

Use `ShardManager::builder` with `.shards(from..to, total)` to run a subset of shards per process:

```rust,no_run
use cordex::gateway::ShardManager;
use cordex::prelude::*;

async fn run(token: String) -> cordex::Result<()> {
    let mut manager = ShardManager::builder(&token, Intents::GUILDS)
        .shards(0..4, 8)
        .build()
        .await?;
    manager.start().await?;
    Ok(())
}
```

See `crates/facade/examples/manual_sharding.rs`.

## Thread-per-core

Pin shard `n` to dedicated `current_thread` Tokio runtimes via `RuntimeHandles` and `.spawn_with(Arc::new(spawner))`. See `crates/facade/examples/thread_per_core.rs`.
