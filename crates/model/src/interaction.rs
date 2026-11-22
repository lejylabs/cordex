//! Interaction types (interactions/receiving-and-responding.mdx).
//!
//! Message-facing pieces ([`InteractionMetadata`], [`Resolved`]) live
//! alongside the full [`Interaction`] object and callback payloads.

use std::collections::HashMap;

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::value::RawValue;

use crate::channel::Channel;
use crate::command::{CommandOptionChoice, CommandOptionType, CommandType, InteractionContextType};
use crate::component::Component;
use crate::entitlement::Entitlement;
use crate::guild::{Member, Role};
use crate::id::Id;
use crate::id::marker::{
    ApplicationMarker, AttachmentMarker, ChannelMarker, CommandMarker, GenericMarker, GuildMarker,
    InteractionMarker, MessageMarker, RoleMarker, UserMarker,
};
use crate::message::{AllowedMentions, Attachment, Embed, Message, MessageFlags};
use crate::permissions::Permissions;
use crate::poll::PollCreate;
use crate::serde_util::{id_or_zero, int_enum};
use crate::token::InteractionToken;
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

fn interaction_version() -> u8 {
    1
}

/// An interaction received over the gateway or HTTP.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[non_exhaustive]
pub struct Interaction {
    /// Interaction ID.
    pub id: Id<InteractionMarker>,
    /// Application this interaction is for.
    pub application_id: Id<ApplicationMarker>,
    /// Interaction type.
    #[serde(rename = "type")]
    pub kind: InteractionType,
    /// Type-specific payload; absent on `PING` and unknown future types
    /// that send no `data`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<InteractionData>,
    /// Partial guild (`id`, `locale`, `features`); not a full guild object.
    pub guild: Option<InteractionGuild>,
    /// Guild ID.
    pub guild_id: Option<Id<GuildMarker>>,
    /// Partial channel.
    pub channel: Option<Box<Channel>>,
    /// Channel ID.
    pub channel_id: Option<Id<ChannelMarker>>,
    /// Invoking member (guilds).
    pub member: Option<Box<Member>>,
    /// Invoking user (DMs; also present alongside `member` in some payloads).
    pub user: Option<User>,
    /// Continuation token for responding (15 minutes).
    pub token: InteractionToken,
    /// Always `1`.
    #[serde(default = "interaction_version")]
    pub version: u8,
    /// Message the component or modal is attached to.
    pub message: Option<Box<Message>>,
    /// App permissions in the source location.
    #[serde(default)]
    pub app_permissions: Permissions,
    /// Invoking user's locale (absent on `PING`).
    pub locale: Option<Box<str>>,
    /// Guild preferred locale.
    pub guild_locale: Option<Box<str>>,
    /// Entitlements of the invoking user.
    #[serde(default)]
    pub entitlements: Vec<Entitlement>,
    /// Installation contexts this interaction was authorized for.
    #[serde(default)]
    pub authorizing_integration_owners: AuthorizingIntegrationOwners,
    /// Context the interaction was triggered from.
    pub context: Option<InteractionContextType>,
    /// Attachment size limit in bytes.
    #[serde(default)]
    pub attachment_size_limit: u64,
}

impl Interaction {
    /// Invoking user: `user`, or `member.user`.
    pub fn author(&self) -> Option<&User> {
        self.user
            .as_ref()
            .or_else(|| self.member.as_ref().and_then(|m| m.user.as_ref()))
    }

    /// `custom_id` of a component or modal, if this interaction has one.
    pub fn custom_id(&self) -> Option<&str> {
        match self.data.as_ref()? {
            InteractionData::MessageComponent(data) => Some(data.custom_id.as_ref()),
            InteractionData::ModalSubmit(data) => Some(data.custom_id.as_ref()),
            InteractionData::Command(_) | InteractionData::Unknown(_) => None,
        }
    }
}

/// Partial guild sent with an interaction (`id`, `locale`, `features`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct InteractionGuild {
    /// Guild ID.
    pub id: Id<GuildMarker>,
    /// Preferred locale of the guild.
    pub locale: Option<Box<str>>,
    /// Enabled guild features.
    #[serde(default)]
    pub features: Vec<crate::guild::GuildFeature>,
}

