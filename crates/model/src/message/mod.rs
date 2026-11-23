//! Messages (resources/message.mdx, "Message Object").

pub mod allowed_mentions;
pub mod attachment;
pub mod embed;

use serde::{Deserialize, Serialize};

pub use allowed_mentions::{AllowedMentionType, AllowedMentions};
pub use attachment::{Attachment, AttachmentFlags};
pub use embed::{
    AUTHOR_LIMIT, DESCRIPTION_LIMIT, Embed, EmbedAuthor, EmbedError, EmbedField, EmbedFlags,
    EmbedFooter, EmbedMedia, EmbedMediaFlags, EmbedProvider, EmbedType, EmbedVideo,
    FIELD_COUNT_LIMIT, FIELD_NAME_LIMIT, FIELD_VALUE_LIMIT, FOOTER_LIMIT, TITLE_LIMIT, TOTAL_LIMIT,
};

use crate::application::Application;
use crate::channel::{Channel, ChannelType};
use crate::component::Component;
use crate::emoji::Emoji;
use crate::guild::Member;
use crate::id::Id;
use crate::id::marker::{
    ApplicationMarker, ChannelMarker, GuildMarker, LobbyMarker, MessageMarker, RoleMarker,
    SkuMarker, UserMarker, WebhookMarker,
};
use crate::interaction::{InteractionMetadata, MessageInteraction, Resolved};
use crate::poll::Poll;
use crate::serde_util::{bitflags_serde_int, int_enum};
use crate::sticker::{Sticker, StickerItem};
use crate::timestamp::Timestamp;
use crate::user::User;

/// A message.
///
/// List fields default to empty, so partial messages (interaction resolved
/// data, `MESSAGE_UPDATE`) parse as well.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Message {
    /// Message ID.
    pub id: Id<MessageMarker>,
    /// Channel.
    pub channel_id: Id<ChannelMarker>,
    /// Lobby this message belongs to (Social SDK linked-channel messages).
    pub lobby_id: Option<Id<LobbyMarker>>,
    /// Channel object with recipient information (Social SDK DM webhook events).
    pub channel: Option<Box<Channel>>,
    /// Guild (gateway events only).
    pub guild_id: Option<Id<GuildMarker>>,
    /// Author; for webhooks the webhook's name and avatar.
    pub author: User,
    /// Member of the author (`MESSAGE_CREATE`/`UPDATE` in guilds, without `user`).
    pub member: Option<Box<Member>>,
    /// Content (empty without the `MESSAGE_CONTENT` intent, except DMs,
    /// mentions and the bot's own messages).
    #[serde(default)]
    pub content: Box<str>,
    /// When the message was sent.
    pub timestamp: Timestamp,
    /// When the message was edited.
    pub edited_timestamp: Option<Timestamp>,
    /// Text-to-speech message.
    #[serde(default)]
    pub tts: bool,
    /// Mentions `@everyone`/`@here`.
    #[serde(default)]
    pub mention_everyone: bool,
    /// Mentioned users (with partial `member` in gateway events).
    #[serde(default)]
    pub mentions: Box<[MentionedUser]>,
    /// Mentioned roles.
    #[serde(default)]
    pub mention_roles: Box<[Id<RoleMarker>]>,
    /// Mentioned channels (crossposts only).
    #[serde(default)]
    pub mention_channels: Box<[ChannelMention]>,
    /// Attachments.
    #[serde(default)]
    pub attachments: Box<[Attachment]>,
    /// Embeds.
    #[serde(default)]
    pub embeds: Box<[Embed]>,
    /// Reactions.
    #[serde(default)]
    pub reactions: Box<[Reaction]>,
    /// Nonce for verifying a sent message.
    pub nonce: Option<Nonce>,
    /// Whether the message is pinned.
    #[serde(default)]
    pub pinned: bool,
    /// Webhook that sent the message.
    pub webhook_id: Option<Id<WebhookMarker>>,
    /// Message type.
    #[serde(rename = "type")]
    pub kind: MessageType,
    /// Rich Presence activity.
    pub activity: Option<Box<MessageActivity>>,
    /// Rich Presence application (partial).
    pub application: Option<Box<Application>>,
    /// Application of an interaction or webhook message.
    pub application_id: Option<Id<ApplicationMarker>>,
    /// Flags.
    #[serde(default)]
    pub flags: MessageFlags,
    /// Source of a reply, crosspost, pin, forward, …
    pub message_reference: Option<Box<MessageReference>>,
    /// Forwarded message content.
    #[serde(default)]
    pub message_snapshots: Box<[MessageSnapshot]>,
    /// The replied-to message; `None` inside if it was deleted.
    pub referenced_message: Option<Box<Message>>,
    /// Interaction that created the message.
    pub interaction_metadata: Option<Box<InteractionMetadata>>,
    /// Deprecated; use `interaction_metadata`.
    pub interaction: Option<Box<MessageInteraction>>,
    /// Thread started from this message.
    pub thread: Option<Box<Channel>>,
    /// Components.
    #[serde(default)]
    pub components: Box<[Component]>,
    /// Stickers.
    #[serde(default)]
    pub sticker_items: Box<[StickerItem]>,
    /// Deprecated full stickers.
    #[serde(default)]
    pub stickers: Box<[Sticker]>,
    /// Approximate position in a thread.
    pub position: Option<u32>,
    /// Type of the channel the message was sent in (`MESSAGE_CREATE` only).
    #[serde(default)]
    pub channel_type: Option<crate::channel::ChannelType>,
    /// Role subscription purchase/renewal data.
    pub role_subscription_data: Option<Box<RoleSubscriptionData>>,
    /// Users, members, channels and roles referenced in components.
    pub resolved: Option<Box<Resolved>>,
    /// Poll.
    pub poll: Option<Box<Poll>>,
    /// Call in a DM.
    pub call: Option<Box<MessageCall>>,
    /// Shared client theme.
    pub shared_client_theme: Option<Box<SharedClientTheme>>,
}

