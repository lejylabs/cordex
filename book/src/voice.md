# Voice Signaling (`cordex-voice`)

`cordex-voice` is an optional, pure-Rust crate for **Discord voice signaling and session coordination**. It has no C/FFI, Opus, or audio-transport dependencies, making it ideal for **Lavalink v4** or custom voice backends while keeping bots that do not use voice completely lean.

## Enabling Voice

Enable the `voice` feature on `cordex` (it is off by default):

```toml
[dependencies]
cordex = { version = "0.1", features = ["voice"] }
```

Make sure your gateway intents include `Intents::GUILD_VOICE_STATES` so Discord sends `VOICE_STATE_UPDATE` events.

## Connecting & Correlating Events

1. Wrap your `Shard` or `ShardManager` in an `Arc` and pass it to `VoiceManager::new`.
2. Forward every gateway `Event` to `voice.update_from_event(&event)` (this automatically records the bot's user ID from `Event::Ready` and correlates `VOICE_STATE_UPDATE` and `VOICE_SERVER_UPDATE` in either order).
3. Call `join` / `join_with`, then `wait_for_connection` to obtain a complete `VoiceConnectionInfo`.

```rust,no_run
use std::sync::Arc;
use std::time::Duration;

use cordex::prelude::*;

async fn join_voice(
    manager: Arc<ShardManager>,
    guild_id: Id<cordex::model::id::marker::GuildMarker>,
    channel_id: Id<cordex::model::id::marker::ChannelMarker>,
) -> cordex::Result<()> {
    let voice = Arc::new(VoiceManager::new(manager));

    // In your event loop:
    // voice.update_from_event(&event);

    voice
        .join_with(guild_id, channel_id, VoiceOptions::new().self_deaf(true))
        .await?;

    let info = voice
        .wait_for_connection(guild_id, Duration::from_secs(10))
        .await?;

    // Ready-to-serialize Lavalink v4 payload:
    // { "token": "...", "endpoint": "...", "sessionId": "...", "channelId": "..." }
    let lavalink = info.to_lavalink();
    let _body = serde_json::json!({ "voice": lavalink });

    Ok(())
}
```

## Handling Server Moves & Disconnects

Use `voice.subscribe()` to receive a `tokio::sync::broadcast::Receiver<VoiceEvent>` for all guilds:

- `VoiceEvent::Connected(info)` — initial handshake completed.
- `VoiceEvent::ServerUpdated(info)` — Discord migrated the guild to a new voice server; resend `info.to_lavalink()` to Lavalink.
- `VoiceEvent::StateUpdated(info)` — channel move or mute/deaf change while still connected.
- `VoiceEvent::EndpointReallocating { guild_id }` — temporary `endpoint: null` while Discord reallocates.
- `VoiceEvent::Disconnected { guild_id }` — bot left or was disconnected (`channel_id: null`).