/// Type-specific `data` of an interaction.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum InteractionData {
    /// `APPLICATION_COMMAND` and `APPLICATION_COMMAND_AUTOCOMPLETE`.
    Command(CommandData),
    /// `MESSAGE_COMPONENT`.
    MessageComponent(MessageComponentData),
    /// `MODAL_SUBMIT`.
    ModalSubmit(ModalSubmitData),
    /// A type this version does not know yet, kept as raw JSON.
    Unknown(Box<RawValue>),
}

impl PartialEq for InteractionData {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Command(a), Self::Command(b)) => a == b,
            (Self::MessageComponent(a), Self::MessageComponent(b)) => a == b,
            (Self::ModalSubmit(a), Self::ModalSubmit(b)) => a == b,
            (Self::Unknown(a), Self::Unknown(b)) => a.get() == b.get(),
            _ => false,
        }
    }
}

/// Application command / autocomplete `data`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct CommandData {
    /// Invoked command.
    pub id: Id<CommandMarker>,
    /// Command name.
    pub name: Box<str>,
    /// Command type.
    #[serde(rename = "type")]
    pub kind: CommandType,
    /// Resolved users, roles, channels, messages and attachments.
    pub resolved: Option<Resolved>,
    /// Options and values; partial on autocomplete.
    #[serde(default)]
    pub options: Vec<CommandDataOption>,
    /// Guild the command is registered to.
    pub guild_id: Option<Id<GuildMarker>>,
    /// Target of a user or message command.
    pub target_id: Option<Id<GenericMarker>>,
}

impl CommandData {
    /// Direct option by name (top level only).
    pub fn option(&self, name: &str) -> Option<&CommandDataOption> {
        self.options.iter().find(|o| &*o.name == name)
    }

    /// Currently focused option for autocomplete, walking nested options.
    pub fn focused_option(&self) -> Option<&CommandDataOption> {
        fn walk(options: &[CommandDataOption]) -> Option<&CommandDataOption> {
            for option in options {
                if option.focused {
                    return Some(option);
                }
                if let Some(found) = walk(&option.options) {
                    return Some(found);
                }
            }
            None
        }
        walk(&self.options)
    }
}

/// One option (or subcommand) in command interaction data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct CommandDataOption {
    /// Option name.
    pub name: Box<str>,
    /// Option type.
    #[serde(rename = "type")]
    pub kind: CommandOptionType,
    /// User-supplied value (mutually exclusive with `options`).
    pub value: Option<CommandOptionValue>,
    /// Nested options of a subcommand or group.
    #[serde(default)]
    pub options: Vec<CommandDataOption>,
    /// Whether this option is focused for autocomplete.
    #[serde(default)]
    pub focused: bool,
}

/// Value of a command option as submitted by the user.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CommandOptionValue {
    /// Boolean option.
    Boolean(bool),
    /// Integer option.
    Integer(i64),
    /// Number option.
    Number(f64),
    /// String, snowflake (user/role/channel/mentionable/attachment) or
    /// remaining text.
    String(Box<str>),
}

/// Message component `data`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct MessageComponentData {
    /// Developer `custom_id`.
    pub custom_id: Box<str>,
    /// Component type.
    pub component_type: u8,
    /// Numeric component `id` (Components V2).
    pub id: Option<u32>,
    /// Selected values (always present for selects).
    #[serde(default)]
    pub values: Vec<Box<str>>,
    /// Resolved entities of selected options.
    pub resolved: Option<Resolved>,
}

/// Modal submit `data`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ModalSubmitData {
    /// Developer `custom_id` of the modal.
    pub custom_id: Box<str>,
    /// Submitted component values.
    #[serde(default)]
    pub components: Vec<SubmittedComponent>,
    /// Resolved entities of selected options.
    pub resolved: Option<Resolved>,
}

/// A component as submitted in a modal (or nested under a Label).
///
/// One struct covers every documented response shape (`value`, `values`,
/// nested `component` / `components`) so unknown extra keys stay ignored
/// while submitted values round-trip.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SubmittedComponent {
    /// Component type.
    #[serde(rename = "type")]
    pub kind: u8,
    /// Numeric component `id`.
    pub id: Option<u32>,
    /// Developer `custom_id`.
    pub custom_id: Option<Box<str>>,
    /// Text-input / checkbox value.
    pub value: Option<Box<str>>,
    /// Selected values (selects, file uploads, radio/checkbox groups).
    #[serde(default)]
    pub values: Vec<Box<str>>,
    /// Nested components (legacy action rows).
    #[serde(default)]
    pub components: Vec<SubmittedComponent>,
    /// Wrapped component (Label, type 18).
    pub component: Option<Box<SubmittedComponent>>,
}