impl Message {
    /// Link to the message (`@me` for DMs).
    pub fn link(&self) -> String {
        match self.guild_id {
            Some(guild) => format!(
                "https://discord.com/channels/{guild}/{}/{}",
                self.channel_id, self.id
            ),
            None => format!(
                "https://discord.com/channels/@me/{}/{}",
                self.channel_id, self.id
            ),
        }
    }

    /// Whether the message uses Components V2.
    pub const fn is_components_v2(&self) -> bool {
        self.flags.contains(MessageFlags::IS_COMPONENTS_V2)
    }
}

/// A mentioned user; in gateway events with a partial `member`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct MentionedUser {
    /// The user.
    #[serde(flatten)]
    pub user: User,
    /// Partial member (gateway events in guilds).
    pub member: Option<Box<Member>>,
}

impl<'de> Deserialize<'de> for MentionedUser {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Extra {
            #[serde(default)]
            member: Option<Box<Member>>,
        }
        let (user, extra): (User, Extra) = crate::serde_util::split_flatten(d)?;
        Ok(Self {
            user,
            member: extra.member,
        })
    }
}

impl TryFrom<crate::serde_util::Scalar> for Nonce {
    type Error = &'static str;

    fn try_from(value: crate::serde_util::Scalar) -> Result<Self, Self::Error> {
        use crate::serde_util::Scalar;
        match value {
            Scalar::Int(i) => Ok(Self::Integer(i)),
            Scalar::Str(s) => Ok(Self::String(s)),
            Scalar::Bool(_) | Scalar::Float(_) => Err("a nonce is an integer or a string"),
        }
    }
}

impl<'de> Deserialize<'de> for Nonce {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        d.deserialize_any(crate::serde_util::ScalarVisitor(
            "an integer or string",
            std::marker::PhantomData,
        ))
    }
}

/// Message nonce: number or string.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum Nonce {
    /// Integer nonce.
    Integer(i64),
    /// String nonce (max. 25 characters).
    String(Box<str>),
}

int_enum! {
    /// Message type.
    pub enum MessageType: u8 {
        /// Normal message.
        Default = 0,
        /// Recipient added to a group DM.
        RecipientAdd = 1,
        /// Recipient removed from a group DM.
        RecipientRemove = 2,
        /// Call.
        Call = 3,
        /// Channel name changed.
        ChannelNameChange = 4,
        /// Channel icon changed.
        ChannelIconChange = 5,
        /// Message pinned.
        ChannelPinnedMessage = 6,
        /// Member joined.
        UserJoin = 7,
        /// Server boosted.
        GuildBoost = 8,
        /// Boost tier 1 reached.
        GuildBoostTier1 = 9,
        /// Boost tier 2 reached.
        GuildBoostTier2 = 10,
        /// Boost tier 3 reached.
        GuildBoostTier3 = 11,
        /// Channel followed.
        ChannelFollowAdd = 12,
        /// Discovery disqualified.
        GuildDiscoveryDisqualified = 14,
        /// Discovery requalified.
        GuildDiscoveryRequalified = 15,
        /// Discovery grace period initial warning.
        GuildDiscoveryGracePeriodInitialWarning = 16,
        /// Discovery grace period final warning.
        GuildDiscoveryGracePeriodFinalWarning = 17,
        /// Thread created.
        ThreadCreated = 18,
        /// Reply.
        Reply = 19,
        /// Slash command response.
        ChatInputCommand = 20,
        /// Thread starter message.
        ThreadStarterMessage = 21,
        /// Invite reminder.
        GuildInviteReminder = 22,
        /// Context menu command response.
        ContextMenuCommand = 23,
        /// Auto moderation action.
        AutoModerationAction = 24,
        /// Role subscription purchase.
        RoleSubscriptionPurchase = 25,
        /// Premium upsell.
        InteractionPremiumUpsell = 26,
        /// Stage started.
        StageStart = 27,
        /// Stage ended.
        StageEnd = 28,
        /// Stage speaker.
        StageSpeaker = 29,
        /// Stage topic.
        StageTopic = 31,
        /// Application premium subscription.
        GuildApplicationPremiumSubscription = 32,
        /// Incident alert mode enabled.
        GuildIncidentAlertModeEnabled = 36,
        /// Incident alert mode disabled.
        GuildIncidentAlertModeDisabled = 37,
        /// Raid reported.
        GuildIncidentReportRaid = 38,
        /// Raid report was a false alarm.
        GuildIncidentReportFalseAlarm = 39,
        /// Purchase notification.
        PurchaseNotification = 44,
        /// Poll result.
        PollResult = 46,
    }
}

