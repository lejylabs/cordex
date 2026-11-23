# Changelog

All notable changes are listed here. The project follows
[Semantic Versioning](https://semver.org/); before 1.0 a minor release
(`0.x`) may break the API.

## 0.1.3 - 2026-11-23

- **Ergonomics additions (`cordex`, `cordex-http`, `cordex-model`):**
  - **Shared bot state:** `Bot::data(value)` and `Context::data::<T>() -> Option<&T>` store arbitrary `Send + Sync + 'static` state by type without extra locking.
  - **Global error handler & pre-command hook:** `Bot::on_error(|ctx, err| ...)` catches every handler `Err` centrally; `Bot::before_command(|ctx| ...)` runs before every command handler (return `Ok(false)` to block execution).
  - **`MessageBuilder` with Markdown escaping:** `cordex_model::message::MessageBuilder` (re-exported in `cordex::prelude`) with `push`, `push_line`, `push_safe`, `push_bold[_safe]`, `push_italic[_safe]`, `push_underline[_safe]`, `push_strike[_safe]`, `push_spoiler[_safe]`, `push_mono[_safe]`, `push_quote_line`, `push_codeblock`, `push_named_link[_safe]`, `mention`, `push_timestamp`, `build` and `into_string`.
  - **Built-in graceful shutdown:** `Bot::run_until_ctrl_c()` (handles `Ctrl+C` and Unix `SIGTERM`) and `Bot::run_until(future)` shut down all shards cleanly.
  - **Pagination & collector ergonomics:** `CommandContext::paginate(pages, timeout)` renders `◀` / `1 / N` / `▶` buttons with an author-only guard, sliding idle timeout, and automatic button disabling on timeout; `ButtonClicks::idle_timeout(duration)` and `ButtonClicks::stop()` added.
  - **Scoped audit-log reasons & shortcuts:** `Client::with_reason(...)` returns a cheap scoped clone that attaches `X-Audit-Log-Reason` to every request; `Client::forward_message(...)`, `Context::forward_message(...)`, `Context::send(channel_id, content)` and `Context::dm(user_id, content)` added.

## 0.1.2 - 2026-11-23

- **`cordex-http`: HTTP/2 via ALPN enabled by default** (`http2` feature on `cordex-http` and `cordex`, `http2_adaptive_window(true)`), falling back to HTTP/1.1 automatically; `ClientBuilder::http1_only(true)` forces HTTP/1.1 at runtime, or disable `http2` for smaller binaries.
- **Smaller binaries:** non-generic `#[inline(never)]` `parse_component` in `cordex-model` and `[profile.release.package.cordex-model] opt-level = "s"` in the workspace release profile.
- **Optional `metrics` feature** on `cordex-http`, `cordex-gateway` and `cordex`: exports HTTP and gateway counters through the `metrics` crate (off by default).
- **Performance pass:**
  - `cordex-gateway`: zero-copy single-frame `zlib-stream` fast path (`ZlibStream::try_inflate_single` + reused `zlib_out` buffer in `Session`), zero-copy outbound `Bytes` in `Session::flush_user`, lock-free `AtomicUsize` fast path in `Standby::process`, single-lock `EventPipe::pop` and race-free `wait_space`.
  - `cordex-cache`: `O(1)` `release_users` via refcount map, per-guild `guild_channels`/`guild_threads` indexes (`O(guild_channels)` instead of full-cache scans), and a zero-dependency integer `IdHasher` for `Id<T>` keys.
  - `cordex-http`: merged `last_used` into `BucketState` (halving mutex locks and `Instant::now()` calls per request), `Cow<'static, str>` route templates (`Route::raw` / `Route::new`), pre-sized `Multipart::encode` / `encode_form` buffers, and removed remaining `serde_json::from_slice` calls.
  - `cordex-interactions`: direct `Choice::into_model()` conversion in `autocomplete` (eliminating the `serde_json::Value` round-trip).
  - `cordex-model`: fast top-level `"type"` peek in `Component::deserialize` before falling back to serde.
- **Breaking — smaller model memory footprint:**
  - `cordex-model`: changed read-only list fields on `Message`, `Channel`, `Guild`, `Member`, `GuildMemberUpdate` and `PresenceUpdate` from `Vec<T>` to `Box<[T]>` (saving 8 bytes per field plus spare capacity), and boxed rare optional sub-structs on `User`, `Member`, `GuildMemberUpdate`, `Role`, `Guild`, `Channel`, `Message` and `PresenceUpdate`.
  - `cordex-http`: `Route::new` / `Route::raw` now take `impl Into<Cow<'static, str>>` so static route templates avoid heap allocations.

## 0.1.1 - 2026-11-23

- **`cordex-cache`: per-resource `CachePolicy` (`Off`, `Lazy`, `Eager`).**
  `CacheConfig::lazy(...)` and `CacheConfig::with_policy(..., CachePolicy::Lazy)`
  skip bulk dumps (`GUILD_CREATE`, `GUILD_MEMBERS_CHUNK`, `THREAD_LIST_SYNC`,
  `SOUNDBOARD_SOUNDS`) and cache entries only when seen in live events
  (messages, interactions, member/voice/guild/channel/role updates, etc.).
  Purely lazy configs omit `GUILD_CREATE` from `wanted_events()`.
  `GuildMemberUpdate::to_member()` added in `cordex-model`.

## 0.1.0 - 2026-11-23

First regular release. Covers 100 % of the measured official bot API
(`docs/coverage.md`: endpoints, gateway events, opcodes, close codes, enum
values, object fields and flags against `discord-api-docs@9b737ec51ee8`).
From here on normal SemVer for `0.x`: a minor release may break the API, a
patch release never does. Changes since `0.1.0-beta.4`:

- **Metrics getters:** `ShardStatus::counters` (`ShardCounters`: identifies,
  resumes, dispatches, dropped events, bytes received and decompressed) and
  `Client::metrics()` (`ClientMetrics`: requests, transport errors,
  timeouts, 429s by scope, 5xx, invalid-request window, active buckets).
  `RateLimiter::bucket_count` is a new provided method (default `None`).
  `ShardStatus` is now `#[non_exhaustive]`.
- **Configurable decompressed-message cap:** `max_message_bytes` on
  `ShardBuilder` and `ShardManagerBuilder` (default
  `DEFAULT_MAX_MESSAGE_BYTES`, 16 MiB).
- **Faster parsing and caching:** `INTERACTION_CREATE` parses `data` in one
  pass (1.82 → 1.62 µs, 15 → 12 allocations); the cache handles
  `GUILD_CREATE` with one lock per map, reserved capacity and no scan of all
  cached channels on a guild's first `GUILD_CREATE` (806 → 657 µs for 1000
  members). Details in `docs/performance.md`.
