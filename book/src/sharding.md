# Sharding & Resharding

## Automatic sharding & live resharding

`Bot` shards automatically based on `GET /gateway/bot`. Every 8 hours (configurable via `Bot::reshard_interval`), it checks whether Discord recommends more shards; if so and `session_start_limit.remaining` allows it, `Bot` starts the new shard set, waits for all shards to become `READY`, switches over, and closes the old set.

You can also tune automatic sharding preferences with `AutoShardingConfig` (or the `Bot::guilds_per_shard(1_000)` shortcut) — for example to target fewer guilds per shard than Discord's 2,500 cap, enforce `min_shards` / `max_shards`, or align to a multiple (such as `max_concurrency` or your cluster count):

```rust,no_run
use cordex::gateway::AutoShardingConfig;
use cordex::prelude::*;

# async fn run() -> cordex::Result<()> {
Bot::from_env()?
    .auto_sharding(
        AutoShardingConfig::new()
            .guilds_per_shard(1_000)
            .min_shards(4)
            .round_up_to_multiple(4),
    )
    .run_until_ctrl_c()
    .await
# }
```

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

## Shard Ranges, Clusters & Arbitrary IDs in `Bot`

When using the simple `Bot` layer across multiple processes, you can configure:
- `Bot::shard_range(0..4, 8)` — contiguous shard slice
- `Bot::shard_ids([0, 2, 4, 6], 8)` — arbitrary / strided shard IDs
- `Bot::shard_cluster(cluster_id, cluster_count, total)` — automatic fair cluster slicing (using `cluster_shard_range`, ideal for Kubernetes `StatefulSet`s)
- `Bot::session_store(Arc::new(store))` — automatic zero-downtime `RESUME` via `FileSessionStore` or `RedisSessionStore`
- `Bot::identify_queue(queue)`, `Bot::spawn_with(spawner)`, and `Bot::presence_for_shard(|id, total| ...)`

```rust,no_run
use cordex::prelude::*;

# async fn run() -> cordex::Result<()> {
Bot::new("token")
    .shard_cluster(0, 3, 10) // shards 0..4 of 10
    .run_until_ctrl_c()
    .await
# }
```