impl MessageType {
    /// Whether messages of this type can be deleted (docs "Deletable"
    /// column; `AutoModerationAction` only with `MANAGE_MESSAGES`).
    pub const fn is_deletable(self) -> bool {
        !matches!(
            self,
            Self::RecipientAdd
                | Self::RecipientRemove
                | Self::Call
                | Self::ChannelNameChange
                | Self::ChannelIconChange
                | Self::ThreadStarterMessage
        )
    }
}

bitflags::bitflags! {
    /// Message flags.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
    pub struct MessageFlags: u64 {
        /// Published to following channels.
        const CROSSPOSTED = 1 << 0;
        /// Crosspost from another channel.
        const IS_CROSSPOST = 1 << 1;
        /// No embeds.
        const SUPPRESS_EMBEDS = 1 << 2;
        /// Source of the crosspost was deleted.
        const SOURCE_MESSAGE_DELETED = 1 << 3;
        /// From the urgent message system.
        const URGENT = 1 << 4;
        /// Has a thread with the same ID.
        const HAS_THREAD = 1 << 5;
        /// Only visible to the invoking user.
        const EPHEMERAL = 1 << 6;
        /// Deferred interaction response ("thinking").
        const LOADING = 1 << 7;
        /// Some roles could not be mentioned in a thread.
        const FAILED_TO_MENTION_SOME_ROLES_IN_THREAD = 1 << 8;
        /// No push/desktop notifications.
        const SUPPRESS_NOTIFICATIONS = 1 << 12;
        /// Voice message.
        const IS_VOICE_MESSAGE = 1 << 13;
        /// Has a forwarded snapshot.
        const HAS_SNAPSHOT = 1 << 14;
        /// Uses Components V2 (no `content`/`embeds`, layout components allowed).
        const IS_COMPONENTS_V2 = 1 << 15;
    }
}

bitflags_serde_int!(MessageFlags, u64);

/// Rich Presence activity of a message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct MessageActivity {
    /// Activity type.
    #[serde(rename = "type")]
    pub kind: MessageActivityType,
    /// Party ID.
    pub party_id: Option<Box<str>>,
}

int_enum! {
    /// Message activity type.
    pub enum MessageActivityType: u8 {
        /// Join.
        Join = 1,
        /// Spectate.
        Spectate = 2,
        /// Listen.
        Listen = 3,
        /// Join request.
        JoinRequest = 5,
        /// Stream request.
        StreamRequest = 6,
    }
}

/// Call in a DM.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct MessageCall {
    /// Participants.
    #[serde(default)]
    pub participants: Vec<Id<UserMarker>>,
    /// When the call ended.
    pub ended_timestamp: Option<Timestamp>,
}

/// Reference to another message (reply, forward, crosspost, pin, …).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct MessageReference {
    /// Reference type (default `Default` = reply-like).
    #[serde(
        rename = "type",
        default = "MessageReferenceType::default_value",
        skip_serializing_if = "MessageReferenceType::is_default"
    )]
    pub kind: MessageReferenceType,
    /// Referenced message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message_id: Option<Id<MessageMarker>>,
    /// Channel of the referenced message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel_id: Option<Id<ChannelMarker>>,
    /// Guild of the referenced message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guild_id: Option<Id<GuildMarker>>,
    /// When sending: fail if the message does not exist (default true).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fail_if_not_exists: Option<bool>,
}

impl MessageReference {
    /// Reply to a message.
    pub const fn reply(message_id: Id<MessageMarker>) -> Self {
        Self {
            kind: MessageReferenceType::Default,
            message_id: Some(message_id),
            channel_id: None,
            guild_id: None,
            fail_if_not_exists: None,
        }
    }

