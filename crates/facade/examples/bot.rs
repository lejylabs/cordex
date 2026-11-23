//! A complete bot with the simple layer (`cordex::bot`).
//!
//! - `/ping`: replies with pong.
//! - `/greet user`: greets a user.
//! - `/color query`: autocomplete.
//! - `/vote`: buttons; presses arrive in `on_button("vote", …)`.
//! - `/feedback`: opens a modal; the submit arrives in `on_modal`.
//! - `!hi` in a channel: text reply (needs the `MESSAGE_CONTENT` intent
//!   enabled in the developer portal).
//!
//! ```text
//! DISCORD_TOKEN=... GUILD_ID=... cargo run -p cordex --example bot
//! ```
//!
//! `GUILD_ID` is optional: with it the commands show up in that guild
//! at once, without it they are registered globally.

use std::time::Duration;

use cordex::prelude::*;

const COLORS: [&str; 6] = ["red", "orange", "yellow", "green", "blue", "violet"];

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();
    let token = std::env::var("DISCORD_TOKEN").map_err(|_| "DISCORD_TOKEN is not set")?;

    let mut bot = Bot::new(token)
        .message_content()
        .on_ready(|ctx: ReadyContext| async move {
            println!(
                "online as {} in {} guilds",
                ctx.bot_user().username,
                ctx.guild_count()
            );
            Ok(())
        })
        .command(
            Command::slash("ping", "Replies with pong"),
            |ctx: CommandContext| async move { ctx.reply("Pong!").await },
        )
        .command(
            Command::slash("greet", "Greets a user")
                .option(CommandOption::user("user", "Who to greet").required()),
            |ctx: CommandContext| async move {
                let user = ctx.user_option("user")?;
                ctx.reply(format!("Hello {}!", user.mention())).await
            },
        )
        .command(
            Command::slash("color", "Pick a color").option(
                CommandOption::string("query", "Start typing")
                    .required()
                    .autocomplete(),
            ),
            |ctx: CommandContext| async move {
                // Slow work is fine: after 2.5 s the bot defers on its own.
                tokio::time::sleep(Duration::from_secs(3)).await;
                ctx.reply(format!("You picked {}", ctx.string_option("query")?))
                    .await
            },
        )
        .autocomplete("color", |ctx: AutocompleteContext| async move {
            let typed = ctx.focused_value().to_lowercase();
            let hits = COLORS
                .iter()
                .filter(|c| c.starts_with(&typed))
                .map(|c| Choice::string(*c, *c));
            ctx.suggest(hits).await
        })
        .command(
            Command::slash("vote", "Yes or no?"),
            |ctx: CommandContext| async move {
                let buttons = ActionRow::new([
                    Button::success("vote:yes", "Yes"),
                    Button::danger("vote:no", "No"),
                ]);
                ctx.reply(Reply::new("Cast your vote:").components(vec![buttons.into()]))
                    .await
            },
        )
        .on_button("vote", |ctx: ComponentContext| async move {
            let who = ctx.user().map_or("someone", |u| &*u.username);
            ctx.update(format!("{who} voted {}", ctx.custom_id_arg()))
                .await
        })
        .command(
            Command::slash("feedback", "Tell us something"),
            |ctx: CommandContext| async move {
                let field = Label::new(
                    "Your feedback",
                    TextInput::paragraph("text").max_length(500),
                );
                ctx.modal("feedback", "Feedback", vec![field.into()]).await
            },
        )
        .on_modal("feedback", |ctx: ModalContext| async move {
            let text = ctx.value("text").unwrap_or_default();
            ctx.reply_ephemeral(format!("Thanks! You wrote: {text}"))
                .await
        })
        .on_message(|ctx: MessageContext| async move {
            if ctx.content() == "!hi" {
                ctx.reply("Hi!").await?;
            }
            Ok(())
        });

    if let Ok(guild) = std::env::var("GUILD_ID") {
        let guild = guild
            .trim()
            .parse()
            .map_err(|_| format!("GUILD_ID must be a guild id (a number), got {guild:?}"))?;
        bot = bot.guild_commands(guild);
    }
    bot.run().await?;
    Ok(())
}
