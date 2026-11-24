//! Public data types for voice signaling and Lavalink v4 integration.

use std::fmt;

use cordex_model::id::Id;
use cordex_model::id::marker::{ChannelMarker, GuildMarker};
use serde::{Deserialize, Serialize};

/// Options passed when joining or updating a voice channel.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct VoiceOptions {
    /// Whether the bot should join self-muted.
    pub self_mute: bool,
    /// Whether the bot should join self-deafened.
    pub self_deaf: bool,
}

impl VoiceOptions {
    /// Default options (`self_mute: false`, `self_deaf: false`).
    #[must_use]
    pub const fn new() -> Self {
        Self {
            self_mute: false,
            self_deaf: false,
        }
    }

    /// Set whether the bot is self-muted.
    #[must_use]
    pub const fn self_mute(mut self, self_mute: bool) -> Self {
        self.self_mute = self_mute;
        self
    }

    /// Set whether the bot is self-deafened.
    #[must_use]
    pub const fn self_deaf(mut self, self_deaf: bool) -> Self {
        self.self_deaf = self_deaf;
        self
    }
}

/// Complete, correlated voice connection credentials for a guild.
///
/// Produced once both `VOICE_STATE_UPDATE` (for the bot user) and
/// `VOICE_SERVER_UPDATE` (with a non-null `endpoint`) have been received.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct VoiceConnectionInfo {
    /// Guild this voice connection belongs to.
    pub guild_id: Id<GuildMarker>,
    /// Voice channel the bot is connected to.
    pub channel_id: Id<ChannelMarker>,
    /// Voice session ID from `VOICE_STATE_UPDATE`.
    pub session_id: Box<str>,
    /// Voice connection token from `VOICE_SERVER_UPDATE`.
    pub token: Box<str>,
    /// Voice server host from `VOICE_SERVER_UPDATE`.
    pub endpoint: Box<str>,
    /// Whether the bot is self-muted.
    pub self_mute: bool,
    /// Whether the bot is self-deafened.
    pub self_deaf: bool,
}

impl VoiceConnectionInfo {
    /// Build a **Lavalink v4** `voice` object (`token`, `endpoint`, `sessionId`,
    /// `channelId`) ready to be serialized in `PATCH /v4/sessions/{id}/players/{guildId}`.
    #[must_use]
    pub fn to_lavalink(&self) -> LavalinkVoiceState {
        LavalinkVoiceState {
            token: self.token.clone(),
            endpoint: self.endpoint.clone(),
            session_id: self.session_id.clone(),
            channel_id: self.channel_id,
        }
    }
}

impl fmt::Debug for VoiceConnectionInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VoiceConnectionInfo")
            .field("guild_id", &self.guild_id)
            .field("channel_id", &self.channel_id)
            .field("session_id", &self.session_id)
            .field("token", &"***")
            .field("endpoint", &self.endpoint)
            .field("self_mute", &self.self_mute)
            .field("self_deaf", &self.self_deaf)
            .finish()
    }
}

/// Lavalink v4 voice state payload for `PATCH /v4/sessions/{sessionId}/players/{guildId}`.
///
/// Serializes with camelCase field names (`token`, `endpoint`, `sessionId`, `channelId`).
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LavalinkVoiceState {
    /// Voice connection token.
    pub token: Box<str>,
    /// Voice server endpoint host.
    pub endpoint: Box<str>,
    /// Voice session ID.
    pub session_id: Box<str>,
    /// Connected voice channel ID.
    pub channel_id: Id<ChannelMarker>,
}

impl fmt::Debug for LavalinkVoiceState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LavalinkVoiceState")
            .field("token", &"***")
            .field("endpoint", &self.endpoint)
            .field("session_id", &self.session_id)
            .field("channel_id", &self.channel_id)
            .finish()
    }
}

impl From<&VoiceConnectionInfo> for LavalinkVoiceState {
    fn from(info: &VoiceConnectionInfo) -> Self {
        info.to_lavalink()
    }
}

/// Lifecycle events emitted by [`crate::VoiceManager`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum VoiceEvent {
    /// Both `VOICE_STATE_UPDATE` and `VOICE_SERVER_UPDATE` have arrived for a
    /// guild and a connection is now ready.
    Connected(VoiceConnectionInfo),
    /// Discord sent a new `VOICE_SERVER_UPDATE` for an active voice connection
    /// (e.g. voice server migration / reconnect).
    ServerUpdated(VoiceConnectionInfo),
    /// The bot's voice state changed while remaining connected (e.g. moved
    /// channels or mute/deaf toggled).
    StateUpdated(VoiceConnectionInfo),
    /// Discord sent `VOICE_SERVER_UPDATE` with `endpoint: null`, indicating
    /// temporary voice server reallocation.
    EndpointReallocating {
        /// Guild whose voice server is reallocating.
        guild_id: Id<GuildMarker>,
    },
    /// The bot disconnected from voice in `guild_id` (`channel_id: null`).
    Disconnected {
        /// Guild where the bot disconnected.
        guild_id: Id<GuildMarker>,
    },
}
