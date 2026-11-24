# `cordex-voice`

Pure-Rust Discord voice **signaling and session coordination** for `cordex`.

`cordex-voice` has no C/FFI, Opus, or audio-transport dependencies. It handles:

- Sending Opcode 4 (`VOICE_STATE_UPDATE`) commands via the [`VoiceGateway`] trait (implemented out of the box for `cordex-gateway`'s `Shard` and `ShardManager` behind the default `gateway` feature).
- Tracking per-guild voice state (`channel_id`, `session_id`, `self_mute`, `self_deaf`) and voice server details (`token`, `endpoint`).
- Correlating `VOICE_STATE_UPDATE` and `VOICE_SERVER_UPDATE` in **either order** into a complete [`VoiceConnectionInfo`].
- Waiting asynchronously for a guild's voice handshake (`wait_for_connection`) and subscribing to live updates (`subscribe`, `watch`), including server reallocations, channel moves, and disconnects.
- Serializing a ready-to-send **Lavalink v4** voice payload (`LavalinkVoiceState`).

## Quick Example

```rust,no_run
use std::sync::Arc;
use std::time::Duration;

use cordex_model::gateway::Event;
use cordex_model::id::Id;
use cordex_voice::{VoiceManager, VoiceOptions};

async fn example(
    voice: Arc<VoiceManager>,
    guild_id: Id<cordex_model::id::marker::GuildMarker>,
    channel_id: Id<cordex_model::id::marker::ChannelMarker>,
) -> Result<(), cordex_voice::VoiceError> {
    // 1. In your gateway event loop, forward every event:
    //    voice.update_from_event(&event);
    //    (This also auto-detects the bot's user ID from `Event::Ready`.)

    // 2. Join a voice channel (e.g. self-deafened for a music bot):
    voice
        .join_with(guild_id, channel_id, VoiceOptions::new().self_deaf(true))
        .await?;

    // 3. Wait until both VOICE_STATE_UPDATE and VOICE_SERVER_UPDATE arrive:
    let info = voice
        .wait_for_connection(guild_id, Duration::from_secs(10))
        .await?;

    // 4. Send to Lavalink v4 (`PATCH /v4/sessions/{id}/players/{guildId}`):
    let lavalink_payload = info.to_lavalink();
    let _json = serde_json::to_string(&lavalink_payload);

    // 5. Leave when done:
    voice.leave(guild_id).await?;
    Ok(())
}
```
