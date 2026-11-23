# Optional Cache

Caching in `cordex` is **completely opt-in**. Enable the `cache` feature on `cordex`:

```toml
cordex = { version = "0.1.3", features = ["cache"] }
```

By default (`CacheConfig::default()`), nothing is stored. Every resource (`ResourceType`), every optional field group (`CacheParts`), and every limit is switched on explicitly and can be changed at runtime via `InMemoryCache::set_config`.

## `CachePolicy`: `Off`, `Lazy`, `Eager`

Each `ResourceType` has one of three policies:

- **`Off`** *(default)*: Not stored.
- **`Lazy`**: Skips bulk dumps (`GUILD_CREATE`, `GUILD_MEMBERS_CHUNK`, `THREAD_LIST_SYNC`, `SOUNDBOARD_SOUNDS`) and caches entries only when seen in live events (messages, interactions, member/voice/guild/channel/role updates). If every enabled resource is `Lazy`, `wanted_events()` omits `GUILD_CREATE` so the gateway skips parsing it.
- **`Eager`**: Caches everything, including bulk dumps on `GUILD_CREATE`.

```rust,no_run
use cordex::prelude::*;

let config = CacheConfig::new()
    // Eagerly keep guild structure for permission checks
    .resources(ResourceType::GUILDS | ResourceType::CHANNELS | ResourceType::ROLES)
    // Lazily cache members and users as they interact with the bot
    .lazy(ResourceType::MEMBERS | ResourceType::USERS)
    .parts(CacheParts::MEMBER_ROLES | CacheParts::CHANNEL_OVERWRITES);
```

## Permissions without REST calls

With `GUILDS`, `CHANNELS`, `ROLES`, `MEMBERS` and `CacheParts::MEMBER_ROLES | CacheParts::CHANNEL_OVERWRITES` enabled, `cache.permissions(guild, user, channel)` computes effective channel permissions locally (including thread parent overwrites and timeout exemptions).

## Backfill

The cache has no network access of its own. To load message history or guild members from before startup, call `cordex::backfill::messages`, `cordex::backfill::guild_messages`, or `cordex::backfill::guild_members`.
