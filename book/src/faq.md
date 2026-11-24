# FAQ & Troubleshooting

## My bot doesn't receive `message.content` (it's always empty)

`MESSAGE_CONTENT` is a **privileged intent**. You must:
1. Enable **Message Content Intent** in the Discord Developer Portal under **Bot → Privileged Gateway Intents**.
2. Opt in explicitly in code: `.intents(Intents::MESSAGE_CONTENT)`.

*(DMs, messages mentioning the bot, and the bot's own messages always include content even without the intent.)*

## `cache.permissions(...)` returns `PermissionCacheError`

`cache.permissions(guild, user, channel)` computes permissions locally without REST calls, so it requires all of the following to be enabled in `CacheConfig`:

- `ResourceType::GUILDS | ResourceType::CHANNELS | ResourceType::ROLES | ResourceType::MEMBERS`
- `CacheParts::MEMBER_ROLES | CacheParts::CHANNEL_OVERWRITES`

The returned `PermissionCacheError` variant tells you exactly which resource or part is missing (e.g. `MemberRolesOff`, `OverwritesOff`, `Member`, `Roles`).

## My slash commands don't show up immediately

- **Global commands** can take a few minutes to propagate across Discord clients (restarting the Discord client often refreshes them faster).
- During development, register commands on a single test server with `.guild_command(guild_id, ...)` — guild commands appear **instantly**.
- Note that `Bot` skips re-registering commands if Discord already has identical definitions.

## `GUILD_MEMBER_ADD` / `GUILD_MEMBER_REMOVE` events never fire

`GUILD_MEMBERS` is a **privileged intent**. Enable **Server Members Intent** in the Discord Developer Portal and add `.intents(Intents::GUILD_MEMBERS)` to your `Bot` or `ShardBuilder`.

## How do I reduce RAM usage on large bots?

1. Use `CachePolicy::Lazy` for `ResourceType::MEMBERS | ResourceType::USERS` so `GUILD_CREATE` bulk dumps are skipped.
2. Only enable the `CacheParts` you actually read (for example, omit `USER_PROFILE` and `MEMBER_PROFILE` if you only need role IDs for permission checks).
3. Set caps via `.max_members_per_guild(Some(...))`, `.max_presences_per_guild(Some(...))`, and `.message_cache_size(...)`.
4. Enable `zstd-stream` if gateway bandwidth is a bottleneck.

## Does `cordex` support voice?

No, voice (UDP/RTP/Opus) is intentionally out of scope so `cordex` stays lean, `#![forbid(unsafe_code)]`, and focused on the bot API. You can still receive `VOICE_STATE_UPDATE` and `VOICE_SERVER_UPDATE` events to integrate with external voice servers like Lavalink.