impl<'de> Deserialize<'de> for Interaction {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Raw {
            id: Id<InteractionMarker>,
            application_id: Id<ApplicationMarker>,
            #[serde(rename = "type")]
            kind: InteractionType,
            data: Option<Box<RawValue>>,
            guild: Option<InteractionGuild>,
            guild_id: Option<Id<GuildMarker>>,
            channel: Option<Box<Channel>>,
            channel_id: Option<Id<ChannelMarker>>,
            member: Option<Box<Member>>,
            user: Option<User>,
            token: InteractionToken,
            #[serde(default = "interaction_version")]
            version: u8,
            message: Option<Box<Message>>,
            #[serde(default)]
            app_permissions: Permissions,
            locale: Option<Box<str>>,
            guild_locale: Option<Box<str>>,
            #[serde(default)]
            entitlements: Vec<Entitlement>,
            #[serde(default)]
            authorizing_integration_owners: AuthorizingIntegrationOwners,
            context: Option<InteractionContextType>,
            #[serde(default)]
            attachment_size_limit: u64,
        }

        let raw = Raw::deserialize(deserializer)?;
        let data = match (raw.kind, raw.data) {
            (InteractionType::Ping, _) | (_, None) => None,
            (
                InteractionType::ApplicationCommand
                | InteractionType::ApplicationCommandAutocomplete,
                Some(value),
            ) => Some(
                serde_json::from_str(value.get())
                    .map(InteractionData::Command)
                    .map_err(D::Error::custom)?,
            ),
            (InteractionType::MessageComponent, Some(value)) => Some(
                serde_json::from_str(value.get())
                    .map(InteractionData::MessageComponent)
                    .map_err(D::Error::custom)?,
            ),
            (InteractionType::ModalSubmit, Some(value)) => Some(
                serde_json::from_str(value.get())
                    .map(InteractionData::ModalSubmit)
                    .map_err(D::Error::custom)?,
            ),
            (_, Some(value)) => Some(InteractionData::Unknown(value)),
        };

        Ok(Self {
            id: raw.id,
            application_id: raw.application_id,
            kind: raw.kind,
            data,
            guild: raw.guild,
            guild_id: raw.guild_id,
            channel: raw.channel,
            channel_id: raw.channel_id,
            member: raw.member,
            user: raw.user,
            token: raw.token,
            version: raw.version,
            message: raw.message,
            app_permissions: raw.app_permissions,
            locale: raw.locale,
            guild_locale: raw.guild_locale,
            entitlements: raw.entitlements,
            authorizing_integration_owners: raw.authorizing_integration_owners,
            context: raw.context,
            attachment_size_limit: raw.attachment_size_limit,
        })
    }
}

impl Serialize for InteractionData {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Command(data) => data.serialize(serializer),
            Self::MessageComponent(data) => data.serialize(serializer),
            Self::ModalSubmit(data) => data.serialize(serializer),
            Self::Unknown(raw) => raw.serialize(serializer),
        }
    }
}

int_enum! {
    /// Interaction callback type (the `type` of an interaction response).
    pub enum InteractionCallbackType: u8 {
        /// ACK a ping.
        Pong = 1,
        /// Respond with a message.
        ChannelMessageWithSource = 4,
        /// ACK and edit later; the user sees a loading state.
        DeferredChannelMessageWithSource = 5,
        /// ACK a component; edit the original later, no loading state.
        DeferredUpdateMessage = 6,
        /// Edit the message the component is attached to.
        UpdateMessage = 7,
        /// Autocomplete suggestions.
        ApplicationCommandAutocompleteResult = 8,
        /// Open a modal.
        Modal = 9,
        /// Deprecated: premium upgrade button.
        PremiumRequired = 10,
        /// Launch the app's Activity.
        LaunchActivity = 12,
    }
}

/// Response sent to Discord for an interaction.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct InteractionResponse {
    /// Callback type.
    #[serde(rename = "type")]
    pub kind: InteractionCallbackType,
    /// Callback data (message, autocomplete choices or modal).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<InteractionCallbackData>,
}

impl InteractionResponse {
    /// Response of any callback type with optional data.
    pub const fn new(kind: InteractionCallbackType, data: Option<InteractionCallbackData>) -> Self {
        Self { kind, data }
    }

