//! The optional cache: only guilds, channels, roles and members are kept,
//! and `/perms` computes a user's permissions from it without a REST call.
//!
//! Needs the `GUILD_MEMBERS` privileged intent enabled in the developer
//! portal (members of the guild).
//!
//! ```text
//! DISCORD_TOKEN=... cargo run -p cordex --example cache_and_permissions --features cache
//! ```

use cordex::prelude::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cache = CacheConfig::new()
        .resources(
            ResourceType::GUILDS
                | ResourceType::CHANNELS
                | ResourceType::THREADS
                | ResourceType::ROLES
                | ResourceType::MEMBERS,
        )
        .parts(CacheParts::MEMBER_ROLES | CacheParts::CHANNEL_OVERWRITES);

    Bot::new(std::env::var("DISCORD_TOKEN")?)
        .intents(Intents::GUILD_MEMBERS)
        .cache(cache)
        .command(
            Command::slash("perms", "Your permissions in this channel"),
            |ctx: CommandContext| async move {
                let (Some(guild), Some(channel), Some(user)) =
                    (ctx.guild_id(), ctx.channel_id(), ctx.user().map(|u| u.id))
                else {
                    return ctx.reply_ephemeral("Use this in a server.").await;
                };
                let Some(cache) = ctx.cache() else {
                    return ctx.reply_ephemeral("No cache.").await;
                };
                let text = match cache.permissions(guild, user, channel) {
                    Ok(perms) => format!(
                        "Send messages: {}\nManage messages: {}\nAll: `{perms:?}`",
                        perms.contains(Permissions::SEND_MESSAGES),
                        perms.contains(Permissions::MANAGE_MESSAGES),
                    ),
                    Err(missing) => format!("Not possible yet: {missing}"),
                };
                ctx.reply_ephemeral(text).await
            },
        )
        .run()
        .await?;
    Ok(())
}