- **Breaking:** `User::discriminator` is a `Discriminator(u16)` (`get`,
  `is_none`, `Display` as `0` or `0042`) instead of `Box<str>`; saves an
  allocation per user. `cdn::user_avatar` / `cdn::default_avatar` take a
  `Discriminator`.
- **Fix: modal submits with a checkbox failed to parse.** Discord sends a
  checkbox `value` as a boolean; `SubmittedComponent::value` is now a
  `SubmittedValue` (`Text` / `Bool`, with `as_str` / `as_bool`). New
  `ModalContext::checked(id)`; a radio group's choice is read with
  `ModalContext::value(id)` (Discord sends one `value`, not `values`).
  Found in the live test.
- **Fix: `cache.permissions` failed with "member is not cached"** for the
  user of a command. Without the presences intent `GUILD_CREATE` carries
  almost no members; the cache (with `MEMBERS` on) now stores the invoking
  member of every guild interaction.
- **`Bot::register_commands(false)`** switches the automatic command
  registration off, for commands registered elsewhere (deploy step, another
  process of the same application).
- **Benchmarks:** `cargo xtask bench` runs criterion benches (gateway parsing,
  filtered events, cache update and lookup) and a dhat allocation bench, and
  compares with `benches/baseline.json` (warning at > 10 % slower). Results
  in `docs/performance.md`: `MESSAGE_CREATE` 1.4 µs and 7 allocations, a
  filtered event 0.36 µs, cache lookups ≈ 20 ns.
- **`InteractionClient`**: `client.interaction(app_id)` (or
  `client.interaction_for_current_app().await`) gives the application-scoped
  endpoints without repeating the ID: global and guild commands (incl. bulk
  `set_*_commands`), command permissions, interaction responses and
  follow-ups, application emojis, entitlements, SKUs, role connection
  metadata, activity instances.
- **Builders for every component**, including Components V2 (`Container`,
  `Section`, `TextDisplay`, `Thumbnail`, `MediaGallery`, `FileComponent`,
  `Separator`), user/role/mentionable/channel selects (`SelectMenu::users(..)`
  etc.), modal radio groups, checkbox groups, checkboxes and file uploads.
- **`IS_COMPONENTS_V2` is set automatically** for messages, follow-ups,
  interaction responses and webhooks that contain V2 components;
  `Reply::layout(..)` for V2-only replies, and a local error when V2
  components are combined with content or embeds. Webhook executions can
  send components now (`ExecuteWebhook::components`).
- **`ctx.button_clicks(message, timeout)`**: collect all clicks on a message
  (polls, paging); each is a `ComponentContext`.
- **Live resharding in `Bot`**: with automatic sharding the bot checks every
  8 hours (`Bot::reshard_interval`) whether Discord recommends more shards,
  starts the new shard set, waits until all shards are ready, switches over
  and closes the old shards. `Context::shards()` now returns
  `Arc<ShardManager>`.
- **Examples per task**: `buttons_and_waiting`, `components_v2`,
  `embeds_and_modals`, `cache_and_permissions`, `manual_sharding`,
  `http_only`, `interactions_http`.
- **Smaller and faster model parsing:** `#[serde(flatten)]` and
  `#[serde(untagged)]` are gone from the deserialize side (`GUILD_CREATE` is
  no longer buffered as a value tree and parsed twice; scalar option values
  use a visitor), and REST/HTTP-interaction bodies are parsed from `&str` like
  gateway payloads. Release binary of `examples/bot.rs`: 9.85 → 9.16 MiB.
- `Id::created_at()` returns the creation time as a `Timestamp`.
- **Docs:** new README for the project and for every crate (badges, overview,
  examples), each README is the crate's rustdoc front page and its examples
  run as doctests; clearer crates.io descriptions; CONTRIBUTING with project
  layout; the live test bot is no longer in the published `cordex` package.
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