    /// `PONG` (type 1).
    pub const fn pong() -> Self {
        Self {
            kind: InteractionCallbackType::Pong,
            data: None,
        }
    }

    /// Message response (type 4) with content.
    pub fn message(content: impl Into<Box<str>>) -> Self {
        Self {
            kind: InteractionCallbackType::ChannelMessageWithSource,
            data: Some(InteractionCallbackData {
                content: Some(content.into()),
                ..InteractionCallbackData::empty()
            }),
        }
    }

    /// Deferred message (type 5); `ephemeral` sets the `EPHEMERAL` flag.
    pub fn defer(ephemeral: bool) -> Self {
        Self {
            kind: InteractionCallbackType::DeferredChannelMessageWithSource,
            data: if ephemeral {
                Some(InteractionCallbackData {
                    flags: MessageFlags::EPHEMERAL,
                    ..InteractionCallbackData::empty()
                })
            } else {
                None
            },
        }
    }

    /// Autocomplete suggestions (type 8).
    pub fn autocomplete(choices: Vec<CommandOptionChoice>) -> Self {
        Self {
            kind: InteractionCallbackType::ApplicationCommandAutocompleteResult,
            data: Some(InteractionCallbackData {
                choices,
                ..InteractionCallbackData::empty()
            }),
        }
    }

    /// Modal (type 9).
    pub fn modal(
        custom_id: impl Into<Box<str>>,
        title: impl Into<Box<str>>,
        components: Vec<Component>,
    ) -> Self {
        Self {
            kind: InteractionCallbackType::Modal,
            data: Some(InteractionCallbackData {
                custom_id: Some(custom_id.into()),
                title: Some(title.into()),
                components,
                ..InteractionCallbackData::empty()
            }),
        }
    }
}

/// Callback data for a message, autocomplete result or modal.
///
/// Fields of the variants that are not in use stay empty/`None`.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[non_exhaustive]
pub struct InteractionCallbackData {
    /// TTS message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tts: Option<bool>,
    /// Message content.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<Box<str>>,
    /// Embeds (max. 10).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub embeds: Vec<Embed>,
    /// Allowed mentions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_mentions: Option<AllowedMentions>,
    /// Message flags (`EPHEMERAL`, `SUPPRESS_EMBEDS`, …).
    #[serde(default, skip_serializing_if = "MessageFlags::is_empty")]
    pub flags: MessageFlags,
    /// Components (message or modal).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub components: Vec<Component>,
    /// Partial attachments (`filename`, `description`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attachments: Vec<Attachment>,
    /// Poll to create.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub poll: Option<PollCreate>,
    /// Autocomplete choices (max. 25).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub choices: Vec<CommandOptionChoice>,
    /// Modal `custom_id`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_id: Option<Box<str>>,
    /// Modal title (max. 45 characters).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<Box<str>>,
}

impl InteractionCallbackData {
    fn empty() -> Self {
        Self::default()
    }
}

/// Response of `POST /interactions/{id}/{token}/callback` when
/// `with_response` is set.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct InteractionCallbackResponse {
    /// Interaction that was responded to.
    pub interaction: InteractionCallback,
    /// Resource created by the response.
    pub resource: Option<InteractionCallbackResource>,
}

/// Interaction metadata in a callback response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct InteractionCallback {
    /// Interaction ID.
    pub id: Id<InteractionMarker>,
    /// Interaction type.
    #[serde(rename = "type")]
    pub kind: InteractionType,
    /// Activity instance if one was launched or joined.
    pub activity_instance_id: Option<Box<str>>,
    /// Message created by the interaction.
    pub response_message_id: Option<Id<MessageMarker>>,
    /// Whether that message is in a loading state.
    pub response_message_loading: Option<bool>,
    /// Whether that message is ephemeral.
    pub response_message_ephemeral: Option<bool>,
}

/// Resource created by an interaction callback.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct InteractionCallbackResource {
    /// Callback type that created the resource.
    #[serde(rename = "type")]
    pub kind: InteractionCallbackType,
    /// Activity launched by `LAUNCH_ACTIVITY`.
    pub activity_instance: Option<ActivityInstanceResource>,
    /// Message created by `CHANNEL_MESSAGE_WITH_SOURCE` or `UPDATE_MESSAGE`.
    pub message: Option<Box<Message>>,
}

