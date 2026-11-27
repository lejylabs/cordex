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

- **Instant testing on a dev server:** use `.guild_commands(guild_id)` (guild commands update immediately, unlike global commands).
- **External deployment:** call `.register_commands(false)` if a separate deploy script or another process manages command registration.
- **Modular bots:** group related commands, component/modal handlers, events, and module-scoped `before_command` guards in a `BotModule` and attach it with `Bot::mount(module)`.
- **Subcommands:** inspect the invoked subcommand with `ctx.subcommand()`, `ctx.subcommand_group()`, `ctx.subcommand_name()`, and `ctx.subcommand_path()`.

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

## Pre-Command Hooks, Cooldowns, Permissions & Error Handling

- **`.before_command(|ctx| ...)`** runs before every command handler (chainable; module-scoped when added on a `BotModule`). Return `Ok(false)` to block execution.
- **`.command_with_cooldown(cmd, duration, CooldownScope::User, handler)`** enforces per-command cooldowns (`User`, `Channel`, `Guild`, or `Global`) with automatic ephemeral notices (customizable via `.cooldown_message(...)`). For custom keys, `cordex::bot::Cooldowns<K>` is also available.
- **`ctx.require_user_perm(perm)?` / `ctx.require_bot_perm(perm)?`** return `Error::MissingPermission` when a required permission is absent.
- **Additional interaction guards:** `ctx.require_guild()?` (returns `GuildId`), `ctx.require_nsfw()?`, `ctx.require_role(role_id)?`, `ctx.require_entitlement(sku_id)?`, and `ctx.require_owner(&owners)?` short-circuit with typed `cordex::Error` variants (`GuildOnly`, `NsfwOnly`, `MissingRole`, `MissingEntitlement`, `OwnerOnly`).
- **`.on_error(|ctx, err| ...)`** catches every `Err` returned by any handler in one central place.
