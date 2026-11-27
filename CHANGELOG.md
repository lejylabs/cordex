# Changelog

All notable changes are listed here. The project follows
[Semantic Versioning](https://semver.org/); before 1.0 a minor release
(`0.x`) may break the API.

## Unreleased

## 0.2.4 - 2026-11-27

- **Bug Fixes:**
  - `Bot::run_until` now shuts down with `CloseMode::Resumable` when a `SessionStore` is configured so active shard sessions are persisted instead of cleared on graceful shutdown.
  - `reshard_loop` / `prepare_reshard` now preserve `AutoShardingConfig`, `presence_for_shard`, `identify_queue`, `session_store`, and `spawn_with`, and evaluate resharding against `AutoShardingConfig::resolve(recommended)`.
  - `ShardManager::start` now compares Discord's recommended shard count against `total_shards` instead of the local process's shard slice, preventing false warnings in multi-process clusters.
  - `ButtonClicks::next` no longer resets the `idle_timeout` deadline when a click is rejected by `.filter(...)`.
- **Performance & Memory:**
  - `RedisSessionStore` now reuses a cached Redis connection (with automatic reconnect on failure) instead of opening a new TCP connection per shard call.
  - `UnixSocketTransport` now pools up to 16 idle HTTP/1.1 keep-alive connections instead of reconnecting on every request.
  - `StringInterner::intern` now automatically prunes unreferenced entries when growing past 1,024 strings.
  - Capped `ShardManager`'s fan-in event channel capacity to prevent excessive memory allocation on high shard counts.

## 0.2.3 - 2026-11-27

- **Extended opt-in database type integrations (`cordex-model`):**
  - Added `Locale`, all `u64` bitflags (`Intents`, `UserFlags`, `MessageFlags`, `ChannelFlags`, `MemberFlags`, `RoleFlags`, `SystemChannelFlags`), and key `u8` enums (`ChannelType`, `InteractionType`, `PremiumTier`, `AutoModerationActionType`) across `postgres-types`, `rusqlite`, `redis`, and `mysql`.
  - Added native PostgreSQL `JSONB` / `JSON` `ToSql` / `FromSql` support for `Embed`, `Component`, and `AllowedMentions` (`postgres-types` feature).
- **Bot-layer interaction guards & collector enhancements (`cordex`):**
  - Added `require_guild()`, `require_nsfw()`, `require_role(role_id)`, `require_entitlement(sku_id)`, and `require_owner(&owners)` on `CommandContext`, `ComponentContext`, and `ModalContext` (with matching `Error::GuildOnly`, `Error::NsfwOnly`, `Error::MissingRole`, `Error::MissingEntitlement`, and `Error::OwnerOnly` variants).
  - Extended `ButtonClicks` with `.max(limit)`, `.filter(predicate)`, `.collected()`, and `.end_reason() -> Option<CollectorEndReason>` (`Timeout`, `Idle`, `Limit`, `Stopped`, `Closed`).
- **Redis `SessionStore` backend (`cordex-gateway`, `redis` feature):**
  - Added `RedisSessionStore` (configurable key prefix and TTL) for zero-downtime gateway `RESUME` across container and multi-node restarts; enabled automatically when `cordex` is built with `features = ["redis"]`.
- **Flexible automatic & manual sharding (`cordex-gateway` & `cordex`):**
  - Added `AutoShardingConfig` (`guilds_per_shard`, `min_shards`, `max_shards`, `round_up_to_multiple`) plus `ShardManagerBuilder::auto_sharding` and `Bot::auto_sharding` / `Bot::guilds_per_shard`.
  - Added `cluster_shard_range(cluster_id, cluster_count, total)`, `ShardManagerBuilder::shard_ids`, `ShardManagerBuilder::cluster`, `ShardManagerBuilder::session_store`, and `ShardManagerBuilder::presence_for_shard`.
  - Exposed `Bot::shard_ids`, `Bot::shard_cluster`, `Bot::identify_queue`, `Bot::session_store`, `Bot::spawn_with`, and `Bot::presence_for_shard`.

## 0.2.2 - 2026-11-26

- **Bug Fix (`cordex-http`):**
  - Supplied `TokioTimer::new()` via `.timer(...)` on the `hyper` client builder in `HyperTransport::new` so HTTP/2 keep-alive no longer panics at runtime (`You must supply a timer.`) when HTTP/2 is negotiated.

## 0.2.1 - 2026-11-26

- **New opt-in derive-macro crate `cordex-macros` (`macros` feature):**
  - Added `#[derive(CommandArgs)]` and `#[derive(CommandChoice)]` plus `CommandArgs` / `CommandChoice` traits, `Command::with_args::<A>()`, and `CommandOption::choices(...)` (strictly opt-in, off by default).
- **Opt-in database & cache type integrations (`cordex-model` & `cordex`, all off by default):**
  - `postgres-types`: binary wire-format `ToSql` / `FromSql` for `Id<T>` (`INT8`), `Permissions` (`INT8`), `Timestamp` (`TIMESTAMPTZ`), and `ImageHash` (`TEXT` / `VARCHAR`).
  - `rusqlite` (SQLite): `ToSql` / `FromSql` for `Id<T>`, `Permissions`, `Timestamp`, and `ImageHash`.
  - `redis`: `ToRedisArgs` / `FromRedisValue` for `Id<T>`, `Permissions`, `Timestamp`, and `ImageHash`.
  - `mysql` (MySQL / MariaDB via `mysql_common`, works with both `mysql` and `mysql_async`): `FromValue` / `Into<Value>` for `Id<T>`, `Permissions`, `Timestamp`, and `ImageHash`.
- **New Axum integration crate `cordex-axum`:**
  - Added `VerifiedInteraction` Ed25519 extractor (`401 Unauthorized` on invalid/missing signature), `interactions_router` (automatic `PING` → `PONG`), and `health_router` (`/healthz` & `/readyz` backed by `HealthSnapshot`).
- **Linux / Transport & Memory Performance Tuning:**
  - Enabled TCP keepalive (30 s idle, 10 s interval, 3 retries), HTTP/2 keepalive probes, `BytesMut` direct JSON serialization, and `#[cfg(unix)]` `ClientBuilder::unix_socket` (UDS transport) in `cordex-http`.
  - Added `StringInterner` (`Arc<str>` deduplication + `prune_unused`) and `CacheConfig::compact()` minimal-RAM preset in `cordex-cache`.
  - Added opt-in `simd-json` feature on `cordex-model` & `cordex` (`cordex_model::simd_json::from_slice`).
  - Documented Linux allocator / `zstd-stream` / thread-per-core / release-profile tuning in `book/src/production.md`.

## 0.2.0 - 2026-11-26

- **Background Tasks / Scheduler, First-Class i18n, OAuth2 & WASM:**
  - Added lifecycle-bound `Bot::every`, `Bot::every_immediate`, and `Bot::background` (and on `BotModule`) with `ShutdownToken`, `SchedulerHandle`, `MissedTickBehavior::Skip`, panic/error isolation, and graceful shutdown.
  - Added `Locale` enum in `cordex-model`, `locale()` / `guild_locale()` / `effective_locale(default)` on all interaction contexts, bulk `.name_localizations` / `.description_localizations` & `.name_localized` / `.description_localized` shortcuts on `Command` / `CommandOption`, and `book/src/i18n.md`.
  - Added opt-in `oauth2` feature on `cordex-http` / `cordex` (`OAuth2Client`, `OAuth2Config`, `TokenResponse`, `PartialGuild::can_manage`, PKCE `S256` authorize URLs, token exchange/refresh/revoke, Bearer endpoints) and `wasm32-unknown-unknown` CI check for `cordex-model` & `cordex-interactions`.
- **Offline Testing-Harness & Gateway-Replay (new crate `cordex-test` + `cordex-gateway/record`):**
  - Added `cordex-test` with `TestBot` (wraps any `Bot` without code changes), in-memory `MockHttp` recorder & error injection, cache pre-seed helpers (`seed_guild`, `seed_channel`, `seed_user`, `seed_member`), interaction/event builders (`slash`, `click_button`, `submit_modal`, `trigger_autocomplete`, `send_message`), rich assertions (`assert_replied`, `assert_ephemeral`, `assert_deferred`, `assert_modal`, `assert_autocomplete_choices`, `assert_sent_message`), and full `tokio::time::pause()` / `advance()` compatibility.
  - Added opt-in `record` feature on `cordex-gateway` (`TrafficRecorder`, `RecordedFrame`, consistent 1:1 `Anonymizer`) and `TestBot::replay_jsonl` / `replay_jsonl_str`.
- **Discord Everyday Toolkit, Middleware & Components V2:**
  - Added `Bot::around_command`, `BotModule::around_command`, and `Next::run(ctx)` for composable command middleware.
  - Added `SyncReport`, `diff_commands`, `dev_guild_id_from_env`, and `Bot::sync_dev_guild_from_env()` (`DISCORD_DEV_GUILD_ID`).
  - Added stateless HMAC-SHA256 signed `custom_id` payloads (`CustomIdSigner`, `SignedCustomId`, `CustomIdError`) within Discord's 100-char limit (pure Rust, zero external deps).
  - Added fluent Components V2 `LayoutBuilder` (`text`, `separator`, `section`, `media_gallery`, `file`, `container`, `thumbnail`) with local limit validation and automatic `IS_COMPONENTS_V2`.
  - Added codeblock-aware `split_message` / `split_message_2000` / `split_message_4096`, `escape_markdown`, `escape_mentions`, and `permission_calc::highest_role_position`.
- **Production Resilience, Cache Eviction & Observability:**
  - Added `SessionStore` trait and atomic `FileSessionStore` in `cordex-gateway` for zero-downtime `RESUME` persistence across restarts.
  - Added 3-tier priority-aware global rate limiting (`RequestPriority::{Critical, Normal, Background}` with fair-scheduling aging) in `cordex-http` (`RequestBuilder::priority`).
  - Added bounded cache eviction & write-through in `cordex-cache` (`max_messages_per_channel`, `max_users`, `member_ttl`, `write_through`, `insert_guild`, `insert_channel`, `insert_user`, `insert_guild_member`) and REST-fallback write-through in `Context`.
  - Added `Context::health_snapshot() -> HealthSnapshot` (`ShardHealth`, `CacheHealth`, `is_ready()`, `to_json()`).
- **Performance, Memory-Footprint & Bug-Hardening:**
  - Hardened `InteractionHandle` auto-defer with an explicit state machine (`ResponseState`) to prevent double-ack races (`40060`), added non-`1000` resumable WebSocket close codes (`4000`), `Standby` cancellation `Drop` guards, cooldown map eviction, HTTP rate-limit bucket cleanup, internal `FxHash` for snowflake cache maps, decompress buffer high-water-mark shrinking, and `size_of` regression tests.

## 0.1.9 - 2026-11-25

- **New zero-dependency CLI crate `cordex-migrate`:**
  - `cordex-migrate analyze [PATH]` scans `Cargo.toml` and `src/**/*.rs` for `serenity`, `poise`, `songbird`, and `twilight-*` usage, recommends the exact `cordex` dependency and minimal feature set, and lists safe auto-rewrites vs. manual follow-ups.
  - `cordex-migrate apply [PATH] [--dry-run | --write] [--no-todos]` performs deterministic mechanical rewrites outside comments and string literals and inserts idempotent `// TODO(cordex-migrate): ...` guidance notes linked to the expanded migration guide.
- **`cordex-model`:**
  - Added `EmbedAudio` and `Embed.audio` (pinned `discord-api-docs@5a61ef8ebaf5`, #3).
  - Added `parse_invite_url(url)` (re-exported at crate root) and `ParseError::InviteUrl` for parsing `discord.gg/{code}` and `discord.com/invite/{code}` URLs.
  - Added zero-cost snowflake ID type aliases (`GuildId`, `ChannelId`, `UserId`, `RoleId`, `MessageId`, `EmojiId`, `ApplicationId`, `AttachmentId`, `WebhookId`, `CommandId`, `InteractionId`, and all remaining markers in `cordex_model::id`; common aliases re-exported at crate root and in `cordex::prelude`).
  - Added `cordex_model::message::colors` RGB constants (`BLURPLE`, `GREEN`, `YELLOW`, `RED`, `ORANGE`, `BLUE`, `TEAL`, `PURPLE`, `FUCHSIA`, `WHITE`, `DARK_GREY`, `BLACK`) and preset constructors `Embed::success`, `Embed::error`, `Embed::info`, and `Embed::warn`.
  - Added `StringSelect::disabled()`.
- **`cordex` facade (`Bot`):**
  - Added `BotModule` and `Bot::mount` (nestable) to split commands, component/modal handlers, events, intents, and module-scoped `before_command` guards across files.
  - Added dynamic component and modal routing: `on_component_prefix` / `on_button_prefix` / `on_modal_prefix` (longest prefix wins, exposing `ctx.matched_tail()`) and `{param}` segment pattern routing via `on_component_pattern` / `on_button_pattern` / `on_modal_pattern` (exposing `ctx.param(...)` and `ctx.param_id::<T>(...)`).
  - Added high-level inline interactive flows: `CommandContext::confirm` / `confirm_with`, `CommandContext::select`, `CommandContext::prompt`, and `ask_modal` on `CommandContext` and `ComponentContext` (subscribes to `Standby` before opening the modal to prevent submit races).
  - Added command-level cooldowns (`Bot::command_with_cooldown`, `BotModule::command_with_cooldown`, `CooldownScope::{User, Channel, Guild, Global}`, `Bot::cooldown_message`) and chained `Bot::before_command` hooks.
  - Added subcommand helpers on `CommandContext`: `subcommand_group()`, `subcommand_name()`, and `subcommand_path()`.
  - Added beginner-safety shortcuts: `Bot::from_env()` / `Bot::from_env_var(...)` (`Error::MissingEnv`), `require_user_perm` / `require_bot_perm` (`Error::MissingPermission`), and a one-shot runtime `tracing::warn!` when guild messages arrive with empty content because `MESSAGE_CONTENT` is disabled.
- **Documentation:**
  - Greatly expanded `book/src/migration.md` with `cordex-migrate` walkthroughs, crate & type/method cheatsheets, 10 side-by-side Serenity/Poise & Twilight recipes, and a gotchas checklist.
  - Updated `book/src/simple-layer.md`, `book/src/messages.md`, `book/src/commands.md`, and READMEs for all `0.1.9` additions.

## 0.1.8 - 2026-11-24

- **`cordex-cache`:**
  - Added `PreviousState` and `InMemoryCache::update_with_previous(&event)` so diff-based features (audit logging, autorole `pending` transitions, voice channel switches) receive the replaced/removed cached entity (`User`, `Guild`, `Channel`, `Thread`, `Role`, `Member`, `VoiceState`, `Message`) without maintaining a second shadow cache.
  - Added `InMemoryCache::can_moderate(guild, moderator, target)` for Discord role-hierarchy checks.
  - Added `InMemoryCache::shrink_to_fit()` and `InMemoryCache::prune_members(max_per_guild)` for memory footprint control.
- **`cordex-model`:**
  - Added `permission_calc::highest_role` and `permission_calc::can_moderate` (owner protection, administrator protection, role position + ID tie-breaker).
  - Added `Component::count_nested`, `Component::total_nested`, and `Component::validate_v2_limits` (`<= 10` top-level, `<= 40` total nested components).
- **`cordex-interactions`:**
  - Added read-only accessors on `Reply` (`content_ref`, `embeds_ref`, `components_ref`, `allowed_mentions_ref`, `flags_value`, `is_tts`).
- **`cordex` facade (`Bot`):**
  - `Context::send` and `Context::dm` now accept `impl Into<Reply>` (plain text, embeds, and Components V2 with automatic `IS_COMPONENTS_V2` flag).
  - Added cache-first + REST-fallback helpers on `Context`: `get_guild`, `get_channel`, `get_member`, `get_user`, plus `sync_guild_commands` (diff-checked bulk overwrite).
  - Added `app_permissions`, `member_permissions`, `bot_can`, and `user_can` on `CommandContext`, `ComponentContext`, and `ModalContext`.
  - Added `EventContext::previous()` (with `cache` feature) exposing the `PreviousState` from `InMemoryCache::update_with_previous`.
  - Added `CommandContext::*_option_opt` shortcuts (`string_option_opt`, `integer_option_opt`, `number_option_opt`, `boolean_option_opt`, `user_option_opt`, `member_option_opt`, `role_option_opt`, `channel_option_opt`, `attachment_option_opt`).
  - Added `Context::wait_for_modal(custom_id, timeout)` for inline modal flows.
  - Added typed event shortcuts on `Bot`: `on_guild_create`, `on_member_join`, `on_member_leave`, `on_reaction_add`, `on_voice_state`.
  - Added `examples/moderation.rs` showcasing `bot_can`, `user_can`, `can_moderate`, `string_option_opt`, and audit-log reasons.
  - Added `Bot::shard_range(range, total)` for multi-process shard clustering.
  - Added `Bot::on_unknown_component` and `Bot::on_unknown_modal` fallback handlers.
  - Upgraded `Bot::on_component` / `Bot::on_modal` routing to longest `:`-delimited prefix matching (e.g. `"giveaway:join:42"` checks `"giveaway:join:42"`, `"giveaway:join"`, then `"giveaway"`).

## 0.1.7 - 2026-11-24

- **New optional crate `cordex-voice` (pure-Rust voice signaling & session coordination):**
  - `VoiceManager` tracks per-guild voice state (`channel_id`, `session_id`, `token`, `endpoint`, `self_mute`, `self_deaf`), correlates `VOICE_STATE_UPDATE` and `VOICE_SERVER_UPDATE` in either order, and exposes `join`, `join_with`, `leave`, `update_voice_state`, `connection_info`, `wait_for_connection`, `watch`, and `subscribe`.
  - Decoupled via the `VoiceGateway` trait (implemented for `cordex_gateway::Shard` and `ShardManager` behind the default `gateway` feature).
  - Includes `LavalinkVoiceState` (`info.to_lavalink()`) for Lavalink v4 payloads.
- **New optional crate `cordex-lavalink` (pure-Rust Lavalink v4 client):**
  - `LavalinkClient` connects to `/v4/websocket`, tracks `sessionId`, broadcasts `LavalinkEvent`s, and provides typed REST helpers (`load_tracks`, `update_player`, `update_voice`, `play`, `stop`, `set_paused`, `set_volume`, `destroy_player`).
  - `LavalinkClient::attach_voice(&voice)` automatically forwards `Connected`, `ServerUpdated`, and `Disconnected` events from `VoiceManager` to Lavalink.
- **`cordex-gateway`:**
  - Added `ShardId::for_guild`, `ShardManager::shard_for_guild`, and `ShardManager::update_voice_state`.
- **`cordex` facade:**
  - Added opt-in features `voice` and `lavalink` (off by default), `Bot::voice()`, `Context::voice()`, `Error::Voice`, `Error::Lavalink`, and `examples/voice_lavalink.rs`.

## 0.1.6 - 2026-11-24

- **`cordex-cache`:**
  - Added `InMemoryCache::guild_members_with_ids(guild)` (`Vec<(UserId, Arc<Member>)>`) and `InMemoryCache::guild_member_ids(guild)` (`Vec<UserId>`) so callers can look up cached users (e.g. `user.bot` for user/bot counters) without losing the map key when `member.user` is stripped.
  - Added `InMemoryCache::guild_voice_states(guild)` (`Vec<Arc<VoiceState>>`) to list all voice states in a guild directly.
- **`cordex-http` & `cordex-model`:**
  - Added `.permission_overwrites(Vec<PermissionOverwrite>)` on `CreateGuildChannel` and `UpdateChannel` (sending `"permission_overwrites": []` clears all overwrites).
  - Added `PermissionOverwrite::new`, `PermissionOverwrite::role`, and `PermissionOverwrite::member` constructors in `cordex-model`.

## 0.1.5 - 2026-11-24

- **`cordex-http`: attachment metadata & clearing on message, webhook, and interaction edits:**
  - `FollowupBody` (shared by `UpdateInteractionResponse` and `CreateFollowupMessage`) now populates `"attachments": [{"id": 0, "filename": ...}]` from attached files on send, so editing an interaction response with a new file replaces the previous attachment instead of appending.
  - Added `.clear_attachments()` and `.clear_embeds()` across `UpdateInteractionResponse`, `CreateFollowupMessage`, `EditMessage`, `CreateMessage`, and `UpdateWebhookMessage` (plus `.components(...)` on `UpdateWebhookMessage`) so edits can explicitly send `"attachments": []`, `"embeds": []`, and `"components": []`.
- **`cordex-model`: explicit clearing on `InteractionResponse` / `InteractionCallbackData`:**
  - Added `InteractionResponse::update_message(content)` (type 7) and `.clear_components()`, `.clear_embeds()`, `.clear_attachments()` on both `InteractionResponse` and `InteractionCallbackData` so type-7 updates can explicitly serialize `"components": []`, `"embeds": []`, and `"attachments": []`.
- **Documentation & crates.io metadata:** expanded the mdBook guide (`https://lejylabs.github.io/cordex/`), added `homepage` to all published crates, and linked the guide from all crate READMEs.

## 0.1.4 - 2026-11-24

- **`cordex-cache`: full event maintenance & cascade cleanups across all cached resources:**
  - **Guild member counts & `GUILD_UPDATE` preservation:** tracks `member_count` from `GUILD_CREATE`, increments/decrements on `GUILD_MEMBER_ADD` / `GUILD_MEMBER_REMOVE`, clears on `GUILD_DELETE`, and exposes `InMemoryCache::guild_member_count(id)`. `GUILD_UPDATE` preserves `approximate_member_count` / `approximate_presence_count` when omitted by Discord.
  - **Member removal cascades:** `GUILD_MEMBER_REMOVE` now also drops stale `voice_states` and `presences` for the removed member; `USER_UPDATE` updates `users` alongside `current_user`.
  - **Role deletion cascades (fixes `permissions()`):** `GUILD_ROLE_DELETE` removes the deleted role ID from cached `member.roles` and role `permission_overwrites` on guild channels.
  - **Channel deletion cascades & incremental field updates:** `CHANNEL_DELETE` drops child threads (and their cached messages), voice states and stage instances in that channel. `CHANNEL_PINS_UPDATE` updates `last_pin_timestamp`, `MESSAGE_CREATE` updates `last_message_id`, `THREAD_MEMBERS_UPDATE` updates thread `member_count`, and `GUILD_SCHEDULED_EVENT_USER_ADD` / `REMOVE` maintain `user_count`.
  - **Message reaction caching:** `MESSAGE_REACTION_ADD`, `MESSAGE_REACTION_REMOVE`, `MESSAGE_REACTION_REMOVE_EMOJI` and `MESSAGE_REACTION_REMOVE_ALL` maintain `Message.reactions` (normal & burst counts, `me`/`me_burst`, `burst_colors`) and cache reacting guild members when `MEMBERS` is enabled.
  - **`wanted_events()` updated** for all newly handled maintenance and cascade events.
- **`cordex-model`:** added `Reaction::single(...)` constructor for building `Reaction` entries.

## 0.1.3 - 2026-11-23

- **Optional `zstd-stream` gateway compression (`cordex-gateway`, `cordex`):** opt-in `zstd-stream` feature switches gateway transport compression from pure-Rust `zlib-stream` to `zstd-stream` via `zstd` (lower bandwidth and faster decompression; requires a C compiler to build `libzstd`, off by default).
- **Ergonomics additions (`cordex`, `cordex-http`, `cordex-model`):**
  - **Shared bot state:** `Bot::data(value)` and `Context::data::<T>() -> Option<&T>` store arbitrary `Send + Sync + 'static` state by type without extra locking.
  - **Global error handler & pre-command hook:** `Bot::on_error(|ctx, err| ...)` catches every handler `Err` centrally; `Bot::before_command(|ctx| ...)` runs before every command handler (return `Ok(false)` to block execution).
  - **`MessageBuilder` with Markdown escaping:** `cordex_model::message::MessageBuilder` (re-exported in `cordex::prelude`) with `push`, `push_line`, `push_safe`, `push_bold[_safe]`, `push_italic[_safe]`, `push_underline[_safe]`, `push_strike[_safe]`, `push_spoiler[_safe]`, `push_mono[_safe]`, `push_quote_line`, `push_codeblock`, `push_named_link[_safe]`, `mention`, `push_timestamp`, `build` and `into_string`.
  - **Built-in graceful shutdown:** `Bot::run_until_ctrl_c()` (handles `Ctrl+C` and Unix `SIGTERM`) and `Bot::run_until(future)` shut down all shards cleanly.
  - **Pagination & collector ergonomics:** `CommandContext::paginate(pages, timeout)` renders `◀` / `1 / N` / `▶` buttons with an author-only guard, sliding idle timeout, and automatic button disabling on timeout; `ButtonClicks::idle_timeout(duration)` and `ButtonClicks::stop()` added.
  - **Scoped audit-log reasons & shortcuts:** `Client::with_reason(...)` returns a cheap scoped clone that attaches `X-Audit-Log-Reason` to every request; `Client::forward_message(...)`, `Context::forward_message(...)`, `Context::send(channel_id, content)`, `Context::dm(user_id, content)`, `MessageContext::react(...)` and `MessageContext::delete()` added.
  - **Presence, typing, history, links & cooldowns:** `Bot::presence(...)` and `Context::set_presence(...)`; RAII `Typing` guard via `Client::start_typing` / `Context::start_typing`; `Client::messages_iter(channel_id)` (`MessageHistory` async paginator); `parse_message_link` / `MessageLink` in `cordex-model`; and thread-safe `Cooldowns<K>` in `cordex::bot`.
  - **Command builder & option shortcuts:** optional helpers `Bot::slash`, `Bot::user_command`, `Bot::message_command`; direct option builders on `Command` (`.string`, `.required_string`, `.integer`, `.required_integer`, `.number`, `.required_number`, `.boolean`, `.required_boolean`, `.user_option`, `.required_user`, `.role`, `.required_role`, `.channel`, `.required_channel`, `.attachment`, `.required_attachment`); and optional getters (`ctx.string_opt`, `ctx.integer_opt`, `ctx.number_opt`, `ctx.user_opt`, `ctx.member_opt`, `ctx.role_opt`, `ctx.channel_opt`, `ctx.attachment_opt`) directly on `CommandContext`.

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
