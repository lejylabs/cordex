# Quickstart: Your First Bot

This guide takes you from a fresh Rust project to a running Discord bot with slash commands in about 5 minutes.

## 1. Create a Discord Application

1. Open the [Discord Developer Portal](https://discord.com/developers/applications) and click **New Application**.
2. Go to the **Bot** tab and click **Reset Token** to copy your bot token. Never commit this token to Git.
3. Under **Privileged Gateway Intents**, only enable what your bot actually needs (for slash commands alone, you don't need any privileged intents).
4. Go to **OAuth2 → URL Generator**, select `bot` and `applications.commands`, pick the permissions your bot needs, and open the generated URL to invite the bot to a test server.

## 2. Create the Rust Project

```bash
cargo new my-bot
cd my-bot
```

Add `cordex` and `tokio` to `Cargo.toml` (requires Rust 1.95+):

```toml
[dependencies]
cordex = "0.1.9"
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

*(Alternatively, scaffold a project directly with `cargo generate lejylabs/cordex-template`.)*

## 3. Write `src/main.rs`

```rust,no_run
use cordex::prelude::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    Bot::from_env()?
        .slash("ping", "Replies with Pong!", |ctx: CommandContext| async move {
            ctx.reply("Pong!").await
        })
        .command(
            Command::slash("say", "Repeats your text")
                .required_string("text", "What to say"),
            |ctx: CommandContext| async move {
                let text = ctx.string_option("text")?;
                ctx.reply_ephemeral(text).await
            },
        )
        .run_until_ctrl_c()
        .await?;

    Ok(())
}
```

## 4. Run Your Bot

```bash
# Linux / macOS
DISCORD_TOKEN="your-bot-token" cargo run

# Windows (PowerShell)
$env:DISCORD_TOKEN = "your-bot-token"
cargo run
```

On startup, `Bot` connects to the Discord Gateway, automatically syncs your slash commands (only when definitions changed), and shuts down cleanly when you press `Ctrl+C`.

## Next Steps

- Learn about handlers, shared state, hooks, and inline waiting in [Simple Layer (`Bot`)](simple-layer.md).
- Dive into subcommands, autocomplete, and cooldowns in [Commands & Interactions](commands.md).
- Enable opt-in caching in [Optional Cache](cache.md).
