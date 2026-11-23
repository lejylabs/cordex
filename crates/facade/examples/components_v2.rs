//! Components V2 layout: a container with text, a section with a
//! thumbnail, a separator, an image gallery and buttons. cordex sets the
//! `IS_COMPONENTS_V2` message flag on its own.
//!
//! ```text
//! DISCORD_TOKEN=... cargo run -p cordex --example components_v2
//! ```

use cordex::model::component::{
    Container, MediaGallery, Section, Separator, TextDisplay, Thumbnail,
};
use cordex::prelude::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    Bot::new(std::env::var("DISCORD_TOKEN")?)
        .command(
            Command::slash("card", "Shows a Components V2 card"),
            |ctx: CommandContext| async move {
                let avatar = ctx
                    .user()
                    .map(|u| u.avatar_url().size(128).to_string())
                    .unwrap_or_default();
                let card = Container::new([
                    Component::from(TextDisplay::new("# Welcome")),
                    Section::new(
                        [TextDisplay::new(
                            "This card uses the new layout components.",
                        )],
                        Thumbnail::new(avatar).description("Your avatar"),
                    )
                    .into(),
                    Separator::new().into(),
                    MediaGallery::new(["https://cdn.discordapp.com/embed/avatars/0.png"]).into(),
                    ActionRow::new([
                        Button::primary("card:like", "Like"),
                        Button::link("https://github.com/lejylabs/cordex", "cordex"),
                    ])
                    .into(),
                ])
                .accent_color(0x0058_65F2);
                ctx.reply(Reply::layout(vec![card.into()])).await
            },
        )
        .on_button("card", |ctx: ComponentContext| async move {
            ctx.reply_ephemeral("Thanks!").await
        })
        .run()
        .await?;
    Ok(())
}
