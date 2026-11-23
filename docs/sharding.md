# Sharding

`cordex-gateway` supports single shards, automatic sharding, manual shard
ranges for multi-process clusters, and custom runtime placement
(thread-per-core).

## Automatic sharding

`ShardManager::builder(token, intents).automatic().build().await?` calls
`GET /gateway/bot` to fetch the recommended shard count and
`session_start_limit`, creates all shards with a shared `LocalIdentifyQueue`,
and exposes a fan-in event stream (`manager.events()`).

In the simple layer (`cordex::bot::Bot`), automatic sharding is enabled by
default and includes **live resharding** (default interval: 8 hours,
configurable via `Bot::reshard_interval`): when Discord recommends more shards
and `session_start_limit.remaining` is sufficient, `Bot` starts the new shard
set, waits until all new shards are `READY`, switches over, and closes the old
set.

## Manual sharding and multi-process clusters

To split shards across processes, pass a shard range and total count:

```rust,ignore
let manager = ShardManager::builder(&token, intents)
    .shards(0..8, 16) // this process runs shards 0..8 out of 16
    .build()
    .await?;
```

See `crates/facade/examples/manual_sharding.rs` for a complete runnable
example.

## Identify queue

Discord limits `IDENTIFY` to one per bucket (`shard_id % max_concurrency`)
every 5 seconds.

- **`LocalIdentifyQueue`** (default): enforces 5.25 s spacing per bucket and
  tracks `session_start_limit.remaining`, waiting for `reset_after` when the
  budget is exhausted.
- **`IdentifyQueue` trait:** Implement this trait backed by Redis/Dragonfly or
  a coordinator service for multi-process deployments. `RESUME` never goes
  through the identify queue.

## Thread-per-core placement (`Spawner`)

`cordex` never spawns tasks on a runtime you did not choose:

- **`CurrentRuntime`** (default): spawns on `tokio::runtime::Handle::try_current()`.
- **`RuntimeHandles`**: distributes shard `n` onto `handles[n % handles.len()]`
  so each shard stays pinned to its own `current_thread` runtime without work
  stealing.

See `crates/facade/examples/thread_per_core.rs` for a runnable example.
