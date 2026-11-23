# Changelog

All notable changes are listed here. The project follows
[Semantic Versioning](https://semver.org/); before 1.0 a minor release
(`0.x`) may break the API.

## Unreleased

- Simple layer (`cordex::bot`, facade feature `bot`, default on): `Bot`
  with plain async closures as handlers (`command`, `autocomplete`,
  `on_button`/`on_component`, `on_modal`, `on_message`, `on_ready`,
  `on_event`). Intents follow from the handlers, shards are automatic,
  commands are registered with one bulk overwrite on the first `READY`
  (global or per guild), commands and modals are deferred automatically
  after 2.5 s, handler errors and panics are logged and answered with an
  ephemeral error message. Optional cache via `Bot::cache`.
- `cordex-model`: short constructors for common components
  (`ActionRow::new`, `Button::primary`/`secondary`/`success`/`danger`/`link`,
  `StringSelect::new`, `SelectOption::new`, `TextInput::short`/`paragraph`,
  `Label::new`) and `From<…> for Component`.

## 0.1.0-beta.2 - 2026-11-23

- `cordex-cache` (new, facade feature `cache`): optional in-memory cache fed
  with `update(&event)`. Everything is off by default: 14 resources
  (`ResourceType`), 11 optional parts (`CacheParts`), limits for messages per
  channel, members and presences per guild, `set_config` at runtime (switching
  off drops data at once), `validate()` warnings, `wanted_events()` for the
  gateway filter, entry counts via `stats()`.
- `cordex-gateway`: `EventTypeFlags` can be collected from dispatch names.
- `cordex::backfill` (features `cache` + `http`): load data from before the
  bot started on request: `messages` / `guild_messages` (REST, paged, bounded
  parallelism, forbidden channels reported) and `guild_members` (gateway,
  needs `GUILD_MEMBERS`). `InMemoryCache::insert_messages` /
  `insert_members` / `guild_threads`.
- `cordex-http`: fixed a route being blocked for an hour when Discord answered
  without rate limit headers while its bucket was still unknown.

## 0.1.0-beta.1 - 2026-11-22

First beta. Rust 1.95 or newer.

- `cordex-model`: typed Discord objects (users, guilds, members, roles,
  channels, threads, messages, embeds, components incl. Components V2,
  interactions, commands, scheduled events, auto moderation, invites,
  webhooks, stage instances, soundboard, presences, audit log, polls,
  entitlements) and gateway events; unknown enum values and events are kept.
- `cordex-http`: REST client with per-route and global rate limiter, invalid
  request guard, retries, multipart, local validation, typed endpoints for
  the common resources, raw requests, `mock` client for tests.
- `cordex-gateway`: shards with zlib-stream, heartbeat/zombie detection,
  resume/reconnect, close-code actions, outbound limiter, event filter,
  backpressure, panic guard; shard manager (automatic and manual), identify
  queue, session start limits, custom spawners for thread-per-core.
- `cordex-interactions`: command builders with local validation, typed
  option access, `InteractionHandle` (respond, defer, auto-defer, edit,
  follow-up, autocomplete, modal), `Reply`; Ed25519 verification and a hyper
  endpoint behind the `verify` / `server` features.
- `cordex`: facade with features, one `Error` type and a `prelude`.
