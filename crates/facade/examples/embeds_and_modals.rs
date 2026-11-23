//! Embeds built with Discord's limits checked locally, and a modal with a
//! text field, a radio group and a checkbox.
//!
//! ```text
//! DISCORD_TOKEN=... cargo run -p cordex --example embeds_and_modals
//! ```

use cordex::model::component::{Checkbox, ChoiceOption, RadioGroup};
use cordex::prelude::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    Bot::new(std::env::var("DISCORD_TOKEN")?)
        .command(
            Command::slash("profile", "Shows a user")
                .option(CommandOption::user("user", "Who").required()),
            |ctx: CommandContext| async move {
                let user = ctx.user_option("user")?;
                let embed = Embed::new()
                    .title(user.username.as_ref())?
                    .description(format!("{} joined Discord.", user.mention()))?
                    .field(
                        "Account created",
                        user.id.created_at().markdown(TimestampStyle::Relative),
                        true,
                    )?
                    .field("ID", user.id.to_string(), true)?
                    .thumbnail(user.avatar_url().size(256).to_string())
                    .color(0x0058_65F2);
                ctx.reply(Reply::embed(embed)).await
            },
        )
        .command(
            Command::slash("feedback", "Tell us what you think"),
            |ctx: CommandContext| async move {
                let modal = Modal::new("feedback", "Feedback")
                    .paragraph("text", "Your feedback")
                    .component(Label::new(
                        "Rating",
                        RadioGroup::new(
                            "rating",
                            [
                                ChoiceOption::new("Great", "great"),
                                ChoiceOption::new("Okay", "okay").selected(),
                                ChoiceOption::new("Bad", "bad"),
                            ],
                        ),
                    ))
                    .component(Label::new("Contact me", Checkbox::new("contact")));
                ctx.modal(modal).await
            },
        )
        .on_modal("feedback", |ctx: ModalContext| async move {
            let text = ctx.value("text").unwrap_or_default();
            let rating = ctx.values("rating").first().map_or("-", |v| &**v);
            ctx.reply_ephemeral(format!("Thanks! Rating: {rating}\n> {text}"))
                .await
        })
        .run()
        .await?;
    Ok(())
}
