//! Voice signaling example with `Bot::voice()` and Lavalink v4 payload output.
//!
//! Run with:
//! `DISCORD_TOKEN=... cargo run -p cordex --example voice_lavalink --features voice`

use std::time::Duration;

use cordex::prelude::*;

#[tokio::main]
async fn main() -> cordex::Result<()> {
    tracing_subscriber::fmt::init();

    let Ok(token) = std::env::var("DISCORD_TOKEN") else {
        eprintln!("set DISCORD_TOKEN before running this example");
        return Ok(());
    };

    Bot::new(token)
        .voice()
        .command(
            Command::slash(
                "join",
                "Join a voice channel and print the Lavalink v4 payload",
            )
            .option(CommandOption::channel("channel", "Voice channel to join").required()),
            |ctx: CommandContext| async move {
                let Some(guild_id) = ctx.guild_id() else {
                    return ctx.reply_ephemeral("Use this command in a server.").await;
                };
                let channel_id = ctx.options().channel("channel")?.id;
                let Some(voice) = ctx.voice() else {
                    return ctx.reply_ephemeral("Voice is not enabled.").await;
                };

                voice
                    .join_with(guild_id, channel_id, VoiceOptions::new().self_deaf(true))
                    .await?;

                let info = voice
                    .wait_for_connection(guild_id, Duration::from_secs(10))
                    .await?;

                let lavalink = info.to_lavalink();
                ctx.reply_ephemeral(format!(
                    "Connected to `{}` (endpoint `{}`). Ready for Lavalink v4!",
                    lavalink.channel_id, lavalink.endpoint
                ))
                .await
            },
        )
        .command(
            Command::slash("leave", "Leave the current voice channel"),
            |ctx: CommandContext| async move {
                let Some(guild_id) = ctx.guild_id() else {
                    return ctx.reply_ephemeral("Use this command in a server.").await;
                };
                if let Some(voice) = ctx.voice() {
                    voice.leave(guild_id).await?;
                }
                ctx.reply_ephemeral("Left voice channel.").await
            },
        )
        .run_until_ctrl_c()
        .await
}
