//! Interaction types (interactions/receiving-and-responding.mdx).
//!
//! Message-facing pieces ([`InteractionMetadata`], [`Resolved`]) live
//! alongside the full [`Interaction`] object and callback payloads.

use std::collections::HashMap;

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
#[derive(Debug, Clone, PartialEq, Serialize)]
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

impl TryFrom<crate::serde_util::Scalar> for CommandOptionValue {
    type Error = &'static str;

    fn try_from(value: crate::serde_util::Scalar) -> Result<Self, Self::Error> {
        use crate::serde_util::Scalar;
        Ok(match value {
            Scalar::Bool(b) => Self::Boolean(b),
            Scalar::Int(i) => Self::Integer(i),
            Scalar::Float(f) => Self::Number(f),
            Scalar::Str(s) => Self::String(s),
        })
    }
}

impl<'de> Deserialize<'de> for CommandOptionValue {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        d.deserialize_any(crate::serde_util::ScalarVisitor(
            "a boolean, number or string",
            std::marker::PhantomData,
        ))
    }
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
    /// Text-input or radio-group value (string), checkbox state (bool).
    pub value: Option<SubmittedValue>,
    /// Selected values (selects, file uploads, checkbox groups).
    #[serde(default)]
    pub values: Vec<Box<str>>,
    /// Nested components (legacy action rows).
    #[serde(default)]
    pub components: Vec<SubmittedComponent>,
    /// Wrapped component (Label, type 18).
    pub component: Option<Box<SubmittedComponent>>,
}

/// `value` of a [`SubmittedComponent`]: a string for text inputs and radio
/// groups, a boolean for checkboxes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubmittedValue {
    /// Text input or radio group.
    Text(Box<str>),
    /// Checkbox (`true` = checked).
    Bool(bool),
}

impl SubmittedValue {
    /// The string value, `None` for a checkbox.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::Text(text) => Some(text),
            Self::Bool(_) => None,
        }
    }

    /// The checkbox state, `None` for a string value.
    pub const fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(checked) => Some(*checked),
            Self::Text(_) => None,
        }
    }
}

impl Serialize for SubmittedValue {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Text(text) => serializer.serialize_str(text),
            Self::Bool(checked) => serializer.serialize_bool(*checked),
        }
    }
}

impl<'de> Deserialize<'de> for SubmittedValue {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;

        impl serde::de::Visitor<'_> for Visitor {
            type Value = SubmittedValue;

            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("a string or a boolean")
            }

            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Self::Value, E> {
                Ok(SubmittedValue::Text(v.into()))
            }

            fn visit_bool<E: serde::de::Error>(self, v: bool) -> Result<Self::Value, E> {
                Ok(SubmittedValue::Bool(v))
            }
        }

        deserializer.deserialize_any(Visitor)
    }
}

/// Parse `data` for a known interaction type in one pass.
fn parse_data<'de, A: serde::de::MapAccess<'de>>(
    map: &mut A,
    kind: InteractionType,
) -> Result<Option<InteractionData>, A::Error> {
    Ok(match kind {
        InteractionType::ApplicationCommand | InteractionType::ApplicationCommandAutocomplete => {
            map.next_value::<Option<CommandData>>()?
                .map(InteractionData::Command)
        }
        InteractionType::MessageComponent => map
            .next_value::<Option<MessageComponentData>>()?
            .map(InteractionData::MessageComponent),
        InteractionType::ModalSubmit => map
            .next_value::<Option<ModalSubmitData>>()?
            .map(InteractionData::ModalSubmit),
        InteractionType::Ping => {
            map.next_value::<serde::de::IgnoredAny>()?;
            None
        }
        _ => map
            .next_value::<Option<Box<RawValue>>>()?
            .map(InteractionData::Unknown),
    })
}

