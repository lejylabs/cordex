# Changelog

All notable changes are listed here. The project follows
[Semantic Versioning](https://semver.org/); before 1.0 a minor release
(`0.x`) may break the API.

## Unreleased

- **Social SDK REST and models (measured coverage 100 %):** lobbies
  (`POST`/`PUT`/`GET`/`PATCH`/`DELETE /lobbies`, members, bulk update,
  channel linking, messages, moderation metadata, linked-channel invites)
  and application identity profiles (get/update/delete identities and
  profile data). Models: `Lobby`, `LobbyMember`, `LobbyMessage`,
  `SdkDmMessage`, `ApplicationIdentityProfile`, `ProfileData`, dynamic
  fields. Webhook events `LOBBY_MESSAGE_*` and `GAME_DIRECT_MESSAGE_*`
  parse to typed payloads. REST 235/235, fields 1270/1270, bitflags
  142/142, total 2094/2094 against `discord-api-docs@9b737ec51ee8`.
- **Phase 8 ergonomics:** embed builder (`Embed::new().title(..)` with local
  limits), CDN URLs (`user.avatar_url()`, guild icon/banner, emoji, sticker,
  …), `mention()` on snowflake IDs, mention parsing, timestamp markdown,
  `Standby` / `ctx.wait_for_*`, permission calculator
  (`permission_calc` and `cache.permissions`),
  `ClientBuilder::default_allowed_mentions` / `Bot::default_allowed_mentions`,
  `parse_webhook_url`.
- Fixes found in review of the Phase 8 work:
  - `cache.permissions`: threads use the parent channel's overwrites; errors
    instead of wrong results when `MEMBER_ROLES` / `CHANNEL_OVERWRITES` are
    off or a member role is not cached; owners and administrators are exempt
    from timeouts (also in `permission_calc::channel_permissions`).
  - `Standby`: waiters that timed out or were dropped are removed on the next
    event even if it does not match (no unbounded growth); stream waiters use
    a bounded buffer (`STANDBY_STREAM_CAPACITY`, extra events are dropped with
    a warning).
  - `ctx.wait_for_button` returns a `ComponentContext` (answer with
    `update`/`reply`); a component taken by a waiter is no longer routed to
    `on_component` handlers too (no double answer). Docs say which events
    and intents each `wait_for_*` needs.
  - CDN: GIF stickers use `media.discordapp.net`; `CdnUrl::format` picks
    `webp`/`jpg`/`png`/`gif`.
  - `parse_mentions` no longer skips a mention after a stray `<`; unknown
    timestamp styles are not parsed.
  - Embed fields reject empty names and values (`EmbedError::Empty`).
  - `parse_webhook_url` accepts `discordapp.com` hosts and ignores query
    strings.
- **All gateway receive events are typed** (82/82): new `VOICE_CHANNEL_STATUS_UPDATE`,
  `VOICE_CHANNEL_START_TIME_UPDATE`, `CHANNEL_INFO`, `INTEGRATION_CREATE` /
  `UPDATE` / `DELETE`, `SUBSCRIPTION_CREATE` / `UPDATE` / `DELETE` and
  `RATE_LIMITED`, with matching `EventTypeFlags`.
- `cordex-http`: guild templates, voice regions and voice states (stage
  moderation), invite target users (CSV upload, bulk add/remove, job status),
  `search_guild_messages` with filters (returns `SearchResult::NotIndexed`
  on Discord's 202 answer), `GET /oauth2/applications/@me` and
  `GET /oauth2/@me`.
- `cordex-model`: `GuildTemplate`.
- Coverage: routes the docs mark as deprecated are no longer counted.
- Coverage now also measures **object fields** and **bitflags**.
- `cordex-model`: fields the comparison found missing: all `Activity` fields
  are on `PresenceActivity` already; added `Connection.integrations`,
  `Identify.capabilities`, `GuildMembersChunk.presences`,
  `Message.channel_type`, `SkuFlags` (`Sku.flags` is now typed).
- `cordex-model`: webhook events (`webhook_event`): `WebhookEventPayload`,
  `WebhookEventBody::parse` into `ApplicationAuthorized`,
  `ApplicationDeauthorized` and entitlement events.

## 0.1.0-beta.4 - 2026-11-23

Coverage against the official docs (`discord-api-docs@9b737ec51ee8`, see
`docs/coverage.md`): REST endpoints 194/238 (beta.3: 77), gateway receive
events 71/82, gateway opcodes, close codes, component types and enum values
100 %; object fields and bitflags are not measured yet.

- `cordex-http`: 43 more REST endpoints (REST coverage 77 → 120 of 238):
  channel invites, typing, followers, voice channel status, group DM
  recipients; thread members and archived threads; crosspost and reaction
  users; single commands and command permissions; original interaction
  response; scheduled event list/get/users; audit log with filters; computed
  member/role permissions (guild and channel); current user update, guilds,
  guild member, connections and role connections; `GET /gateway`; guild
  webhooks, webhook updates, Slack/GitHub-compatible execution.
- `cordex-http`: 26 guild endpoints: update guild, channel positions, list
  and search members, add member (`OAuth2`), current member profile, bulk
  ban (limits checked locally), single role, role member counts, prune count
  and prune, voice regions, integrations, widget settings/data/image, vanity
  URL, welcome screen, onboarding, incident actions.
- `cordex-http`: 48 more endpoints (REST coverage now 194 of 238):
  guild and application emojis, stickers (form upload) and sticker packs,
  soundboard sounds (play, list, create, edit, delete), auto moderation rules,
  stage instances, entitlements (get, consume, test entitlements), SKUs and
  SKU subscriptions, current application (get, edit), activity instances,
  role connection metadata, poll answer voters and ending polls. Upload sizes
  (emoji 256 KiB, sticker and sound 512 KiB) are checked before sending.
- Docs watch: the daily workflow opens an issue with a checklist of new and
  removed endpoints, events and enum values plus the changed bot API pages;
  `cargo xtask coverage` downloads the docs as a tarball by commit hash.
- `cordex-http`: request builder types are exported from `cordex_http::endpoint`.
- `cordex-model`: `Sku`/`SkuType`, `Subscription`/`SubscriptionStatus`,
  `StickerPack`, `RoleConnectionMetadata`/`RoleConnectionMetadataType`,
  `ActivityInstance`.
- `cordex-model`: `FollowedChannel`, `UserGuild`, `Connection`,
  `ApplicationRoleConnection`, `GuildPreview`, `GuildWidget(Settings)`,
  `Onboarding` (+ prompts, options, mode and prompt type), `PruneCount`,
  `VanityUrl`, `BulkBan`, `RoleMemberCounts`.

## 0.1.0-beta.3 - 2026-11-23

Coverage against the official docs (`discord-api-docs@9b737ec51ee8`, see
`docs/coverage.md`): REST endpoints 77/238, gateway receive events 71/82,
gateway opcodes, close codes, component types and enum values 100 %; object
fields and bitflags are not measured yet.

- Simple layer (`cordex::bot`, facade feature `bot`, default on): `Bot`
  with plain async closures as handlers (`command`, `autocomplete`,
  `on_button`/`on_component`, `on_modal`, `on_message`, `on_ready`,
  `on_event`). Intents follow from the handlers, shards are automatic,
  commands are registered with one bulk overwrite on the first `READY`
  (global or per guild), commands and modals are deferred automatically
  after 2.5 s, handler errors and panics are logged and answered with an
  ephemeral error message. Optional cache via `Bot::cache`. On start the
  registered commands are compared with the local ones; unchanged commands
  are not overwritten again.
- `cordex-interactions`: `Modal` builder (`text_input`, `paragraph`,
  `component`, local limit checks) and `InteractionHandle::show_modal`.
- `cordex-gateway`: fatal close errors name the fix (for example 4014: which
  developer portal switch to turn on, 4004: reset the token);
  `CloseCode::hint()` in `cordex-model`.
- `cordex-model`: short constructors for common components
  (`ActionRow::new`, `Button::primary`/`secondary`/`success`/`danger`/`link`,
  `StringSelect::new`, `SelectOption::new`, `TextInput::short`/`paragraph`,
  `Label::new`) and `From<…> for Component`.
- `cordex::bot`: only the events a handler or the cache reads are parsed
  (gateway event filter from the handlers); no intents by default (a command
  bot needs none); handlers run in one task each, panics are caught in place.
- `cargo xtask coverage` and `docs/coverage.md`: API coverage against a pinned
  commit of the official docs; CI checks the report and a daily workflow opens
  an issue when the docs change.

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
