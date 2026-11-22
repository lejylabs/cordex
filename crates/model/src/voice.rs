//! Voice state and regions (resources/voice.mdx). Voice *transport* is out
//! of scope (PLAN.md §1); these are the REST/gateway objects.

use serde::{Deserialize, Serialize};

use crate::guild::Member;
use crate::id::Id;
use crate::id::marker::{ChannelMarker, GuildMarker, UserMarker};
use crate::timestamp::Timestamp;

/// A user's voice connection status.
#[allow(clippy::struct_excessive_bools)] // Discord sends these as separate fields
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct VoiceState {
    /// Guild; omitted in some `GUILD_CREATE` voice states.
    pub guild_id: Option<Id<GuildMarker>>,
    /// Voice channel; `None` when the user has disconnected.
    pub channel_id: Option<Id<ChannelMarker>>,
    /// User.
    pub user_id: Id<UserMarker>,
    /// Member (guild voice states).
    pub member: Option<Box<Member>>,
    /// Voice session ID.
    pub session_id: Box<str>,
    /// Server-deafened.
    #[serde(default)]
    pub deaf: bool,
    /// Server-muted.
    #[serde(default)]
    pub mute: bool,
    /// Self-deafened.
    #[serde(default)]
    pub self_deaf: bool,
    /// Self-muted.
    #[serde(default)]
    pub self_mute: bool,
    /// Streaming via Go Live.
    #[serde(default)]
    pub self_stream: bool,
    /// Camera enabled.
    #[serde(default)]
    pub self_video: bool,
    /// Permission to speak denied (stage).
    #[serde(default)]
    pub suppress: bool,
    /// When the user requested to speak.
    pub request_to_speak_timestamp: Option<Timestamp>,
}

impl VoiceState {
    /// Whether the user is connected to a voice channel.
    pub const fn is_connected(&self) -> bool {
        self.channel_id.is_some()
    }
}

/// A voice region.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct VoiceRegion {
    /// Region ID.
    pub id: Box<str>,
    /// Display name.
    pub name: Box<str>,
    /// Closest region for the current user.
    #[serde(default)]
    pub optimal: bool,
    /// Deprecated region (avoid switching to it).
    #[serde(default)]
    pub deprecated: bool,
    /// Custom region (events, …).
    #[serde(default)]
    pub custom: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn docs_example_omits_guild_id() {
        let json = r#"{
            "channel_id": "157733188964188161",
            "user_id": "80351110224678912",
            "session_id": "90326bd25d71d39b9ef95b299e3872ff",
            "deaf": false,
            "mute": false,
            "self_deaf": false,
            "self_mute": true,
            "suppress": false,
            "request_to_speak_timestamp": "2021-03-31T18:45:31.297561+00:00"
        }"#;
        let state: VoiceState = serde_json::from_str(json).unwrap();
        assert!(state.is_connected());
        assert!(state.self_mute);
        assert!(state.guild_id.is_none());
        assert!(state.request_to_speak_timestamp.is_some());
    }
}
