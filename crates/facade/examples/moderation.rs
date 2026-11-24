//! Moderation command example: permission checks, role hierarchy, optional
//! options, and audit-log reasons.
//!
//! Run with:
//! `DISCORD_TOKEN=... cargo run --example moderation --features cache`

use cordex::cache::{CacheConfig, ResourceType};
use cordex::prelude::*;

#[tokio::main]
async fn main() -> cordex::Result<()> {
    let Ok(token) = std::env::var("DISCORD_TOKEN") else {
        eprintln!("set DISCORD_TOKEN");
        return Ok(());
    };

    let kick = Command::slash("kick", "Kick a member from the server")
        .option(CommandOption::user("user", "Member to kick").required())
        .option(CommandOption::string("reason", "Optional audit log reason"));

    let cache = CacheConfig::new().resources(
        ResourceType::GUILDS | ResourceType::MEMBERS | ResourceType::ROLES | ResourceType::USERS,
    );

    Bot::new(token)
        .cache(cache)
        .command(kick, |ctx: CommandContext| async move {
            let Some(guild_id) = ctx.guild_id() else {
                return ctx
                    .reply_ephemeral("This command only works in a server.")
                    .await;
            };
            if !ctx.user_can(Permissions::KICK_MEMBERS) {
                return ctx
                    .reply_ephemeral("You need the **Kick Members** permission.")
                    .await;
            }
            if !ctx.bot_can(Permissions::KICK_MEMBERS) {
                return ctx
                    .reply_ephemeral("I am missing the **Kick Members** permission.")
                    .await;
            }

            let target = ctx.user_option("user")?;
            let reason = ctx
                .string_option_opt("reason")?
                .unwrap_or("No reason provided");
            let moderator_id = ctx.user().map(|u| u.id);

            if let (Some(cache), Some(mod_id)) = (ctx.cache(), moderator_id)
                && matches!(cache.can_moderate(guild_id, mod_id, target.id), Ok(false))
            {
                return ctx
                    .reply_ephemeral("You cannot moderate a member with an equal or higher role.")
                    .await;
            }

            ctx.http()
                .remove_guild_member(guild_id, target.id)
                .reason(reason)?
                .await?;

            ctx.reply_ephemeral(format!("Kicked **{}** ({reason}).", target.username))
                .await
        })
        .run_until_ctrl_c()
        .await
}
