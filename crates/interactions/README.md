# cordex-interactions

[![crates.io][crates badge]][crates link] [![docs.rs][docs badge]][docs link] [![guide](https://img.shields.io/badge/guide-cordex_docs-1f6feb.svg?style=for-the-badge)](https://lejylabs.github.io/cordex/) [![discord][discord badge]][discord link] ![rust 1.95+][rust badge]

Discord interactions for [cordex]: application commands, responses and an
HTTP interaction endpoint.

- **Command builders** for slash, user and message commands with options,
  choices, localizations and permissions; validated locally with clear
  errors (`"command name 'Ping' must be lowercase"`).
- **`InteractionHandle`**: answer, defer, edit, follow up, open modals. It
  hides Discord's response types and the 3-second rule: auto-defer after
  2.5 s, `respond` after a defer edits the original, a second answer is an
  error instead of a Discord 400, the 15-minute token lifetime is checked.
- **Typed options** (`CommandOptions`): strings, numbers, users with their
  member, roles, channels, attachments, subcommands, autocomplete focus.
- **`Modal`** builder and `Reply` for messages, embeds and components.
- **HTTP interactions** (features `verify` and `server`): Ed25519 signature
  check for any web framework, or a standalone hyper endpoint with PING
  handling, body cap and timestamp check.

Most bots use the facade crate [`cordex`][facade], whose `Bot` routes
interactions to your handlers.

## Example

```rust,no_run
use cordex_http::Client;
use cordex_interactions::{Command, CommandOption, CommandOptions, InteractionHandle, Reply};
use cordex_model::interaction::{Interaction, InteractionData};

fn commands() -> Vec<Command> {
    vec![
        Command::slash("echo", "Repeats a text")
            .option(CommandOption::string("text", "What to repeat").required().max_length(200)),
    ]
}

async fn on_interaction(
    client: Client,
    interaction: &Interaction,
) -> Result<(), Box<dyn std::error::Error>> {
    let handle = InteractionHandle::new(client, interaction);
    if let Some(InteractionData::Command(data)) = &interaction.data {
        let options = CommandOptions::new(data);
        let text = options.string("text")?;
        handle.respond(Reply::new(text).ephemeral()).await?;
    }
    Ok(())
}
```

## Features

| Feature | Default | Content |
|---|---|---|
| `macros` | no | Optional `#[derive(CommandArgs)]` and `#[derive(CommandChoice)]` |
| `verify` | no | `PublicKey` and `verify_signature` (Ed25519) |
| `server` | no | `InteractionEndpoint`: standalone hyper endpoint (implies `verify`) |

## License

Licensed under either of Apache License, Version 2.0 or MIT license at your
option. Questions or help: [cordex Discord server][discord link].

[cordex]: https://github.com/lejylabs/cordex
[facade]: https://crates.io/crates/cordex
[crates badge]: https://img.shields.io/crates/v/cordex-interactions.svg?style=for-the-badge&logo=rust
[crates link]: https://crates.io/crates/cordex-interactions
[docs badge]: https://img.shields.io/docsrs/cordex-interactions?style=for-the-badge&logo=docsdotrs
[docs link]: https://docs.rs/cordex-interactions
[discord badge]: https://img.shields.io/badge/discord-join-5865F2.svg?style=for-the-badge&logo=discord&logoColor=white
[discord link]: https://discord.gg/Xj9nRKyKNV
[rust badge]: https://img.shields.io/badge/rust-1.95+-93450a.svg?style=for-the-badge&logo=rust
