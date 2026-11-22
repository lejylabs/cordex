//! Interaction types (interactions/receiving-and-responding.mdx).
//!
//! The parts that messages reference come first; the full interaction
//! object follows with the interaction phase.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::channel::Channel;
use crate::guild::{Member, Role};
use crate::id::Id;
use crate::id::marker::{
    AttachmentMarker, ChannelMarker, GuildMarker, InteractionMarker, MessageMarker, RoleMarker,
    UserMarker,
};
use crate::message::{Attachment, Message};
use crate::serde_util::{id_or_zero, int_enum};
use crate::user::User;

int_enum! {
    /// Interaction type.
    pub enum InteractionType: u8 {
        /// Ping (HTTP interactions).
        Ping = 1,
        /// Application command.
        ApplicationCommand = 2,
        /// Message component.
        MessageComponent = 3,
        /// Autocomplete.
        ApplicationCommandAutocomplete = 4,
        /// Modal submit.
        ModalSubmit = 5,
    }
}

/// Installation contexts an interaction was authorized for
/// ("Authorizing Integration Owners Object").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[non_exhaustive]
pub struct AuthorizingIntegrationOwners {
    /// `GUILD_INSTALL` key: the guild, or `None` when Discord sends `"0"`
    /// (interaction in a DM with the bot).
    #[serde(
        rename = "0",
        default,
        deserialize_with = "id_or_zero",
        skip_serializing_if = "Option::is_none"
    )]
    pub guild: Option<Id<GuildMarker>>,
    /// `USER_INSTALL` key: the user who authorized the app.
    #[serde(rename = "1", default, skip_serializing_if = "Option::is_none")]
    pub user: Option<Id<UserMarker>>,
}

/// Metadata of the interaction that created a message (all three documented
/// variants in one struct; fields of other variants are `None`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct InteractionMetadata {
    /// Interaction ID.
    pub id: Id<InteractionMarker>,
    /// Interaction type.
    #[serde(rename = "type")]
    pub kind: InteractionType,
    /// User who triggered the interaction.
    pub user: User,
    /// Installation contexts.
    #[serde(default)]
    pub authorizing_integration_owners: AuthorizingIntegrationOwners,
    /// Original response message (follow-ups only).
    pub original_response_message_id: Option<Id<MessageMarker>>,
    /// Target user of a user command.
    pub target_user: Option<User>,
    /// Target message of a message command.
    pub target_message_id: Option<Id<MessageMarker>>,
    /// Message with the component (component interactions).
    pub interacted_message_id: Option<Id<MessageMarker>>,
    /// Interaction that opened the modal (modal submits).
    pub triggering_interaction_metadata: Option<Box<InteractionMetadata>>,
}

/// Deprecated `interaction` field of messages; use [`InteractionMetadata`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct MessageInteraction {
    /// Interaction ID.
    pub id: Id<InteractionMarker>,
    /// Interaction type.
    #[serde(rename = "type")]
    pub kind: InteractionType,
    /// Command name (with subcommands).
    pub name: Box<str>,
    /// User who invoked it.
    pub user: User,
    /// Member who invoked it (partial).
    pub member: Option<Member>,
}

/// Users, members, roles, channels, messages and attachments referenced by
/// an interaction or message ("Resolved Data Structure").
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Resolved {
    /// Users.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub users: HashMap<Id<UserMarker>, User>,
    /// Partial members (without `user`, `deaf`, `mute`).
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub members: HashMap<Id<UserMarker>, Member>,
    /// Roles.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub roles: HashMap<Id<RoleMarker>, Role>,
    /// Partial channels.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub channels: HashMap<Id<ChannelMarker>, Channel>,
    /// Partial messages.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub messages: HashMap<Id<MessageMarker>, Message>,
    /// Attachments.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub attachments: HashMap<Id<AttachmentMarker>, Attachment>,
}
