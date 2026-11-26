# Optional Cache

Caching in `cordex` is **completely opt-in**. Enable the `cache` feature on `cordex`:

```toml
cordex = { version = "0.2.0", features = ["cache"] }
```

By default (`CacheConfig::default()`), nothing is stored. Every resource (`ResourceType`), every optional field group (`CacheParts`), and every limit is switched on explicitly and can be changed at runtime via `InMemoryCache::set_config` (switching a resource off drops its data immediately).

## `CachePolicy`: `Off`, `Lazy`, `Eager`

Each of the 14 `ResourceType` flags (`GUILDS`, `CHANNELS`, `THREADS`, `USERS`, `MEMBERS`, `ROLES`, `EMOJIS`, `STICKERS`, `SOUNDBOARD_SOUNDS`, `VOICE_STATES`, `PRESENCES`, `STAGE_INSTANCES`, `SCHEDULED_EVENTS`, `MESSAGES`) has one of three policies:

- **`Off`** *(default)*: Not stored.
- **`Lazy`**: Skips bulk dumps (`GUILD_CREATE`, `GUILD_MEMBERS_CHUNK`, `THREAD_LIST_SYNC`, `SOUNDBOARD_SOUNDS`) and caches entries only when seen in live events (messages, interactions, member/voice/guild/channel/role updates). If every enabled resource is `Lazy`, `wanted_events()` omits `GUILD_CREATE` so the gateway skips parsing it.
- **`Eager`**: Caches everything, including bulk dumps on `GUILD_CREATE`.

```rust,no_run
use cordex::prelude::*;

let config = CacheConfig::new()
    // Eagerly keep guild structure for local permission checks
    .resources(ResourceType::GUILDS | ResourceType::CHANNELS | ResourceType::ROLES)
    // Lazily cache members and users as they interact with the bot
    .lazy(ResourceType::MEMBERS | ResourceType::USERS)
    .parts(CacheParts::MEMBER_ROLES | CacheParts::CHANNEL_OVERWRITES)
    .max_members_per_guild(Some(5_000));
```

Validate your config at startup with `config.validate()` to catch misconfigurations (such as `MEMBERS` without `USERS`, or `MESSAGES` with `message_cache_size == 0`).

## Automatic Event Maintenance & Cascades

`InMemoryCache::update(&event)` keeps all 14 resources consistent across gateway events:

- **Guilds:** Tracks `member_count` from `GUILD_CREATE`, increments/decrements on `GUILD_MEMBER_ADD` / `GUILD_MEMBER_REMOVE` (read via `cache.guild_member_count(guild_id)`), and preserves `approximate_member_count` / `approximate_presence_count` on `GUILD_UPDATE`.
- **Members & Users:** Users are stored once globally and reference-counted in `O(1)` across guilds. `GUILD_MEMBER_REMOVE` also drops stale `voice_states` and `presences` for that member. `USER_UPDATE` updates both `current_user` and `users`.
- **Roles:** `GUILD_ROLE_DELETE` cascades to remove the deleted role ID from cached `member.roles` and role `permission_overwrites` on guild channels.
- **Channels & Threads:** `CHANNEL_DELETE` cascades to drop child threads (and their cached messages), voice states, and stage instances. `CHANNEL_PINS_UPDATE`, `MESSAGE_CREATE`, and `THREAD_MEMBERS_UPDATE` keep `last_pin_timestamp`, `last_message_id`, and thread `member_count` up to date.
- **Messages & Reactions:** Ring buffer per channel (`message_cache_size`). `MESSAGE_REACTION_ADD`, `MESSAGE_REACTION_REMOVE`, `MESSAGE_REACTION_REMOVE_EMOJI`, and `MESSAGE_REACTION_REMOVE_ALL` maintain `Message.reactions` (normal & burst counts, `me`/`me_burst`, `burst_colors`).
- **Scheduled Events:** `GUILD_SCHEDULED_EVENT_USER_ADD` / `REMOVE` maintain `user_count`.

Feed `config.wanted_events()` into the gateway's `EventTypeFlags` (done automatically by `Bot`) so events the cache doesn't need are never parsed.

## Permissions Without REST Calls

With `GUILDS`, `CHANNELS`, `ROLES`, `MEMBERS`, and `CacheParts::MEMBER_ROLES | CacheParts::CHANNEL_OVERWRITES` enabled, `cache.permissions(guild, user, channel)` computes effective channel permissions locally (including thread parent overwrites and timeout exemptions) and returns a descriptive `PermissionCacheError` if any required resource or part is missing.

## Backfill

The cache never makes network calls on its own. To load message history or guild members from before startup, use `cordex::backfill::messages`, `cordex::backfill::guild_messages`, or `cordex::backfill::guild_members` (or feed fetched entries directly with `cache.insert_messages` / `cache.insert_members`).

## Previous-State Snapshots (Diffs)

`cache.update_with_previous(&event)` returns a `PreviousState` (`User`, `Guild`, `Channel`, `Thread`, `Role`, `Member`, `VoiceState`, `Message`, or `None`) containing the entry that was replaced or removed. In `Bot::on_event`, the snapshot is exposed directly via `ctx.previous()` — useful for audit-log diffs, autorole `pending` transitions, or voice channel switches without maintaining a second shadow cache.

## Role Hierarchy & Memory Maintenance

- `cache.can_moderate(guild_id, moderator_id, target_id)` checks Discord's role hierarchy rules (owner protection, `ADMINISTRATOR` protection, highest role position + ID tie-breaker).
- `cache.shrink_to_fit()` releases over-allocated map/deque capacity after the initial `GUILD_CREATE` burst.
- `cache.prune_members(max_per_guild)` trims cached members per guild while preferring to keep the bot itself and active voice members, releasing orphaned users.
