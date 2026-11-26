# Changelog

Release notes for `cordex`, organized by version. The project follows [Semantic Versioning](https://semver.org/); before `1.0`, a minor release (`0.x`) may break the API while patch releases stay backwards-compatible.

| Version | Date | Highlights |
| --- | --- | --- |
| [Unreleased](changelog/unreleased.md) | — | — |
| [0.2.1](changelog/0.2.1.md) | 2026-11-26 | `cordex-macros`, `cordex-axum`, opt-in `postgres-types`/`rusqlite`/`redis`/`mysql`/`simd-json`, `StringInterner`, `CacheConfig::compact()`, TCP/HTTP2 keepalive & UDS transport |
| [0.2.0](changelog/0.2.0.md) | 2026-11-26 | `cordex-test` offline harness & `.jsonl` gateway replay, `around_command` middleware, signed `CustomId`, Components V2 `LayoutBuilder`, `split_message`, `SessionStore`, priority rate limiter, cache eviction & write-through, `health_snapshot()`, perf & bug hardening |
| [0.1.9](changelog/0.1.9.md) | 2026-11-25 | `cordex-migrate` CLI + full migration guide, `BotModule`, prefix/pattern routing, `confirm`/`select`/`prompt`/`ask_modal`, command cooldowns, `Embed` presets, ID aliases, `Bot::from_env`, `EmbedAudio`, `parse_invite_url` |
| [0.1.8](changelog/0.1.8.md) | 2026-11-24 | `PreviousState` diffs, role-hierarchy `can_moderate`, cache-first `Context` helpers, `shard_range`, prefix component routing |
| [0.1.7](changelog/0.1.7.md) | 2026-11-24 | New `cordex-voice` & `cordex-lavalink` crates |
| [0.1.6](changelog/0.1.6.md) | 2026-11-24 | `guild_members_with_ids`, `guild_voice_states`, `permission_overwrites` builders |
| [0.1.5](changelog/0.1.5.md) | 2026-11-24 | Attachment metadata on edits, `.clear_attachments()` / `.clear_embeds()` / `.clear_components()` |
| [0.1.4](changelog/0.1.4.md) | 2026-11-24 | Full `cordex-cache` cascade cleanups & reaction caching |
| [0.1.3](changelog/0.1.3.md) | 2026-11-23 | Optional `zstd-stream`, `Bot::data`, `on_error`, `MessageBuilder`, pagination, graceful shutdown |
| [0.1.2](changelog/0.1.2.md) | 2026-11-23 | HTTP/2 by default, optional `metrics` feature, `Box<[T]>` memory pass, zero-copy `zlib-stream` |
| [0.1.1](changelog/0.1.1.md) | 2026-11-23 | Per-resource `CachePolicy` (`Off`, `Lazy`, `Eager`) |
| [0.1.0](changelog/0.1.0.md) | 2026-11-23 | First regular release: 100% measured Discord v10 bot API coverage |
