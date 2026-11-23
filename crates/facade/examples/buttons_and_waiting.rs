//! Buttons answered inline: wait for one click (`wait_for_button`) or
//! collect clicks for a while (`button_clicks`).
//!
//! `/confirm` asks yes/no and waits 30 s for the answer; `/poll` counts
//! votes for 60 s and then shows the result.
//!
//! ```text
//! DISCORD_TOKEN=... cargo run -p cordex --example buttons_and_waiting
//! ```

use std::collections::HashSet;
use std::time::Duration;

use cordex::prelude::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    Bot::new(std::env::var("DISCORD_TOKEN")?)
        .command(
            Command::slash("confirm", "Asks before doing something"),
            |ctx: CommandContext| async move {
                let row =
                    ActionRow::new([Button::success("yes", "Yes"), Button::danger("no", "No")]);
                ctx.defer().await?;
                let question = Reply::new("Are you sure?").components(vec![row.into()]);
                let message = ctx.edit(question).await?;
                match ctx
                    .wait_for_button(message.id, Duration::from_secs(30))
                    .await
                {
                    Some(click) if click.custom_id() == "yes" => click.update("Done.").await,
                    Some(click) => click.update("Cancelled.").await,
                    None => ctx.edit("No answer, cancelled.").await.map(|_| ()),
                }
            },
        )
        .command(
            Command::slash("poll", "Pizza or pasta? (60 seconds)"),
            |ctx: CommandContext| async move {
                let row = ActionRow::new([
                    Button::primary("pizza", "Pizza"),
                    Button::primary("pasta", "Pasta"),
                ]);
                ctx.defer().await?;
                let poll = Reply::new("Pizza or pasta?").components(vec![row.into()]);
                let message = ctx.edit(poll).await?;

                let (mut pizza, mut pasta, mut voters) = (0, 0, HashSet::new());
                let mut clicks = ctx.button_clicks(message.id, Duration::from_secs(60));
                while let Some(click) = clicks.next().await {
                    let Some(user) = click.user().map(|u| u.id) else {
                        continue;
                    };
                    if !voters.insert(user) {
                        click.reply_ephemeral("You already voted.").await?;
                        continue;
                    }
                    if click.custom_id() == "pizza" {
                        pizza += 1;
                    } else {
                        pasta += 1;
                    }
                    click.reply_ephemeral("Vote counted.").await?;
                }
                ctx.edit(
                    Reply::new(format!("Result: pizza {pizza}, pasta {pasta}")).components(vec![]),
                )
                .await
                .map(|_| ())
            },
        )
        .run()
        .await?;
    Ok(())
}
