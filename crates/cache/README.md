# cordex-cache

[![crates.io][crates badge]][crates link] [![docs.rs][docs badge]][docs link] [![discord][discord badge]][discord link] ![rust 1.95+][rust badge]

Optional in-memory cache for [cordex], fed with gateway events.

**Everything is off by default.** `CacheConfig::default()` caches nothing;
every resource (`ResourceType`), every optional part of a resource
(`CacheParts`) and every limit is its own switch, also at runtime
(`InMemoryCache::set_config`, switching off drops the data at once). You
only pay memory and CPU for what you switch on.

- **14 resources**: guilds, channels, threads, users, members, roles, emojis,
  stickers, soundboard sounds, voice states, presences, stage instances,
  scheduled events, messages.
- **11 parts** such as member roles, message content, channel overwrites.
- **Limits**: messages per channel (ring buffer), members and presences per
  guild.
- **`wanted_events()`** feeds the gateway event filter, so events the cache
  does not need are never parsed.
- **`permissions(guild, user, channel)`**: computed permissions from cached
  roles and overwrites (threads use their parent channel), with an error
  naming what is missing.
- **No network access**: the cache never fills itself; load old messages and
  members with `cordex::backfill` when you want them.
- Thread-safe; lookups return `Arc` clones and never hold a lock across an
  `.await`.

## Example

```rust
use cordex_cache::{CacheConfig, CacheParts, InMemoryCache, ResourceType};

let cache = InMemoryCache::new(
    CacheConfig::new()
        .resources(ResourceType::GUILDS | ResourceType::MEMBERS | ResourceType::USERS)
        .parts(CacheParts::MEMBER_ROLES),
);
// For each gateway event: cache.update(&event);
assert_eq!(cache.stats().guilds, 0);
```

With the facade's `Bot`: `Bot::new(token).cache(CacheConfig::new()...)` and
`ctx.cache()` in any handler.

## License

Licensed under either of Apache License, Version 2.0 or MIT license at your
option. Questions or help: [cordex Discord server][discord link].

[cordex]: https://github.com/lejylabs/cordex
[crates badge]: https://img.shields.io/crates/v/cordex-cache.svg?style=for-the-badge&logo=rust
[crates link]: https://crates.io/crates/cordex-cache
[docs badge]: https://img.shields.io/docsrs/cordex-cache?style=for-the-badge&logo=docsdotrs
[docs link]: https://docs.rs/cordex-cache
[discord badge]: https://img.shields.io/badge/discord-join-5865F2.svg?style=for-the-badge&logo=discord&logoColor=white
[discord link]: https://discord.gg/Xj9nRKyKNV
[rust badge]: https://img.shields.io/badge/rust-1.95+-93450a.svg?style=for-the-badge&logo=rust