    /// Forward a message from a channel.
    pub const fn forward(channel_id: Id<ChannelMarker>, message_id: Id<MessageMarker>) -> Self {
        Self {
            kind: MessageReferenceType::Forward,
            message_id: Some(message_id),
            channel_id: Some(channel_id),
            guild_id: None,
            fail_if_not_exists: None,
        }
    }
}

int_enum! {
    /// Message reference type.
    pub enum MessageReferenceType: u8 {
        /// Standard reference (reply, crosspost, pin, …).
        Default = 0,
        /// Forward.
        Forward = 1,
    }
}

impl MessageReferenceType {
    const fn default_value() -> Self {
        Self::Default
    }

    #[allow(clippy::trivially_copy_pass_by_ref)] // serde passes fields by reference
    fn is_default(&self) -> bool {
        *self == Self::Default
    }
}

/// Forwarded message content.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct MessageSnapshot {
    /// Subset of the forwarded message.
    pub message: SnapshotMessage,
}

/// Fields of a forwarded message that a snapshot contains.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SnapshotMessage {
    /// Message type.
    #[serde(rename = "type")]
    pub kind: MessageType,
    /// Content.
    #[serde(default)]
    pub content: Box<str>,
    /// Embeds.
    #[serde(default)]
    pub embeds: Vec<Embed>,
    /// Attachments.
    #[serde(default)]
    pub attachments: Vec<Attachment>,
    /// When the original was sent.
    pub timestamp: Timestamp,
    /// When the original was edited.
    pub edited_timestamp: Option<Timestamp>,
    /// Flags.
    #[serde(default)]
    pub flags: MessageFlags,
    /// Mentioned users.
    #[serde(default)]
    pub mentions: Vec<User>,
    /// Mentioned roles.
    #[serde(default)]
    pub mention_roles: Vec<Id<RoleMarker>>,
    /// Deprecated full stickers.
    #[serde(default)]
    pub stickers: Vec<Sticker>,
    /// Stickers.
    #[serde(default)]
    pub sticker_items: Vec<StickerItem>,
    /// Components.
    #[serde(default)]
    pub components: Vec<Component>,
}

int_enum! {
    /// Reaction type (`MESSAGE_REACTION_*` events, Get Reactions).
    pub enum ReactionType: u8 {
        /// Normal reaction.
        Normal = 0,
        /// Super reaction (burst).
        Burst = 1,
    }
}

impl ReactionType {
    pub(crate) const fn default_normal() -> Self {
        Self::Normal
    }
}

/// A reaction on a message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Reaction {
    /// Total count (normal + super).
    pub count: u32,
    /// Counts by kind.
    pub count_details: ReactionCountDetails,
    /// Whether the current user reacted.
    #[serde(default)]
    pub me: bool,
    /// Whether the current user super-reacted.
    #[serde(default)]
    pub me_burst: bool,
    /// Emoji (partial).
    pub emoji: Emoji,
    /// HEX colors of the super reaction.
    #[serde(default)]
    pub burst_colors: Vec<Box<str>>,
}

/// Reaction counts by kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ReactionCountDetails {
    /// Super reactions.
    pub burst: u32,
    /// Normal reactions.
    pub normal: u32,
}

/// Channel mentioned in a crosspost.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ChannelMention {
    /// Channel ID.
    pub id: Id<ChannelMarker>,
    /// Guild ID.
    pub guild_id: Id<GuildMarker>,
    /// Channel type.
    #[serde(rename = "type")]
    pub kind: ChannelType,
    /// Channel name.
    pub name: Box<str>,
}

/// Role subscription purchase or renewal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct RoleSubscriptionData {
    /// SKU and listing.
    pub role_subscription_listing_id: Id<SkuMarker>,
    /// Tier name.
    pub tier_name: Box<str>,
    /// Months subscribed in total.
    pub total_months_subscribed: u32,
    /// Whether this is a renewal.
    pub is_renewal: bool,
}

/// A pinned message (`GET /channels/{id}/messages/pins`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct MessagePin {
    /// When it was pinned.
    pub pinned_at: Timestamp,
    /// The message.
    pub message: Message,
}

/// Page of channel pins.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ChannelPins {
    /// Pins in this page.
    pub items: Vec<MessagePin>,
    /// Whether another page exists.
    pub has_more: bool,
}

/// Custom client theme shared in a message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SharedClientTheme {
    /// HEX colors (max. 5).
    pub colors: Vec<Box<str>>,
    /// Gradient direction (0–360).
    pub gradient_angle: u16,
    /// Color intensity (0–100).
    pub base_mix: u8,
    /// Base mode.
    pub base_theme: Option<BaseTheme>,
}

int_enum! {
    /// Base theme of a shared client theme.
    pub enum BaseTheme: u8 {
        /// Unset.
        Unset = 0,
        /// Dark.
        Dark = 1,
        /// Light.
        Light = 2,
        /// Darker.
        Darker = 3,
        /// Midnight.
        Midnight = 4,
    }
}
