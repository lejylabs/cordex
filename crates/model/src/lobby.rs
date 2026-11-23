//! Lobbies (resources/lobby.mdx) and Social SDK lobby messages
//! (events/webhook-events.mdx).

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::channel::Channel;
use crate::id::Id;
use crate::id::marker::{ApplicationMarker, ChannelMarker, LobbyMarker, MessageMarker, UserMarker};
use crate::message::{MessageFlags, MessageType};
use crate::serde_util::bitflags_serde_int;
use crate::timestamp::Timestamp;
use crate::user::User;

/// A Social SDK lobby.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Lobby {
    /// Lobby ID.
    pub id: Id<LobbyMarker>,
    /// Application that created the lobby.
    pub application_id: Id<ApplicationMarker>,
    /// String key/value pairs (max total length 1000).
    pub metadata: Option<HashMap<Box<str>, Box<str>>>,
    /// Members.
    #[serde(default)]
    pub members: Vec<LobbyMember>,
    /// Guild channel linked to the lobby.
    pub linked_channel: Option<Box<Channel>>,
}

/// A member of a lobby.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct LobbyMember {
    /// User ID.
    pub id: Id<UserMarker>,
    /// String key/value pairs (max total length 1000).
    #[serde(default)]
    pub metadata: Option<HashMap<Box<str>, Box<str>>>,
    /// Member flags.
    #[serde(default)]
    pub flags: LobbyMemberFlags,
    /// Extra 1–80 character display name (in-game character).
    pub additional_name: Option<Box<str>>,
}

bitflags::bitflags! {
    /// Lobby member flags.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
    pub struct LobbyMemberFlags: u64 {
        /// User can link a text channel to a lobby.
        const CAN_LINK_LOBBY = 1 << 0;
    }
}

bitflags_serde_int!(LobbyMemberFlags, u64);

/// Invite to a lobby's linked channel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct LobbyInvite {
    /// Guild invite code.
    pub code: Box<str>,
}

/// Lobby chat message (HTTP send/list and `LOBBY_MESSAGE_*` webhooks).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct LobbyMessage {
    /// Message ID.
    pub id: Id<MessageMarker>,
    /// Message type.
    #[serde(rename = "type")]
    pub kind: MessageType,
    /// Content.
    #[serde(default)]
    pub content: Box<str>,
    /// Lobby.
    pub lobby_id: Id<LobbyMarker>,
    /// Same as `lobby_id` on HTTP lobby messages.
    pub channel_id: Id<ChannelMarker>,
    /// Author.
    pub author: User,
    /// Author's `additional_name` at send time (HTTP).
    pub lobby_member: Option<LobbyMessageMember>,
    /// Dispatch-only metadata.
    #[serde(default)]
    pub metadata: Option<HashMap<Box<str>, Box<str>>>,
    /// Moderation metadata (HTTP).
    #[serde(default)]
    pub moderation_metadata: Option<HashMap<Box<str>, Box<str>>>,
    /// Message flags.
    #[serde(default)]
    pub flags: MessageFlags,
    /// Application that sent the message.
    pub application_id: Option<Id<ApplicationMarker>>,
    /// When the message was sent (webhook updates).
    pub timestamp: Option<Timestamp>,
    /// When the message was edited (webhook updates).
    pub edited_timestamp: Option<Timestamp>,
}

/// Nested `lobby_member` on an HTTP lobby message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct LobbyMessageMember {
    /// Author's additional display name.
    pub additional_name: Box<str>,
}

/// `LOBBY_MESSAGE_DELETE` data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct LobbyMessageDelete {
    /// Deleted message.
    pub id: Id<MessageMarker>,
    /// Lobby.
    pub lobby_id: Id<LobbyMarker>,
}

/// In-game DM between provisional accounts (`GAME_DIRECT_MESSAGE_*`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SdkDmMessage {
    /// Message ID.
    pub id: Id<MessageMarker>,
    /// Message type.
    #[serde(rename = "type")]
    pub kind: MessageType,
    /// Content.
    #[serde(default)]
    pub content: Box<str>,
    /// Author.
    pub author: User,
    /// Message flags.
    #[serde(default)]
    pub flags: MessageFlags,
    /// Application that created the message.
    pub application_id: Option<Id<ApplicationMarker>>,
    /// Channel with recipient information.
    pub channel: Option<Box<Channel>>,
    /// Rich Presence activity.
    pub activity: Option<crate::message::MessageActivity>,
    /// Partial application (Rich Presence).
    pub application: Option<Box<crate::application::Application>>,
    /// Channel ID when Discord sends a standard message object instead.
    pub channel_id: Option<Id<ChannelMarker>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn example_lobby_from_docs() {
        let json = r#"{
            "id": "96008815106887111",
            "application_id": "41771983429993937",
            "metadata": { "topic": "we need more redstone" },
            "members": [{
                "id": "41771983429993000",
                "metadata": null,
                "flags": 1,
                "additional_name": "Twindly Shimmerleaf"
            }]
        }"#;
        let lobby: Lobby = serde_json::from_str(json).unwrap();
        assert_eq!(lobby.id.get(), 96_008_815_106_887_111);
        assert_eq!(lobby.members[0].flags, LobbyMemberFlags::CAN_LINK_LOBBY);
        assert_eq!(
            lobby.members[0].additional_name.as_deref(),
            Some("Twindly Shimmerleaf")
        );
    }
}
