# Changelog

Release notes for `cordex`, organized by version. The project follows [Semantic Versioning](https://semver.org/); before `1.0`, a minor release (`0.x`) may break the API while patch releases stay backwards-compatible.

| Version | Date | Highlights |
| --- | --- | --- |
| [Unreleased](changelog/unreleased.md) | — | `EmbedAudio` / `Embed.audio`, `parse_invite_url` & `ParseError::InviteUrl` |
| [0.1.8](changelog/0.1.8.md) | 2026-11-24 | `PreviousState` diffs, role-hierarchy `can_moderate`, cache-first `Context` helpers, `shard_range`, prefix component routing |
| [0.1.7](changelog/0.1.7.md) | 2026-11-24 | New `cordex-voice` & `cordex-lavalink` crates |
| [0.1.6](changelog/0.1.6.md) | 2026-11-24 | `guild_members_with_ids`, `guild_voice_states`, `permission_overwrites` builders |
| [0.1.5](changelog/0.1.5.md) | 2026-11-24 | Attachment metadata on edits, `.clear_attachments()` / `.clear_embeds()` / `.clear_components()` |
| [0.1.4](changelog/0.1.4.md) | 2026-11-24 | Full `cordex-cache` cascade cleanups & reaction caching |
| [0.1.3](changelog/0.1.3.md) | 2026-11-23 | Optional `zstd-stream`, `Bot::data`, `on_error`, `MessageBuilder`, pagination, graceful shutdown |
| [0.1.2](changelog/0.1.2.md) | 2026-11-23 | HTTP/2 by default, optional `metrics` feature, `Box<[T]>` memory pass, zero-copy `zlib-stream` |
| [0.1.1](changelog/0.1.1.md) | 2026-11-23 | Per-resource `CachePolicy` (`Off`, `Lazy`, `Eager`) |
| [0.1.0](changelog/0.1.0.md) | 2026-11-23 | First regular release: 100% measured Discord v10 bot API coverage |