/// Parse `data` that arrived before `type`.
fn finish_data<E: serde::de::Error>(
    kind: InteractionType,
    raw: Box<RawValue>,
) -> Result<Option<InteractionData>, E> {
    Ok(match kind {
        InteractionType::Ping => None,
        InteractionType::ApplicationCommand | InteractionType::ApplicationCommandAutocomplete => {
            Some(InteractionData::Command(
                serde_json::from_str(raw.get()).map_err(E::custom)?,
            ))
        }
        InteractionType::MessageComponent => Some(InteractionData::MessageComponent(
            serde_json::from_str(raw.get()).map_err(E::custom)?,
        )),
        InteractionType::ModalSubmit => Some(InteractionData::ModalSubmit(
            serde_json::from_str(raw.get()).map_err(E::custom)?,
        )),
        _ => Some(InteractionData::Unknown(raw)),
    })
}

#[derive(Clone, Copy, Deserialize)]
#[serde(field_identifier, rename_all = "snake_case")]
enum InteractionField {
    Id,
    ApplicationId,
    #[serde(rename = "type")]
    Kind,
    Data,
    Guild,
    GuildId,
    Channel,
    ChannelId,
    Member,
    User,
    Token,
    Version,
    Message,
    AppPermissions,
    Locale,
    GuildLocale,
    Entitlements,
    AuthorizingIntegrationOwners,
    Context,
    AttachmentSizeLimit,
    #[serde(other)]
    Other,
}

/// Fields of an interaction while its object is read.
#[derive(Default)]
struct InteractionParts {
    id: Option<Id<InteractionMarker>>,
    application_id: Option<Id<ApplicationMarker>>,
    kind: Option<InteractionType>,
    /// `data` typed (when `type` came first, the usual key order) ...
    data: Option<InteractionData>,
    /// ... or raw, parsed once `type` is known.
    raw_data: Option<Box<RawValue>>,
    guild: Option<InteractionGuild>,
    guild_id: Option<Id<GuildMarker>>,
    channel: Option<Box<Channel>>,
    channel_id: Option<Id<ChannelMarker>>,
    member: Option<Box<Member>>,
    user: Option<User>,
    token: Option<InteractionToken>,
    version: Option<u8>,
    message: Option<Box<Message>>,
    app_permissions: Option<Permissions>,
    locale: Option<Box<str>>,
    guild_locale: Option<Box<str>>,
    entitlements: Option<Vec<Entitlement>>,
    owners: Option<AuthorizingIntegrationOwners>,
    context: Option<InteractionContextType>,
    attachment_size_limit: Option<u64>,
}

impl InteractionParts {
    fn read<'de, A: serde::de::MapAccess<'de>>(
        &mut self,
        field: InteractionField,
        map: &mut A,
    ) -> Result<(), A::Error> {
        use InteractionField as F;
        match field {
            F::Id => self.id = Some(map.next_value()?),
            F::ApplicationId => self.application_id = Some(map.next_value()?),
            F::Kind => self.kind = Some(map.next_value()?),
            F::Data => match self.kind {
                Some(kind) => self.data = parse_data(map, kind)?,
                None => self.raw_data = map.next_value()?,
            },
            F::Guild => self.guild = map.next_value()?,
            F::GuildId => self.guild_id = map.next_value()?,
            F::Channel => self.channel = map.next_value()?,
            F::ChannelId => self.channel_id = map.next_value()?,
            F::Member => self.member = map.next_value()?,
            F::User => self.user = map.next_value()?,
            F::Token => self.token = Some(map.next_value()?),
            F::Version => self.version = Some(map.next_value()?),
            F::Message => self.message = map.next_value()?,
            F::AppPermissions => self.app_permissions = Some(map.next_value()?),
            F::Locale => self.locale = map.next_value()?,
            F::GuildLocale => self.guild_locale = map.next_value()?,
            F::Entitlements => self.entitlements = Some(map.next_value()?),
            F::AuthorizingIntegrationOwners => self.owners = Some(map.next_value()?),
            F::Context => self.context = map.next_value()?,
            F::AttachmentSizeLimit => self.attachment_size_limit = Some(map.next_value()?),
            F::Other => {
                map.next_value::<serde::de::IgnoredAny>()?;
            }
        }
        Ok(())
    }

