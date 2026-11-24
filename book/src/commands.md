# Commands & Interactions

`cordex` supports all Discord application command types — **slash commands**, **user context-menu commands**, and **message context-menu commands** — along with typed option builders, autocomplete, permissions, and middleware hooks.

## Registering Commands

Use the shorthand helpers for simple commands, or `Command::slash` / `Command::user` / `Command::message` when you need options or permissions:

```rust,no_run
use cordex::prelude::*;

async fn run(token: String) -> cordex::Result<()> {
    Bot::new(token)
        // Shorthand for an optionless slash command
        .slash("ping", "Check if the bot is alive", |ctx: CommandContext| async move {
            ctx.reply("Pong!").await
        })
        // Slash command with typed option shortcuts
        .command(
            Command::slash("ban", "Ban a member")
                .required_user("target", "Member to ban")
                .string("reason", "Audit log reason")
                .default_member_permissions(Permissions::BAN_MEMBERS),
            |ctx: CommandContext| async move {
                let target = ctx.user_option("target")?;
                let reason = ctx.string_opt("reason").unwrap_or("No reason given");
                ctx.reply_ephemeral(format!("Would ban {} ({reason})", target.mention()))
                    .await
            },
        )
        // Right-click a user
        .user_command("User Info", |ctx: CommandContext| async move {
            ctx.reply_ephemeral("Showing user info…").await
        })
        // Right-click a message
        .message_command("Bookmark", |ctx: CommandContext| async move {
            ctx.reply_ephemeral("Bookmarked!").await
        })
        .run_until_ctrl_c()
        .await
}
```

### Option Builders & Getters

`Command` provides direct option builders for every Discord option type:

| Builder (optional) | Builder (required) | Getter on `CommandContext` |
|---|---|---|
| `.string(name, desc)` | `.required_string(name, desc)` | `ctx.string_opt(name)` / `ctx.string_option(name)?` |
| `.integer(name, desc)` | `.required_integer(name, desc)` | `ctx.integer_opt(name)` / `ctx.integer_option(name)?` |
| `.number(name, desc)` | `.required_number(name, desc)` | `ctx.number_opt(name)` / `ctx.number_option(name)?` |
| `.boolean(name, desc)` | `.required_boolean(name, desc)` | `ctx.boolean_opt(name)` / `ctx.boolean_option(name)?` |
| `.user_option(name, desc)` | `.required_user(name, desc)` | `ctx.user_opt(name)` / `ctx.user_option(name)?` |
| `.role(name, desc)` | `.required_role(name, desc)` | `ctx.role_opt(name)` / `ctx.role_option(name)?` |
| `.channel(name, desc)` | `.required_channel(name, desc)` | `ctx.channel_opt(name)` / `ctx.channel_option(name)?` |
| `.attachment(name, desc)` | `.required_attachment(name, desc)` | `ctx.attachment_opt(name)` / `ctx.attachment_option(name)?` |

For advanced options (min/max values, choices, channel types, autocomplete), pass a `CommandOption` to `.option(...)`.

## Command Sync & Guild-Scoped Commands

By default, `Bot` registers commands globally on the first `READY` — and skips the overwrite if Discord's registered commands already match your local definitions.

- **Instant testing on a dev server:** use `.guild_command(guild_id, command, handler)` (guild commands update immediately, unlike global commands).
- **External deployment:** call `.register_commands(false)` if a separate deploy script or another process manages command registration.

## Auto-Defer & Ephemeral Replies

Discord requires an initial response within **3 seconds**. `Bot` automatically defers an interaction after **2.5 seconds** if your handler hasn't responded yet, so slow database or HTTP calls won't fail with "Unknown interaction".

You can also defer explicitly:

```rust,no_run
use cordex::prelude::*;

async fn slow_command(ctx: CommandContext) -> cordex::Result<()> {
    ctx.defer_ephemeral().await?;
    // ... do slow work ...
    ctx.edit("Done!").await?;
    Ok(())
}
```

When editing messages, interaction responses, or webhook messages via `cordex-http` / `cordex-model`, you can explicitly remove existing components, embeds, or attachments with `.components(vec![])`, `.clear_embeds()`, and `.clear_attachments()` (for example `InteractionResponse::update_message("Closed.").clear_components()`).

## Pre-Command Hooks, Cooldowns & Error Handling

- **`.before_command(|ctx| ...)`** runs before every command handler. Return `Ok(false)` to block execution (e.g., for permission checks, maintenance mode, or cooldowns).
- **`cordex::bot::Cooldowns<K>`** is a built-in, thread-safe cooldown tracker you can store in `.data(...)`.
- **`.on_error(|ctx, err| ...)`** catches every `Err` returned by any handler in one central place.
