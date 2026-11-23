# cordex-gateway

[![crates.io][crates badge]][crates link] [![docs.rs][docs badge]][docs link] [![discord][discord badge]][discord link] ![rust 1.95+][rust badge]

Discord gateway for [cordex]: shard connections, automatic and manual
sharding, and waiting for events.

- **Connection**: zlib-stream compression, heartbeat with jitter and zombie
  detection, resume and reconnect, every close code handled (fatal ones with
  a hint how to fix them), outbound rate limit.
- **Sharding**: `ShardManager` with automatic shard count or a fixed range
  (multi-process), identify queue that respects `max_concurrency` and the
  session start limit, thread-per-core placement via `Spawner`.
- **Low memory, low latency**: an event filter (`EventTypeFlags`) skips
  unwanted events before parsing, every channel is bounded, a slow consumer
  triggers a resumable disconnect instead of growing memory.
- **Robust**: a panic ends one shard run, not the process; the shard
  reconnects.
- **`Standby`**: wait for the next event that matches a filter, with a
  timeout, or get a stream of them.

Most bots use the facade crate [`cordex`][facade] and its `Bot`, which runs
the shard manager for you.

## Example

```rust,no_run
use cordex_gateway::{ManagerEvent, ShardManager};
use cordex_model::gateway::{Event, Intents};

async fn example(token: &str) -> Result<(), Box<dyn std::error::Error>> {
    let intents = Intents::GUILDS | Intents::GUILD_MESSAGES;
    let mut manager = ShardManager::builder(token, intents).automatic().build().await?;
    let mut events = manager.events();
    manager.start().await?;

    while let Some(item) = events.recv().await {
        if let ManagerEvent::Dispatch { shard, event } = item {
            if let Event::MessageCreate(message) = event {
                println!("[shard {}] {}", shard.number(), message.content);
            }
        }
    }
    Ok(())
}
```

## License

Licensed under either of Apache License, Version 2.0 or MIT license at your
option. Questions or help: [cordex Discord server][discord link].

[cordex]: https://github.com/lejylabs/cordex
[facade]: https://crates.io/crates/cordex
[crates badge]: https://img.shields.io/crates/v/cordex-gateway.svg?style=for-the-badge&logo=rust
[crates link]: https://crates.io/crates/cordex-gateway
[docs badge]: https://img.shields.io/docsrs/cordex-gateway?style=for-the-badge&logo=docsdotrs
[docs link]: https://docs.rs/cordex-gateway
[discord badge]: https://img.shields.io/badge/discord-join-5865F2.svg?style=for-the-badge&logo=discord&logoColor=white
[discord link]: https://discord.gg/Xj9nRKyKNV
[rust badge]: https://img.shields.io/badge/rust-1.95+-93450a.svg?style=for-the-badge&logo=rust