    fn finish<E: serde::de::Error>(self) -> Result<Interaction, E> {
        let kind = self.kind.ok_or_else(|| E::missing_field("type"))?;
        let data = match (self.data, self.raw_data) {
            (Some(data), _) => Some(data),
            (None, Some(raw)) => finish_data(kind, raw)?,
            (None, None) => None,
        };
        Ok(Interaction {
            id: self.id.ok_or_else(|| E::missing_field("id"))?,
            application_id: self
                .application_id
                .ok_or_else(|| E::missing_field("application_id"))?,
            kind,
            data,
            guild: self.guild,
            guild_id: self.guild_id,
            channel: self.channel,
            channel_id: self.channel_id,
            member: self.member,
            user: self.user,
            token: self.token.ok_or_else(|| E::missing_field("token"))?,
            version: self.version.unwrap_or_else(interaction_version),
            message: self.message,
            app_permissions: self.app_permissions.unwrap_or_default(),
            locale: self.locale,
            guild_locale: self.guild_locale,
            entitlements: self.entitlements.unwrap_or_default(),
            authorizing_integration_owners: self.owners.unwrap_or_default(),
            context: self.context,
            attachment_size_limit: self.attachment_size_limit.unwrap_or_default(),
        })
    }
}

impl<'de> Deserialize<'de> for Interaction {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;

        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = Interaction;

            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("an interaction object")
            }

            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut map: A,
            ) -> Result<Interaction, A::Error> {
                let mut parts = InteractionParts::default();
                while let Some(field) = map.next_key()? {
                    parts.read(field, &mut map)?;
                }
                parts.finish()
            }
        }

        deserializer.deserialize_map(Visitor)
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

    /// Update message (type 7) with content.
    ///
    /// Chain [`.clear_components()`](Self::clear_components),
    /// [`.clear_embeds()`](Self::clear_embeds) or
    /// [`.clear_attachments()`](Self::clear_attachments) to explicitly send
    /// empty arrays and remove existing buttons, embeds or attachments.
    pub fn update_message(content: impl Into<Box<str>>) -> Self {
        Self {
            kind: InteractionCallbackType::UpdateMessage,
            data: Some(InteractionCallbackData {
                content: Some(content.into()),
                ..InteractionCallbackData::empty()
            }),
        }
    }

    /// Clear all components on the target message (sends `"components": []`).
    #[must_use]
    pub fn clear_components(mut self) -> Self {
        self.data
            .get_or_insert_with(InteractionCallbackData::empty)
            .clear_components();
        self
    }

    /// Clear all embeds on the target message (sends `"embeds": []`).
    #[must_use]
    pub fn clear_embeds(mut self) -> Self {
        self.data
            .get_or_insert_with(InteractionCallbackData::empty)
            .clear_embeds();
        self
    }

    /// Clear all attachments on the target message (sends `"attachments": []`).
    #[must_use]
    pub fn clear_attachments(mut self) -> Self {
        self.data
            .get_or_insert_with(InteractionCallbackData::empty)
            .clear_attachments();
        self
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
#[derive(Debug, Clone, PartialEq, Default)]
#[non_exhaustive]
pub struct InteractionCallbackData {
    /// TTS message.
    pub tts: Option<bool>,
    /// Message content.
    pub content: Option<Box<str>>,
    /// Embeds (max. 10).
    pub embeds: Vec<Embed>,
    /// Allowed mentions.
    pub allowed_mentions: Option<AllowedMentions>,
    /// Message flags (`EPHEMERAL`, `SUPPRESS_EMBEDS`, …).
    pub flags: MessageFlags,
    /// Components (message or modal).
    pub components: Vec<Component>,
    /// Partial attachments (`filename`, `description`).
    pub attachments: Vec<Attachment>,
    /// Poll to create.
    pub poll: Option<PollCreate>,
    /// Autocomplete choices (max. 25).
    pub choices: Vec<CommandOptionChoice>,
    /// Modal `custom_id`.
    pub custom_id: Option<Box<str>>,
    /// Modal title (max. 45 characters).
    pub title: Option<Box<str>>,
    clear_embeds: bool,
    clear_components: bool,
    clear_attachments: bool,
}

impl<'de> Deserialize<'de> for InteractionCallbackData {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Raw {
            #[serde(default)]
            tts: Option<bool>,
            #[serde(default)]
            content: Option<Box<str>>,
            #[serde(default)]
            embeds: Option<Vec<Embed>>,
            #[serde(default)]
            allowed_mentions: Option<AllowedMentions>,
            #[serde(default)]
            flags: MessageFlags,
            #[serde(default)]
            components: Option<Vec<Component>>,
            #[serde(default)]
            attachments: Option<Vec<Attachment>>,
            #[serde(default)]
            poll: Option<PollCreate>,
            #[serde(default)]
            choices: Vec<CommandOptionChoice>,
            #[serde(default)]
            custom_id: Option<Box<str>>,
            #[serde(default)]
            title: Option<Box<str>>,
        }
        let raw = Raw::deserialize(deserializer)?;
        let clear_embeds = raw.embeds.as_ref().is_some_and(Vec::is_empty);
        let clear_components = raw.components.as_ref().is_some_and(Vec::is_empty);
        let clear_attachments = raw.attachments.as_ref().is_some_and(Vec::is_empty);
        Ok(Self {
            tts: raw.tts,
            content: raw.content,
            embeds: raw.embeds.unwrap_or_default(),
            allowed_mentions: raw.allowed_mentions,
            flags: raw.flags,
            components: raw.components.unwrap_or_default(),
            attachments: raw.attachments.unwrap_or_default(),
            poll: raw.poll,
            choices: raw.choices,
            custom_id: raw.custom_id,
            title: raw.title,
            clear_embeds,
            clear_components,
            clear_attachments,
        })
    }
}

