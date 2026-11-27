# Voice Signaling & Lavalink v4 (`cordex-voice`, `cordex-lavalink`)

`cordex` offers two optional, 100% pure-Rust crates for voice without any C/FFI, Opus, or FFmpeg dependencies:

- **`cordex-voice`** (`feature = "voice"`): Discord voice signaling and session coordination (`VOICE_STATE_UPDATE` + `VOICE_SERVER_UPDATE` correlation, `VoiceManager`, `LavalinkVoiceState`).
- **`cordex-lavalink`** (`feature = "lavalink"`, enables `voice`): Lavalink v4 WebSocket + REST client with an automatic `VoiceManager` bridge.

Both features are **off by default**, so bots that do not use voice stay lean.

## Enabling Voice & Lavalink

```toml
[dependencies]
cordex = { version = "0.2.3", features = ["lavalink"] }
# Or only signaling (for custom backends):
# cordex = { version = "0.2.3", features = ["voice"] }
```

## Using `Bot::voice()`

Calling `.voice()` on `Bot` automatically adds `Intents::GUILD_VOICE_STATES`, parses `VOICE_STATE_UPDATE` and `VOICE_SERVER_UPDATE`, feeds them into the internal `VoiceManager`, and exposes `ctx.voice()` in every handler:

```rust,no_run
use std::time::Duration;
use cordex::prelude::*;

async fn run(token: String) -> cordex::Result<()> {
    Bot::new(token)
        .voice()
        .command(
            Command::slash("join", "Join a voice channel")
                .option(CommandOption::channel("channel", "Voice channel").required()),
            |ctx: CommandContext| async move {
                let guild_id = ctx.guild_id().expect("guild command");
                let channel_id = ctx.options().channel("channel")?.id;
                let voice = ctx.voice().expect("voice enabled");

                voice
                    .join_with(guild_id, channel_id, VoiceOptions::new().self_deaf(true))
                    .await?;

                let info = voice
                    .wait_for_connection(guild_id, Duration::from_secs(10))
                    .await?;

                ctx.reply_ephemeral(format!("Connected to `{}`!", info.endpoint))
                    .await
            },
        )
        .run_until_ctrl_c()
        .await
}
```

## Connecting to Lavalink v4 (`cordex-lavalink`)

`LavalinkClient::attach_voice(&voice)` bridges `VoiceManager` directly to your Lavalink node — whenever Discord sends `Connected`, `ServerUpdated` (region move / reconnect), or `Disconnected`, Lavalink is updated automatically:

```rust,no_run
use std::time::Duration;
use cordex::lavalink::{LavalinkClient, LoadResult, NodeConfig};
use cordex::prelude::*;

async fn play_track(
    voice: &VoiceManager,
    bot_user_id: Id<cordex::model::id::marker::UserMarker>,
    guild_id: Id<cordex::model::id::marker::GuildMarker>,
    channel_id: Id<cordex::model::id::marker::ChannelMarker>,
) -> cordex::Result<()> {
    let lavalink = LavalinkClient::connect(NodeConfig::new(
        "127.0.0.1",
        2333,
        "youshallnotpass",
        bot_user_id,
    ))?;
    lavalink.wait_ready(Duration::from_secs(10)).await?;
    let _bridge = lavalink.attach_voice(voice);

    voice
        .join_with(guild_id, channel_id, VoiceOptions::new().self_deaf(true))
        .await?;
    let _info = voice
        .wait_for_connection(guild_id, Duration::from_secs(10))
        .await?;

    if let LoadResult::Search(tracks) = lavalink.load_tracks("ytsearch:never gonna give you up").await?
        && let Some(first) = tracks.first()
    {
        lavalink.play(guild_id, first.encoded.clone()).await?;
    }
    Ok(())
}
```

## Lifecycle Events

Subscribe to `voice.subscribe()` to react to raw voice signaling events across all guilds:

- `VoiceEvent::Connected(info)` — initial handshake completed.
- `VoiceEvent::ServerUpdated(info)` — Discord migrated the guild to a new voice server.
- `VoiceEvent::StateUpdated(info)` — channel move or mute/deaf change while still connected.
- `VoiceEvent::EndpointReallocating { guild_id }` — temporary `endpoint: null` while Discord reallocates.
- `VoiceEvent::Disconnected { guild_id }` — bot left or was disconnected (`channel_id: null`).
