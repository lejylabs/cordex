//! REST without a gateway connection: send a message with safe default
//! mentions, react, and post through a webhook URL.
//!
//! ```text
//! DISCORD_TOKEN=... CHANNEL_ID=... WEBHOOK_URL=... cargo run -p cordex --example http_only
//! ```

use cordex::model::id::marker::ChannelMarker;
use cordex::prelude::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Never ping @everyone or roles by accident: only users may be pinged.
    let client = Client::builder(std::env::var("DISCORD_TOKEN")?)
        .default_allowed_mentions(AllowedMentions::users_only())
        .build()?;

    let channel: Id<ChannelMarker> = std::env::var("CHANNEL_ID")?.parse()?;
    let message = client
        .create_message(channel)
        .content("Hello @everyone (this does not ping anyone)")?
        .send()
        .await?;
    client.create_reaction(channel, message.id, "👋").await?;
    println!("sent message {}", message.id);

    if let Ok(url) = std::env::var("WEBHOOK_URL") {
        let (id, token) = parse_webhook_url(&url)?;
        client
            .execute_webhook(id, &token)
            .content("Posted through a webhook")?
            .send()
            .await?;
        println!("posted through webhook {id}");
    }
    Ok(())
}