impl InteractionCallbackData {
    fn empty() -> Self {
        Self::default()
    }

    /// Clear all embeds and serialize `"embeds": []`.
    pub fn clear_embeds(&mut self) -> &mut Self {
        self.embeds.clear();
        self.clear_embeds = true;
        self
    }

    /// Clear all components and serialize `"components": []`.
    pub fn clear_components(&mut self) -> &mut Self {
        self.components.clear();
        self.clear_components = true;
        self
    }

    /// Clear all attachments and serialize `"attachments": []`.
    pub fn clear_attachments(&mut self) -> &mut Self {
        self.attachments.clear();
        self.clear_attachments = true;
        self
    }
}

impl Serialize for InteractionCallbackData {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;

        let mut map = serializer.serialize_map(None)?;
        if let Some(tts) = self.tts {
            map.serialize_entry("tts", &tts)?;
        }
        if let Some(content) = &self.content {
            map.serialize_entry("content", content)?;
        }
        if !self.embeds.is_empty() || self.clear_embeds {
            map.serialize_entry("embeds", &self.embeds)?;
        }
        if let Some(allowed_mentions) = &self.allowed_mentions {
            map.serialize_entry("allowed_mentions", allowed_mentions)?;
        }
        if !self.flags.is_empty() {
            map.serialize_entry("flags", &self.flags)?;
        }
        if !self.components.is_empty() || self.clear_components {
            map.serialize_entry("components", &self.components)?;
        }
        if !self.attachments.is_empty() || self.clear_attachments {
            map.serialize_entry("attachments", &self.attachments)?;
        }
        if let Some(poll) = &self.poll {
            map.serialize_entry("poll", poll)?;
        }
        if !self.choices.is_empty() {
            map.serialize_entry("choices", &self.choices)?;
        }
        if let Some(custom_id) = &self.custom_id {
            map.serialize_entry("custom_id", custom_id)?;
        }
        if let Some(title) = &self.title {
            map.serialize_entry("title", title)?;
        }
        map.end()
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

