# Simple Layer (`Bot`)

`cordex::bot::Bot` handles the boilerplate for 90 % of Discord bots without hiding the underlying primitives:

- **Intents & event filtering:** Derived automatically from your registered handlers and cache config. Privileged intents (`GUILD_MEMBERS`, `GUILD_PRESENCES`, `MESSAGE_CONTENT`) are never enabled implicitly — opt in with `.intents(...)`.
- **Command registration:** Bulk-overwrites global (or guild-scoped) commands on the first `READY`, only when the local definitions actually differ from Discord's. Disable with `.register_commands(false)` if commands are deployed separately.
- **Auto-defer:** Interactions are automatically deferred after 2.5 s so you never hit Discord's 3-second timeout.
- **Error & panic containment:** Returning `Err` or panicking inside a handler is logged (and answered with an ephemeral error for interactions) without stopping the bot.

## Handlers

```rust,no_run
use cordex::prelude::*;

async fn run(token: String) -> cordex::Result<()> {
    Bot::new(token)
        .command(
            Command::slash("greet", "Greets a user")
                .option(CommandOption::user("user", "Who to greet").required()),
            |ctx: CommandContext| async move {
                let user = ctx.user_option("user")?;
                ctx.reply(format!("Hello {}!", user.mention())).await
            },
        )
        .on_button("confirm", |ctx: ComponentContext| async move {
            ctx.update("Confirmed!").await
        })
        .on_modal("feedback", |ctx: ModalContext| async move {
            let text = ctx.value("message").unwrap_or("");
            ctx.reply_ephemeral(format!("Thanks: {text}")).await
        })
        .on_message(|ctx: MessageContext| async move {
            if ctx.content() == "!ping" {
                ctx.reply("Pong!").await?;
            }
            Ok(())
        })
        .run()
        .await
}
```

## Inline waiting (`Standby`)

Wait for follow-up clicks or messages right inside a handler without maintaining global state maps:

```rust,no_run
use std::time::Duration;
use cordex::prelude::*;

async fn confirm(ctx: CommandContext) -> cordex::Result<()> {
    let row = ActionRow::new([
        Button::success("yes", "Yes"),
        Button::danger("no", "No"),
    ]);
    ctx.defer().await?;
    let msg = ctx
        .edit(Reply::new("Proceed?").components(vec![row.into()]))
        .await?;

    match ctx.wait_for_button(msg.id, Duration::from_secs(30)).await {
        Some(click) => click.update(format!("Picked {}", click.custom_id())).await,
        None => ctx.edit("Timed out.").await.map(|_| ()),
    }
}
```

For polls or custom flows, use `ctx.button_clicks(msg.id, timeout)` to iterate over all clicks until the timeout expires. For standard multi-page responses, use the built-in paginator:

```rust,no_run
use std::time::Duration;
use cordex::prelude::*;

async fn help(ctx: CommandContext) -> cordex::Result<()> {
    let pages = [Reply::new("Page 1"), Reply::new("Page 2"), Reply::new("Page 3")];
    ctx.paginate(pages, Duration::from_secs(60)).await
}
```

`paginate` renders `◀` / `1 / N` / `▶` buttons, restricts navigation to the invoking user, and disables the buttons once the timeout expires.

## Shared state, hooks & graceful shutdown

Attach arbitrary `Send + Sync + 'static` values to `Bot` with `.data(...)` and read them from any handler via `ctx.data::<T>()`. Use `.before_command(...)` for shared guards/cooldowns, `.on_error(...)` for centralized error reporting, and `.run_until_ctrl_c()` (or `.run_until(future)`) to shut down all shards cleanly:

```rust,no_run
use cordex::prelude::*;

struct Config {
    admin_only: bool,
}

async fn run(token: String) -> cordex::Result<()> {
    Bot::new(token)
        .data(Config { admin_only: false })
        .before_command(|ctx: CommandContext| async move {
            if ctx.data::<Config>().is_some_and(|c| c.admin_only) {
                ctx.reply_ephemeral("Maintenance mode.").await?;
                return Ok(false);
            }
            Ok(true)
        })
        .on_error(|_ctx, err| async move {
            eprintln!("handler failed: {err}");
        })
        .command(Command::slash("ping", "Replies with pong"), |ctx: CommandContext| async move {
            let text = MessageBuilder::new()
                .push_bold("Pong!")
                .push(" Hi ")
                .push_safe(ctx.user().map_or("there", |u| u.username.as_ref()))
                .build();
            ctx.reply(text).await
        })
        .run_until_ctrl_c()
        .await
}
```

For quick outbound messages outside an interaction reply, use `ctx.send(channel_id, "Hello!")` or `ctx.dm(user_id, "Check your DMs!")`.
