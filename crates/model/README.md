# cordex-model

[![crates.io][crates badge]][crates link] [![docs.rs][docs badge]][docs link] [![guide](https://img.shields.io/badge/guide-cordex_docs-1f6feb.svg?style=for-the-badge)](https://lejylabs.github.io/cordex/) [![discord][discord badge]][discord link] ![rust 1.95+][rust badge]

Discord API v10 models for [cordex]: every object, event, opcode, enum value
and flag of the official bot API, plus helpers that need no network.

- **Typed everything**: snowflakes (`Id<UserMarker>`, `Id<GuildMarker>`, … plus
  short aliases `UserId`, `GuildId`, `ChannelId`, `RoleId`, `MessageId`, … that
  cannot be mixed up), flags (`Permissions`, `Intents`, `MessageFlags`, …),
  timestamps, all REST objects and gateway events.
- **Never fails on new Discord data**: unknown fields are ignored, unknown
  enum values and events are kept (`Unknown(..)`), unknown flag bits survive
  a round trip.
- **Small and fast**: large payloads are boxed, `Event` stays at most 48 bytes,
  `GUILD_CREATE` is read without buffering it twice.
- **Helpers**: `MessageBuilder` with Markdown escaping, codeblock-aware
  `split_message`, `escape_markdown`, `escape_mentions`, `Locale`, embed
  builder with Discord's limits (`Embed::success`/`error`/`info`/`warn` +
  `colors`), `AutoModerationAction` and `TriggerMetadata` builders, CDN URLs,
  mentions and timestamp markdown, mention parsing, permission calculator,
  webhook, invite and message URL parsing.
- **Opt-in features**: `postgres-types`, `rusqlite`, `redis`, `mysql`
  (`mysql_common`) type conversions for `Id<T>`, `Permissions`, `Timestamp`,
  and `ImageHash`; plus `simd-json` for SIMD-accelerated JSON parsing.
- **No async, no network**: usable in any project (including `wasm32`).

Most bots use the facade crate [`cordex`][facade], which re-exports this crate.

## Examples

```rust
use cordex_model::gateway::Event;
use cordex_model::id::Id;
use cordex_model::id::marker::UserMarker;
use cordex_model::mention::{Mentionable, ParsedMention, parse_mentions};
use cordex_model::message::{Embed, EmbedError};
use cordex_model::timestamp::TimestampStyle;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Snowflakes, mentions and timestamps.
    let user: Id<UserMarker> = "80351110224678912".parse()?;
    assert_eq!(user.mention(), "<@80351110224678912>");
    let created = user.created_at().markdown(TimestampStyle::Relative);
    assert!(created.starts_with("<t:"));
    let found = parse_mentions("hi <@80351110224678912>");
    assert_eq!(found, vec![ParsedMention::User(user)]);

    // Embeds check Discord's limits locally.
    let embed = Embed::new().title("Hello")?.field("Field", "Value", true)?;
    assert_eq!(embed.fields.len(), 1);
    assert!(matches!(
        Embed::new().title("x".repeat(300)),
        Err(EmbedError::TooLong { .. })
    ));

    // Gateway events from a dispatch name and its JSON.
    let data = serde_json::value::RawValue::from_string("{}".to_owned())?;
    let event = Event::from_dispatch("SOMETHING_NEW", Some(&data))?;
    assert!(matches!(event, Event::Unknown(_)));
    Ok(())
}
```

## License

Licensed under either of Apache License, Version 2.0 or MIT license at your
option. Questions or help: [cordex Discord server][discord link].

[cordex]: https://github.com/lejylabs/cordex
[facade]: https://crates.io/crates/cordex
[crates badge]: https://img.shields.io/crates/v/cordex-model.svg?style=for-the-badge&logo=rust
[crates link]: https://crates.io/crates/cordex-model
[docs badge]: https://img.shields.io/docsrs/cordex-model?style=for-the-badge&logo=docsdotrs
[docs link]: https://docs.rs/cordex-model
[discord badge]: https://img.shields.io/badge/discord-join-5865F2.svg?style=for-the-badge&logo=discord&logoColor=white
[discord link]: https://discord.gg/Xj9nRKyKNV
[rust badge]: https://img.shields.io/badge/rust-1.95+-93450a.svg?style=for-the-badge&logo=rust