    #[test]
    fn modal_submit_radio_and_checkbox_values() {
        let json = r#"{
            "id":"1","application_id":"2","type":5,"token":"t","version":1,
            "data":{"custom_id":"feedback","components":[
                {"id":1,"type":18,"component":{"id":2,"type":4,"custom_id":"text","value":"hi"}},
                {"id":3,"type":18,"component":{"id":4,"type":21,"custom_id":"rating","value":"okay"}},
                {"id":5,"type":18,"component":{"id":6,"type":21,"custom_id":"none","value":null}},
                {"id":7,"type":18,"component":{"id":8,"type":23,"custom_id":"contact","value":true}}
            ]}
        }"#;
        let interaction: Interaction = serde_json::from_str(json).unwrap();
        let InteractionData::ModalSubmit(data) = interaction.data.unwrap() else {
            panic!("expected modal submit data");
        };
        let value = |i: usize| data.components[i].component.as_ref().unwrap().value.clone();
        assert_eq!(value(0).unwrap().as_str(), Some("hi"));
        assert_eq!(value(1).unwrap().as_str(), Some("okay"));
        assert_eq!(value(2), None);
        let checkbox = value(3).unwrap();
        assert_eq!(checkbox.as_bool(), Some(true));
        assert_eq!(checkbox.as_str(), None);
        assert_eq!(SubmittedValue::Text("x".into()).as_bool(), None);
        assert_eq!(serde_json::to_string(&checkbox).unwrap(), "true");
        assert_eq!(
            serde_json::to_string(&SubmittedValue::Text("x".into())).unwrap(),
            r#""x""#
        );
        assert!(serde_json::from_str::<SubmittedValue>("1").is_err());
    }

    #[test]
    fn data_parses_in_any_key_order() {
        let cases = [
            (
                r#"{"data":{"id":"3","name":"a","type":1},"type":2}"#,
                "command",
            ),
            (
                r#"{"type":2,"data":{"id":"3","name":"a","type":1}}"#,
                "command",
            ),
            (
                r#"{"data":{"custom_id":"b","component_type":2},"type":3}"#,
                "component",
            ),
            (
                r#"{"type":3,"data":{"custom_id":"b","component_type":2}}"#,
                "component",
            ),
            (r#"{"data":{"custom_id":"m"},"type":5}"#, "modal"),
            (r#"{"type":5,"data":{"custom_id":"m"}}"#, "modal"),
            (r#"{"data":{"x":1},"type":99}"#, "unknown"),
            (r#"{"type":99,"data":{"x":1}}"#, "unknown"),
            (r#"{"data":{"x":1},"type":1}"#, "none"),
            (r#"{"type":1,"data":{"x":1}}"#, "none"),
            (r#"{"type":2,"data":null}"#, "none"),
        ];
        for (body, want) in cases {
            let json = format!(
                r#"{{"id":"1","application_id":"2","token":"t","extra":[1],{}"#,
                &body[1..]
            );
            let interaction: Interaction = serde_json::from_str(&json).unwrap();
            let got = match interaction.data {
                Some(InteractionData::Command(_)) => "command",
                Some(InteractionData::MessageComponent(_)) => "component",
                Some(InteractionData::ModalSubmit(_)) => "modal",
                Some(InteractionData::Unknown(_)) => "unknown",
                None => "none",
            };
            assert_eq!(got, want, "{json}");
            assert_eq!(interaction.version, 1);
        }
        for missing in [
            r#"{"application_id":"2","type":1,"token":"t"}"#,
            r#"{"id":"1","type":1,"token":"t"}"#,
            r#"{"id":"1","application_id":"2","token":"t"}"#,
            r#"{"id":"1","application_id":"2","type":1}"#,
            r#"{"id":"1","application_id":"2","token":"t","data":{"name":1},"type":2}"#,
            r#"{"id":"1","application_id":"2","token":"t","data":{},"type":3}"#,
            r#"{"id":"1","application_id":"2","token":"t","data":{},"type":5}"#,
            "[1]",
        ] {
            assert!(
                serde_json::from_str::<Interaction>(missing).is_err(),
                "{missing}"
            );
        }
    }

    #[test]
    fn callback_data_preserves_explicit_empty_arrays_on_round_trip() {
        let input = serde_json::json!({
            "type": 7,
            "data": {
                "content": "This form is closed.",
                "components": [],
                "embeds": [],
                "attachments": []
            }
        });
        let resp: InteractionResponse = serde_json::from_value(input.clone()).unwrap();
        let output = serde_json::to_value(&resp).unwrap();
        assert_eq!(output, input);

        // Omitted arrays stay omitted on round-trip.
        let omitted = serde_json::json!({
            "type": 7,
            "data": { "content": "Only text." }
        });
        let resp2: InteractionResponse = serde_json::from_value(omitted.clone()).unwrap();
        assert_eq!(serde_json::to_value(&resp2).unwrap(), omitted);
    }
}