/// Activity instance launched by an interaction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ActivityInstanceResource {
    /// Instance ID.
    pub id: Box<str>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ping_has_no_data() {
        let json = r#"{"id":"1","application_id":"2","type":1,"token":"secret","version":1}"#;
        let ping: Interaction = serde_json::from_str(json).unwrap();
        assert_eq!(ping.kind, InteractionType::Ping);
        assert!(ping.data.is_none());
        assert!(!format!("{ping:?}").contains("secret"));
    }

    #[test]
    fn autocomplete_focused_option() {
        let json = r#"{
            "id":"1","application_id":"2","type":4,"token":"t","version":1,
            "data":{
                "id":"816437322781949972","name":"airhorn","type":1,
                "options":[{"type":3,"name":"variant","value":"data a user is typ","focused":true}]
            }
        }"#;
        let interaction: Interaction = serde_json::from_str(json).unwrap();
        let InteractionData::Command(data) = interaction.data.unwrap() else {
            panic!("expected command data");
        };
        let focused = data.focused_option().unwrap();
        assert_eq!(&*focused.name, "variant");
        assert_eq!(
            focused.value,
            Some(CommandOptionValue::String("data a user is typ".into()))
        );
    }

    #[test]
    fn unknown_interaction_type_keeps_raw_data() {
        let json = r#"{"id":"1","application_id":"2","type":99,"token":"t","data":{"x":1}}"#;
        let interaction: Interaction = serde_json::from_str(json).unwrap();
        assert!(matches!(interaction.kind, InteractionType::Unknown(99)));
        assert!(matches!(
            interaction.data,
            Some(InteractionData::Unknown(_))
        ));
    }

    #[test]
    fn authorizing_owners_zero_guild_is_none() {
        let json = r#"{"0":"0","1":"5"}"#;
        let owners: AuthorizingIntegrationOwners = serde_json::from_str(json).unwrap();
        assert!(owners.guild.is_none());
        assert_eq!(owners.user.unwrap().get(), 5);
    }

    /// Shape of a real gateway `INTERACTION_CREATE` (2026-11): partial
    /// guild without `name`, partial channel, member with `permissions`.
    #[test]
    fn live_gateway_slash_interaction() {
        let json = r#"{
            "version": 1, "type": 2, "token": "secret-token",
            "member": {
                "user": {"username": "lejy", "public_flags": 0, "primary_guild": null,
                    "id": "111", "global_name": "Lejy", "display_name_styles": null,
                    "discriminator": "0", "collectibles": null, "clan": null,
                    "avatar_decoration_data": null, "avatar": "a1b2"},
                "unusual_dm_activity_until": null, "roles": [], "premium_since": null,
                "permissions": "2251799813685247", "pending": false, "nick": null,
                "mute": false, "joined_at": "2024-05-01T10:00:00.000000+00:00",
                "flags": 0, "deaf": false, "communication_disabled_until": null,
                "banner": null, "avatar": null, "collectibles": null
            },
            "locale": "de", "id": "222", "guild_locale": "en-US", "guild_id": "333",
            "guild": {"locale": "en-US", "id": "333", "features": ["COMMUNITY", "NEW_THING"]},
            "entitlements": [], "entitlement_sku_ids": [],
            "data": {"type": 1, "name": "ping", "id": "444", "guild_id": "333"},
            "context": 0, "channel_id": "555",
            "channel": {"type": 0, "topic": null, "theme_color": null,
                "rate_limit_per_user": 0, "position": 0, "permissions": "2251799813685247",
                "parent_id": "666", "nsfw": false, "name": "general",
                "last_message_id": "777", "id": "555", "icon_emoji": null,
                "guild_id": "333", "flags": 0},
            "authorizing_integration_owners": {"0": "333"},
            "attachment_size_limit": 10485760,
            "application_id": "888", "app_permissions": "2251799813685247"
        }"#;
        let i: Interaction = serde_json::from_str(json).unwrap();
        assert_eq!(i.guild.as_ref().unwrap().id.get(), 333);
        assert_eq!(i.guild.as_ref().unwrap().locale.as_deref(), Some("en-US"));
        assert_eq!(
            &*i.channel.as_ref().unwrap().name.clone().unwrap(),
            "general"
        );
        assert_eq!(i.author().unwrap().id.get(), 111);
        let Some(InteractionData::Command(data)) = &i.data else {
            panic!("command data expected");
        };
        assert_eq!(&*data.name, "ping");
    }
}
