//! Dispatch events (opcode 0).
//!
//! Typed variants are added per phase as their models exist; every other
//! event arrives as [`Event::Unknown`] with its raw JSON, so a new Discord
//! event never breaks a shard.

use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;

use crate::application::Application;
use crate::audit_log::AuditLogEntry;
use crate::auto_moderation::{AutoModerationActionExecution, AutoModerationRule};
use crate::channel::{Channel, ThreadMember};
use crate::command::GuildCommandPermissions;
use crate::emoji::Emoji;
use crate::entitlement::Entitlement;
use crate::gateway::presence::PresenceUpdate;
use crate::guild::Integration;
use crate::guild::{Guild, Member, Role, UnavailableGuild};
use crate::id::Id;
use crate::id::marker::{ApplicationMarker, IntegrationMarker};
use crate::id::marker::{ChannelMarker, GuildMarker, MessageMarker, RoleMarker, UserMarker};
use crate::interaction::Interaction;
use crate::invite::{InviteCreate, InviteDelete};
use crate::message::{Message, ReactionType};
use crate::poll::MessagePollVote;
use crate::scheduled_event::{GuildScheduledEventUserAdd, ScheduledEvent};
use crate::sku::Subscription;
use crate::soundboard::{
    GuildSoundboardSoundDelete, SoundboardSound, SoundboardSounds, VoiceChannelEffectSend,
};
use crate::stage::StageInstance;
use crate::sticker::Sticker;
use crate::timestamp::Timestamp;
use crate::user::User;
use crate::voice::VoiceState;

/// A dispatched gateway event.
///
/// Large payloads are boxed so the enum stays small (events travel through
/// channels).
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum Event {
    /// `READY`: the session is established.
    Ready(Box<Ready>),
    /// `RESUMED`: a resume finished; missed events were replayed before it.
    Resumed,
    /// `GUILD_CREATE`: available guild with extra fields, or an unavailable stub.
    GuildCreate(GuildCreate),
    /// `GUILD_UPDATE`.
    GuildUpdate(Box<Guild>),
    /// `GUILD_DELETE`: guild unavailable (`unavailable: true`) or the bot
    /// was removed (`unavailable: false`).
    GuildDelete(UnavailableGuild),
    /// `GUILD_BAN_ADD`.
    GuildBanAdd(Box<GuildBanAdd>),
    /// `GUILD_BAN_REMOVE`.
    GuildBanRemove(Box<GuildBanAdd>),
    /// `GUILD_EMOJIS_UPDATE`.
    GuildEmojisUpdate(Box<GuildEmojisUpdate>),
    /// `GUILD_STICKERS_UPDATE`.
    GuildStickersUpdate(Box<GuildStickersUpdate>),
    /// `GUILD_INTEGRATIONS_UPDATE`.
    GuildIntegrationsUpdate(GuildIntegrationsUpdate),
    /// `GUILD_MEMBER_ADD`.
    GuildMemberAdd(Box<Member>),
    /// `GUILD_MEMBER_REMOVE`.
    GuildMemberRemove(Box<GuildMemberRemove>),
    /// `GUILD_MEMBER_UPDATE`.
    GuildMemberUpdate(Box<GuildMemberUpdate>),
    /// `GUILD_MEMBERS_CHUNK`.
    GuildMembersChunk(Box<GuildMembersChunk>),
    /// `GUILD_ROLE_CREATE`.
    GuildRoleCreate(Box<GuildRoleCreate>),
    /// `GUILD_ROLE_UPDATE`.
    GuildRoleUpdate(Box<GuildRoleCreate>),
    /// `GUILD_ROLE_DELETE`.
    GuildRoleDelete(GuildRoleDelete),
    /// `CHANNEL_CREATE`.
    ChannelCreate(Box<Channel>),
    /// `CHANNEL_UPDATE`.
    ChannelUpdate(Box<Channel>),
    /// `CHANNEL_DELETE`.
    ChannelDelete(Box<Channel>),
    /// `CHANNEL_PINS_UPDATE`.
    ChannelPinsUpdate(ChannelPinsUpdate),
    /// `THREAD_CREATE` (channel; may include `newly_created` and `member`).
    ThreadCreate(Box<Channel>),
    /// `THREAD_UPDATE`.
    ThreadUpdate(Box<Channel>),
    /// `THREAD_DELETE` (partial channel: `id`, `guild_id`, `parent_id`, `type`).
    ThreadDelete(Box<Channel>),
    /// `THREAD_LIST_SYNC`.
    ThreadListSync(Box<ThreadListSync>),
    /// `THREAD_MEMBER_UPDATE`.
    ThreadMemberUpdate(Box<ThreadMember>),
    /// `THREAD_MEMBERS_UPDATE`.
    ThreadMembersUpdate(Box<ThreadMembersUpdate>),
    /// `MESSAGE_CREATE` (with `guild_id` and `member` in guilds).
    MessageCreate(Box<Message>),
    /// `MESSAGE_UPDATE` (full message; `tts` is always false).
    MessageUpdate(Box<Message>),
    /// `MESSAGE_DELETE`.
    MessageDelete(MessageDelete),
    /// `MESSAGE_DELETE_BULK`.
    MessageDeleteBulk(MessageDeleteBulk),
    /// `MESSAGE_REACTION_ADD`.
    MessageReactionAdd(Box<MessageReactionAdd>),
    /// `MESSAGE_REACTION_REMOVE`.
    MessageReactionRemove(Box<MessageReactionRemove>),
    /// `MESSAGE_REACTION_REMOVE_ALL`.
    MessageReactionRemoveAll(MessageReactionRemoveAll),
    /// `MESSAGE_REACTION_REMOVE_EMOJI`.
    MessageReactionRemoveEmoji(Box<MessageReactionRemoveEmoji>),
    /// `INTERACTION_CREATE`.
    InteractionCreate(Box<Interaction>),
    /// `APPLICATION_COMMAND_PERMISSIONS_UPDATE`.
    ApplicationCommandPermissionsUpdate(Box<GuildCommandPermissions>),
    /// `VOICE_STATE_UPDATE`.
    VoiceStateUpdate(Box<VoiceState>),
    /// `VOICE_SERVER_UPDATE`.
    VoiceServerUpdate(VoiceServerUpdate),
    /// `ENTITLEMENT_CREATE`.
    EntitlementCreate(Box<Entitlement>),
    /// `ENTITLEMENT_UPDATE`.
    EntitlementUpdate(Box<Entitlement>),
    /// `ENTITLEMENT_DELETE`.
    EntitlementDelete(Box<Entitlement>),
    /// `USER_UPDATE`.
    UserUpdate(Box<User>),
    /// `WEBHOOKS_UPDATE`.
    WebhooksUpdate(WebhooksUpdate),
    /// `AUTO_MODERATION_RULE_CREATE`.
    AutoModerationRuleCreate(Box<AutoModerationRule>),
    /// `AUTO_MODERATION_RULE_UPDATE`.
    AutoModerationRuleUpdate(Box<AutoModerationRule>),
    /// `AUTO_MODERATION_RULE_DELETE`.
    AutoModerationRuleDelete(Box<AutoModerationRule>),
    /// `AUTO_MODERATION_ACTION_EXECUTION`.
    AutoModerationActionExecution(Box<AutoModerationActionExecution>),
    /// `GUILD_SCHEDULED_EVENT_CREATE`.
    GuildScheduledEventCreate(Box<ScheduledEvent>),
    /// `GUILD_SCHEDULED_EVENT_UPDATE`.
    GuildScheduledEventUpdate(Box<ScheduledEvent>),
    /// `GUILD_SCHEDULED_EVENT_DELETE`.
    GuildScheduledEventDelete(Box<ScheduledEvent>),
    /// `GUILD_SCHEDULED_EVENT_USER_ADD`.
    GuildScheduledEventUserAdd(GuildScheduledEventUserAdd),
    /// `GUILD_SCHEDULED_EVENT_USER_REMOVE`.
    GuildScheduledEventUserRemove(GuildScheduledEventUserAdd),
    /// `INVITE_CREATE`.
    InviteCreate(Box<InviteCreate>),
    /// `INVITE_DELETE`.
    InviteDelete(InviteDelete),
    /// `STAGE_INSTANCE_CREATE`.
    StageInstanceCreate(Box<StageInstance>),
    /// `STAGE_INSTANCE_UPDATE`.
    StageInstanceUpdate(Box<StageInstance>),
    /// `STAGE_INSTANCE_DELETE`.
    StageInstanceDelete(Box<StageInstance>),
    /// `GUILD_SOUNDBOARD_SOUND_CREATE`.
    GuildSoundboardSoundCreate(Box<SoundboardSound>),
    /// `GUILD_SOUNDBOARD_SOUND_UPDATE`.
    GuildSoundboardSoundUpdate(Box<SoundboardSound>),
    /// `GUILD_SOUNDBOARD_SOUND_DELETE`.
    GuildSoundboardSoundDelete(GuildSoundboardSoundDelete),
    /// `GUILD_SOUNDBOARD_SOUNDS_UPDATE`.
    GuildSoundboardSoundsUpdate(Box<SoundboardSounds>),
    /// `SOUNDBOARD_SOUNDS` (response to Request Soundboard Sounds).
    SoundboardSounds(Box<SoundboardSounds>),
    /// `PRESENCE_UPDATE`.
    PresenceUpdate(Box<PresenceUpdate>),
    /// `TYPING_START`.
    TypingStart(Box<TypingStart>),
    /// `GUILD_AUDIT_LOG_ENTRY_CREATE`.
    GuildAuditLogEntryCreate(Box<AuditLogEntry>),
    /// `MESSAGE_POLL_VOTE_ADD`.
    MessagePollVoteAdd(MessagePollVote),
    /// `MESSAGE_POLL_VOTE_REMOVE`.
    MessagePollVoteRemove(MessagePollVote),
    /// `VOICE_CHANNEL_EFFECT_SEND`.
    VoiceChannelEffectSend(Box<VoiceChannelEffectSend>),
    /// `VOICE_CHANNEL_STATUS_UPDATE`.
    VoiceChannelStatusUpdate(Box<VoiceChannelStatusUpdate>),
    /// `VOICE_CHANNEL_START_TIME_UPDATE`.
    VoiceChannelStartTimeUpdate(VoiceChannelStartTimeUpdate),
    /// `CHANNEL_INFO` (answer to Request Channel Info, opcode 43).
    ChannelInfo(Box<ChannelInfo>),
    /// `INTEGRATION_CREATE`.
    IntegrationCreate(Box<IntegrationEvent>),
    /// `INTEGRATION_UPDATE`.
    IntegrationUpdate(Box<IntegrationEvent>),
    /// `INTEGRATION_DELETE`.
    IntegrationDelete(IntegrationDelete),
    /// `SUBSCRIPTION_CREATE`.
    SubscriptionCreate(Box<Subscription>),
    /// `SUBSCRIPTION_UPDATE`.
    SubscriptionUpdate(Box<Subscription>),
    /// `SUBSCRIPTION_DELETE`.
    SubscriptionDelete(Box<Subscription>),
    /// `RATE_LIMITED`: a gateway request (e.g. Request Guild Members) was
    /// rate limited.
    RateLimited(Box<RateLimited>),
    /// An event without a typed variant (yet).
    Unknown(UnknownEvent),
}

/// `READY` payload (events/gateway-events.mdx, "Ready").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Ready {
    /// API version.
    pub v: u8,
    /// The bot user.
    pub user: User,
    /// Guilds of this shard, all unavailable until their `GUILD_CREATE`.
    pub guilds: Vec<UnavailableGuild>,
    /// Session ID for resuming.
    pub session_id: Box<str>,
    /// Gateway URL for resuming.
    pub resume_gateway_url: Box<str>,
    /// `[shard_id, num_shards]`.
    pub shard: Option<[u32; 2]>,
    /// The application (`id`, `flags`, `flags_new`).
    pub application: Application,
}

/// `MESSAGE_DELETE` payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct MessageDelete {
    /// Message.
    pub id: Id<MessageMarker>,
    /// Channel.
    pub channel_id: Id<ChannelMarker>,
    /// Guild.
    pub guild_id: Option<Id<GuildMarker>>,
}

/// `MESSAGE_DELETE_BULK` payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct MessageDeleteBulk {
    /// Messages.
    pub ids: Vec<Id<MessageMarker>>,
    /// Channel.
    pub channel_id: Id<ChannelMarker>,
    /// Guild.
    pub guild_id: Option<Id<GuildMarker>>,
}

/// `GUILD_CREATE` payload: an available guild with extra fields, or an
/// unavailable stub (outage while connecting).
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum GuildCreate {
    /// Guild is available; includes members, channels and voice states.
    Available(Box<AvailableGuild>),
    /// Guild is unavailable; wait for a later `GUILD_CREATE`.
    Unavailable(UnavailableGuild),
}

/// Available `GUILD_CREATE` payload: a guild plus the extra gateway fields.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct AvailableGuild {
    /// Guild object.
    #[serde(flatten)]
    pub guild: Guild,
    /// When the bot joined.
    pub joined_at: Timestamp,
    /// Whether this is considered a large guild.
    #[serde(default)]
    pub large: bool,
    /// Whether the guild is unavailable due to an outage.
    #[serde(default)]
    pub unavailable: bool,
    /// Member count.
    #[serde(default)]
    pub member_count: u32,
    /// Voice states (no `guild_id` on each).
    #[serde(default)]
    pub voice_states: Vec<VoiceState>,
    /// Members (limited without `GUILD_PRESENCES` / in very large guilds).
    #[serde(default)]
    pub members: Vec<Member>,
    /// Channels.
    #[serde(default)]
    pub channels: Vec<Channel>,
    /// Active threads the bot can see.
    #[serde(default)]
    pub threads: Vec<Channel>,
    /// Scheduled events.
    #[serde(default)]
    pub guild_scheduled_events: Vec<ScheduledEvent>,
    /// Presences (partial; non-offline only above the large threshold).
    #[serde(default)]
    pub presences: Vec<PresenceUpdate>,
    /// Live stages.
    #[serde(default)]
    pub stage_instances: Vec<StageInstance>,
    /// Guild soundboard sounds.
    #[serde(default)]
    pub soundboard_sounds: Vec<SoundboardSound>,
}

/// `GUILD_BAN_ADD` / `GUILD_BAN_REMOVE` payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct GuildBanAdd {
    /// Guild.
    pub guild_id: Id<GuildMarker>,
    /// User who was banned or unbanned.
    pub user: User,
}

/// `GUILD_EMOJIS_UPDATE` payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct GuildEmojisUpdate {
    /// Guild.
    pub guild_id: Id<GuildMarker>,
    /// Emojis.
    #[serde(default)]
    pub emojis: Vec<Emoji>,
}

/// `GUILD_STICKERS_UPDATE` payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct GuildStickersUpdate {
    /// Guild.
    pub guild_id: Id<GuildMarker>,
    /// Stickers.
    #[serde(default)]
    pub stickers: Vec<Sticker>,
}

/// `VOICE_CHANNEL_STATUS_UPDATE` payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct VoiceChannelStatusUpdate {
    /// Channel.
    pub id: Id<ChannelMarker>,
    /// Guild.
    pub guild_id: Id<GuildMarker>,
    /// New status (`None` when cleared).
    pub status: Option<Box<str>>,
}

/// `VOICE_CHANNEL_START_TIME_UPDATE` payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct VoiceChannelStartTimeUpdate {
    /// Channel.
    pub id: Id<ChannelMarker>,
    /// Guild.
    pub guild_id: Id<GuildMarker>,
    /// Unix seconds when the voice session started (`None` when it ended).
    #[serde(default)]
    pub voice_start_time: Option<u64>,
}

/// `CHANNEL_INFO` payload: ephemeral data of a guild's channels.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ChannelInfo {
    /// Guild.
    pub guild_id: Id<GuildMarker>,
    /// Channels.
    #[serde(default)]
    pub channels: Vec<ChannelInfoChannel>,
}

/// One channel in [`ChannelInfo`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ChannelInfoChannel {
    /// Channel.
    pub id: Id<ChannelMarker>,
    /// Voice channel status.
    #[serde(default)]
    pub status: Option<Box<str>>,
    /// Unix seconds when the voice session started.
    #[serde(default)]
    pub voice_start_time: Option<u64>,
}

/// `INTEGRATION_CREATE` / `INTEGRATION_UPDATE` payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct IntegrationEvent {
    /// Guild.
    pub guild_id: Id<GuildMarker>,
    /// The integration.
    #[serde(flatten)]
    pub integration: Integration,
}

/// `INTEGRATION_DELETE` payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct IntegrationDelete {
    /// Integration.
    pub id: Id<IntegrationMarker>,
    /// Guild.
    pub guild_id: Id<GuildMarker>,
    /// Bot or `OAuth2` application of the integration.
    #[serde(default)]
    pub application_id: Option<Id<ApplicationMarker>>,
}

/// `RATE_LIMITED` payload.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct RateLimited {
    /// Opcode that was rate limited (8 = Request Guild Members).
    pub opcode: u8,
    /// Seconds to wait before retrying.
    pub retry_after: f64,
    /// Details of the limited request.
    #[serde(default)]
    pub meta: RateLimitedMeta,
}

/// Details of a rate limited request (for opcode 8: guild and nonce).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[non_exhaustive]
pub struct RateLimitedMeta {
    /// Guild of a Request Guild Members.
    #[serde(default)]
    pub guild_id: Option<Id<GuildMarker>>,
    /// Nonce of the request.
    #[serde(default)]
    pub nonce: Option<Box<str>>,
}

/// `GUILD_INTEGRATIONS_UPDATE` payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct GuildIntegrationsUpdate {
    /// Guild.
    pub guild_id: Id<GuildMarker>,
}

/// `GUILD_MEMBER_REMOVE` payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct GuildMemberRemove {
    /// Guild.
    pub guild_id: Id<GuildMarker>,
    /// User who left or was removed.
    pub user: User,
}

/// `GUILD_MEMBER_UPDATE` payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct GuildMemberUpdate {
    /// Guild.
    pub guild_id: Id<GuildMarker>,
    /// Role IDs.
    #[serde(default)]
    pub roles: Vec<Id<RoleMarker>>,
    /// User.
    pub user: User,
    /// Nickname.
    pub nick: Option<Box<str>>,
    /// Guild avatar hash.
    pub avatar: Option<crate::image_hash::ImageHash>,
    /// Guild banner hash.
    pub banner: Option<crate::image_hash::ImageHash>,
    /// When the user joined; `None` for guests.
    pub joined_at: Option<Timestamp>,
    /// When the user started boosting.
    pub premium_since: Option<Timestamp>,
    /// Server-deafened.
    pub deaf: Option<bool>,
    /// Server-muted.
    pub mute: Option<bool>,
    /// Membership screening pending.
    pub pending: Option<bool>,
    /// Timeout end.
    pub communication_disabled_until: Option<Timestamp>,
    /// Guild avatar decoration.
    pub avatar_decoration_data: Option<crate::user::AvatarDecorationData>,
    /// Guild collectibles.
    pub collectibles: Option<crate::user::Collectibles>,
    /// Member flags.
    #[serde(default)]
    pub flags: crate::guild::MemberFlags,
}

/// `GUILD_MEMBERS_CHUNK` payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct GuildMembersChunk {
    /// Guild.
    pub guild_id: Id<GuildMarker>,
    /// Members in this chunk.
    #[serde(default)]
    pub members: Vec<Member>,
    /// Chunk index (`0 <= chunk_index < chunk_count`).
    pub chunk_index: u32,
    /// Total chunks for this request.
    pub chunk_count: u32,
    /// Invalid user IDs from `REQUEST_GUILD_MEMBERS`.
    #[serde(default)]
    pub not_found: Vec<Id<UserMarker>>,
    /// Presences of the members (when requested with `presences: true`).
    #[serde(default)]
    pub presences: Vec<PresenceUpdate>,
    /// Nonce from the request.
    pub nonce: Option<Box<str>>,
}

/// `GUILD_ROLE_CREATE` / `GUILD_ROLE_UPDATE` payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct GuildRoleCreate {
    /// Guild.
    pub guild_id: Id<GuildMarker>,
    /// Role.
    pub role: Role,
}

/// `GUILD_ROLE_DELETE` payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct GuildRoleDelete {
    /// Guild.
    pub guild_id: Id<GuildMarker>,
    /// Role.
    pub role_id: Id<RoleMarker>,
}

/// `CHANNEL_PINS_UPDATE` payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ChannelPinsUpdate {
    /// Guild.
    pub guild_id: Option<Id<GuildMarker>>,
    /// Channel.
    pub channel_id: Id<ChannelMarker>,
    /// When the most recent pin was pinned.
    pub last_pin_timestamp: Option<Timestamp>,
}

/// `THREAD_LIST_SYNC` payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ThreadListSync {
    /// Guild.
    pub guild_id: Id<GuildMarker>,
    /// Parent channels whose threads are being synced; omitted = whole guild.
    #[serde(default)]
    pub channel_ids: Vec<Id<ChannelMarker>>,
    /// Active threads the bot can access.
    #[serde(default)]
    pub threads: Vec<Channel>,
    /// Thread members for the current user.
    #[serde(default)]
    pub members: Vec<ThreadMember>,
}

/// `THREAD_MEMBERS_UPDATE` payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ThreadMembersUpdate {
    /// Thread.
    pub id: Id<ChannelMarker>,
    /// Guild.
    pub guild_id: Id<GuildMarker>,
    /// Approximate member count (capped at 50).
    pub member_count: u32,
    /// Users added to the thread.
    #[serde(default)]
    pub added_members: Vec<ThreadMember>,
    /// Users removed from the thread.
    #[serde(default)]
    pub removed_member_ids: Vec<Id<UserMarker>>,
}

/// `MESSAGE_REACTION_ADD` payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct MessageReactionAdd {
    /// User who reacted.
    pub user_id: Id<UserMarker>,
    /// Channel.
    pub channel_id: Id<ChannelMarker>,
    /// Message.
    pub message_id: Id<MessageMarker>,
    /// Guild.
    pub guild_id: Option<Id<GuildMarker>>,
    /// Member who reacted (guilds).
    pub member: Option<Box<Member>>,
    /// Emoji.
    pub emoji: Emoji,
    /// Author of the message that was reacted to.
    pub message_author_id: Option<Id<UserMarker>>,
    /// Whether this is a super-reaction.
    #[serde(default)]
    pub burst: bool,
    /// Super-reaction colors (`#rrggbb`).
    #[serde(default)]
    pub burst_colors: Vec<Box<str>>,
    /// Reaction type.
    #[serde(rename = "type", default = "ReactionType::default_normal")]
    pub kind: ReactionType,
}

/// `MESSAGE_REACTION_REMOVE` payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct MessageReactionRemove {
    /// User who removed the reaction.
    pub user_id: Id<UserMarker>,
    /// Channel.
    pub channel_id: Id<ChannelMarker>,
    /// Message.
    pub message_id: Id<MessageMarker>,
    /// Guild.
    pub guild_id: Option<Id<GuildMarker>>,
    /// Emoji.
    pub emoji: Emoji,
    /// Whether this was a super-reaction.
    #[serde(default)]
    pub burst: bool,
    /// Reaction type.
    #[serde(rename = "type", default = "ReactionType::default_normal")]
    pub kind: ReactionType,
}

/// `MESSAGE_REACTION_REMOVE_ALL` payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct MessageReactionRemoveAll {
    /// Channel.
    pub channel_id: Id<ChannelMarker>,
    /// Message.
    pub message_id: Id<MessageMarker>,
    /// Guild.
    pub guild_id: Option<Id<GuildMarker>>,
}

/// `MESSAGE_REACTION_REMOVE_EMOJI` payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct MessageReactionRemoveEmoji {
    /// Channel.
    pub channel_id: Id<ChannelMarker>,
    /// Guild.
    pub guild_id: Option<Id<GuildMarker>>,
    /// Message.
    pub message_id: Id<MessageMarker>,
    /// Emoji that was removed.
    pub emoji: Emoji,
}

/// `VOICE_SERVER_UPDATE` payload. `Debug` redacts the voice token.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct VoiceServerUpdate {
    /// Voice connection token.
    pub token: Box<str>,
    /// Guild.
    pub guild_id: Id<GuildMarker>,
    /// Voice server host; `None` while Discord reallocates.
    pub endpoint: Option<Box<str>>,
}

impl std::fmt::Debug for VoiceServerUpdate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VoiceServerUpdate")
            .field("token", &"***")
            .field("guild_id", &self.guild_id)
            .field("endpoint", &self.endpoint)
            .finish()
    }
}

/// `WEBHOOKS_UPDATE` payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct WebhooksUpdate {
    /// Guild.
    pub guild_id: Id<GuildMarker>,
    /// Channel whose webhooks changed.
    pub channel_id: Id<ChannelMarker>,
}

/// `TYPING_START` payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct TypingStart {
    /// Channel.
    pub channel_id: Id<ChannelMarker>,
    /// Guild.
    pub guild_id: Option<Id<GuildMarker>>,
    /// User who started typing.
    pub user_id: Id<UserMarker>,
    /// Unix time in seconds.
    pub timestamp: u64,
    /// Member (guilds).
    pub member: Option<Member>,
}

/// Raw form of an event without a typed variant.
#[derive(Debug, Clone)]
pub struct UnknownEvent {
    /// Event name (`t`).
    pub name: Box<str>,
    /// Event data (`d`) as raw JSON.
    pub data: Box<RawValue>,
}

impl Event {
    /// Builds an event from a dispatch's name and data. Fails only if a
    /// known event has data that does not match its model.
    pub fn from_dispatch(name: &str, data: Option<&RawValue>) -> serde_json::Result<Self> {
        if let Some(event) = Self::typed(name, data) {
            return event;
        }
        let data = match data {
            Some(raw) => raw.to_owned(),
            None => RawValue::from_string("null".to_owned())?,
        };
        Ok(Self::Unknown(UnknownEvent {
            name: name.into(),
            data,
        }))
    }

    /// Typed events; `None` for names without a typed variant.
    #[allow(
        clippy::too_many_lines,
        reason = "one match arm per Discord dispatch event"
    )]
    fn typed(name: &str, data: Option<&RawValue>) -> Option<serde_json::Result<Self>> {
        fn parse<'a, T: Deserialize<'a>>(data: Option<&'a RawValue>) -> serde_json::Result<T> {
            serde_json::from_str(data.map_or("null", RawValue::get))
        }

        let event = match name {
            "READY" => parse(data).map(Self::Ready),
            "RESUMED" => Ok(Self::Resumed),
            "GUILD_CREATE" => {
                #[derive(Deserialize)]
                struct Peek {
                    joined_at: Option<Timestamp>,
                }
                let json = data.map_or("null", RawValue::get);
                match serde_json::from_str::<Peek>(json) {
                    Ok(peek) if peek.joined_at.is_some() => parse(data)
                        .map(GuildCreate::Available)
                        .map(Self::GuildCreate),
                    Ok(_) => parse(data)
                        .map(GuildCreate::Unavailable)
                        .map(Self::GuildCreate),
                    Err(err) => Err(err),
                }
            }
            "GUILD_UPDATE" => parse(data).map(Self::GuildUpdate),
            "GUILD_DELETE" => parse(data).map(Self::GuildDelete),
            "GUILD_BAN_ADD" => parse(data).map(Self::GuildBanAdd),
            "GUILD_BAN_REMOVE" => parse(data).map(Self::GuildBanRemove),
            "GUILD_EMOJIS_UPDATE" => parse(data).map(Self::GuildEmojisUpdate),
            "GUILD_STICKERS_UPDATE" => parse(data).map(Self::GuildStickersUpdate),
            "GUILD_INTEGRATIONS_UPDATE" => parse(data).map(Self::GuildIntegrationsUpdate),
            "GUILD_MEMBER_ADD" => parse(data).map(Self::GuildMemberAdd),
            "GUILD_MEMBER_REMOVE" => parse(data).map(Self::GuildMemberRemove),
            "GUILD_MEMBER_UPDATE" => parse(data).map(Self::GuildMemberUpdate),
            "GUILD_MEMBERS_CHUNK" => parse(data).map(Self::GuildMembersChunk),
            "GUILD_ROLE_CREATE" => parse(data).map(Self::GuildRoleCreate),
            "GUILD_ROLE_UPDATE" => parse(data).map(Self::GuildRoleUpdate),
            "GUILD_ROLE_DELETE" => parse(data).map(Self::GuildRoleDelete),
            "CHANNEL_CREATE" => parse(data).map(Self::ChannelCreate),
            "CHANNEL_UPDATE" => parse(data).map(Self::ChannelUpdate),
            "CHANNEL_DELETE" => parse(data).map(Self::ChannelDelete),
            "CHANNEL_PINS_UPDATE" => parse(data).map(Self::ChannelPinsUpdate),
            "THREAD_CREATE" => parse(data).map(Self::ThreadCreate),
            "THREAD_UPDATE" => parse(data).map(Self::ThreadUpdate),
            "THREAD_DELETE" => parse(data).map(Self::ThreadDelete),
            "THREAD_LIST_SYNC" => parse(data).map(Self::ThreadListSync),
            "THREAD_MEMBER_UPDATE" => parse(data).map(Self::ThreadMemberUpdate),
            "THREAD_MEMBERS_UPDATE" => parse(data).map(Self::ThreadMembersUpdate),
            "MESSAGE_CREATE" => parse(data).map(Self::MessageCreate),
            "MESSAGE_UPDATE" => parse(data).map(Self::MessageUpdate),
            "MESSAGE_DELETE" => parse(data).map(Self::MessageDelete),
            "MESSAGE_DELETE_BULK" => parse(data).map(Self::MessageDeleteBulk),
            "MESSAGE_REACTION_ADD" => parse(data).map(Self::MessageReactionAdd),
            "MESSAGE_REACTION_REMOVE" => parse(data).map(Self::MessageReactionRemove),
            "MESSAGE_REACTION_REMOVE_ALL" => parse(data).map(Self::MessageReactionRemoveAll),
            "MESSAGE_REACTION_REMOVE_EMOJI" => parse(data).map(Self::MessageReactionRemoveEmoji),
            "INTERACTION_CREATE" => parse(data).map(Self::InteractionCreate),
            "APPLICATION_COMMAND_PERMISSIONS_UPDATE" => {
                parse(data).map(Self::ApplicationCommandPermissionsUpdate)
            }
            "VOICE_STATE_UPDATE" => parse(data).map(Self::VoiceStateUpdate),
            "VOICE_SERVER_UPDATE" => parse(data).map(Self::VoiceServerUpdate),
            "ENTITLEMENT_CREATE" => parse(data).map(Self::EntitlementCreate),
            "ENTITLEMENT_UPDATE" => parse(data).map(Self::EntitlementUpdate),
            "ENTITLEMENT_DELETE" => parse(data).map(Self::EntitlementDelete),
            "USER_UPDATE" => parse(data).map(Self::UserUpdate),
            "WEBHOOKS_UPDATE" => parse(data).map(Self::WebhooksUpdate),
            "AUTO_MODERATION_RULE_CREATE" => parse(data).map(Self::AutoModerationRuleCreate),
            "AUTO_MODERATION_RULE_UPDATE" => parse(data).map(Self::AutoModerationRuleUpdate),
            "AUTO_MODERATION_RULE_DELETE" => parse(data).map(Self::AutoModerationRuleDelete),
            "AUTO_MODERATION_ACTION_EXECUTION" => {
                parse(data).map(Self::AutoModerationActionExecution)
            }
            "GUILD_SCHEDULED_EVENT_CREATE" => parse(data).map(Self::GuildScheduledEventCreate),
            "GUILD_SCHEDULED_EVENT_UPDATE" => parse(data).map(Self::GuildScheduledEventUpdate),
            "GUILD_SCHEDULED_EVENT_DELETE" => parse(data).map(Self::GuildScheduledEventDelete),
            "GUILD_SCHEDULED_EVENT_USER_ADD" => parse(data).map(Self::GuildScheduledEventUserAdd),
            "GUILD_SCHEDULED_EVENT_USER_REMOVE" => {
                parse(data).map(Self::GuildScheduledEventUserRemove)
            }
            "INVITE_CREATE" => parse(data).map(Self::InviteCreate),
            "INVITE_DELETE" => parse(data).map(Self::InviteDelete),
            "STAGE_INSTANCE_CREATE" => parse(data).map(Self::StageInstanceCreate),
            "STAGE_INSTANCE_UPDATE" => parse(data).map(Self::StageInstanceUpdate),
            "STAGE_INSTANCE_DELETE" => parse(data).map(Self::StageInstanceDelete),
            "GUILD_SOUNDBOARD_SOUND_CREATE" => parse(data).map(Self::GuildSoundboardSoundCreate),
            "GUILD_SOUNDBOARD_SOUND_UPDATE" => parse(data).map(Self::GuildSoundboardSoundUpdate),
            "GUILD_SOUNDBOARD_SOUND_DELETE" => parse(data).map(Self::GuildSoundboardSoundDelete),
            "GUILD_SOUNDBOARD_SOUNDS_UPDATE" => parse(data).map(Self::GuildSoundboardSoundsUpdate),
            "SOUNDBOARD_SOUNDS" => parse(data).map(Self::SoundboardSounds),
            "PRESENCE_UPDATE" => parse(data).map(Self::PresenceUpdate),
            "TYPING_START" => parse(data).map(Self::TypingStart),
            "GUILD_AUDIT_LOG_ENTRY_CREATE" => parse(data).map(Self::GuildAuditLogEntryCreate),
            "MESSAGE_POLL_VOTE_ADD" => parse(data).map(Self::MessagePollVoteAdd),
            "MESSAGE_POLL_VOTE_REMOVE" => parse(data).map(Self::MessagePollVoteRemove),
            "VOICE_CHANNEL_EFFECT_SEND" => parse(data).map(Self::VoiceChannelEffectSend),
            "VOICE_CHANNEL_STATUS_UPDATE" => parse(data).map(Self::VoiceChannelStatusUpdate),
            "VOICE_CHANNEL_START_TIME_UPDATE" => parse(data).map(Self::VoiceChannelStartTimeUpdate),
            "CHANNEL_INFO" => parse(data).map(Self::ChannelInfo),
            "INTEGRATION_CREATE" => parse(data).map(Self::IntegrationCreate),
            "INTEGRATION_UPDATE" => parse(data).map(Self::IntegrationUpdate),
            "INTEGRATION_DELETE" => parse(data).map(Self::IntegrationDelete),
            "SUBSCRIPTION_CREATE" => parse(data).map(Self::SubscriptionCreate),
            "SUBSCRIPTION_UPDATE" => parse(data).map(Self::SubscriptionUpdate),
            "SUBSCRIPTION_DELETE" => parse(data).map(Self::SubscriptionDelete),
            "RATE_LIMITED" => parse(data).map(Self::RateLimited),
            _ => return None,
        };
        Some(event)
    }

    /// The gateway name of the event (`t`).
    pub fn name(&self) -> &str {
        match self {
            Self::Ready(_) => "READY",
            Self::Resumed => "RESUMED",
            Self::GuildCreate(_) => "GUILD_CREATE",
            Self::GuildUpdate(_) => "GUILD_UPDATE",
            Self::GuildDelete(_) => "GUILD_DELETE",
            Self::GuildBanAdd(_) => "GUILD_BAN_ADD",
            Self::GuildBanRemove(_) => "GUILD_BAN_REMOVE",
            Self::GuildEmojisUpdate(_) => "GUILD_EMOJIS_UPDATE",
            Self::GuildStickersUpdate(_) => "GUILD_STICKERS_UPDATE",
            Self::GuildIntegrationsUpdate(_) => "GUILD_INTEGRATIONS_UPDATE",
            Self::GuildMemberAdd(_) => "GUILD_MEMBER_ADD",
            Self::GuildMemberRemove(_) => "GUILD_MEMBER_REMOVE",
            Self::GuildMemberUpdate(_) => "GUILD_MEMBER_UPDATE",
            Self::GuildMembersChunk(_) => "GUILD_MEMBERS_CHUNK",
            Self::GuildRoleCreate(_) => "GUILD_ROLE_CREATE",
            Self::GuildRoleUpdate(_) => "GUILD_ROLE_UPDATE",
            Self::GuildRoleDelete(_) => "GUILD_ROLE_DELETE",
            Self::ChannelCreate(_) => "CHANNEL_CREATE",
            Self::ChannelUpdate(_) => "CHANNEL_UPDATE",
            Self::ChannelDelete(_) => "CHANNEL_DELETE",
            Self::ChannelPinsUpdate(_) => "CHANNEL_PINS_UPDATE",
            Self::ThreadCreate(_) => "THREAD_CREATE",
            Self::ThreadUpdate(_) => "THREAD_UPDATE",
            Self::ThreadDelete(_) => "THREAD_DELETE",
            Self::ThreadListSync(_) => "THREAD_LIST_SYNC",
            Self::ThreadMemberUpdate(_) => "THREAD_MEMBER_UPDATE",
            Self::ThreadMembersUpdate(_) => "THREAD_MEMBERS_UPDATE",
            Self::MessageCreate(_) => "MESSAGE_CREATE",
            Self::MessageUpdate(_) => "MESSAGE_UPDATE",
            Self::MessageDelete(_) => "MESSAGE_DELETE",
            Self::MessageDeleteBulk(_) => "MESSAGE_DELETE_BULK",
            Self::MessageReactionAdd(_) => "MESSAGE_REACTION_ADD",
            Self::MessageReactionRemove(_) => "MESSAGE_REACTION_REMOVE",
            Self::MessageReactionRemoveAll(_) => "MESSAGE_REACTION_REMOVE_ALL",
            Self::MessageReactionRemoveEmoji(_) => "MESSAGE_REACTION_REMOVE_EMOJI",
            Self::InteractionCreate(_) => "INTERACTION_CREATE",
            Self::ApplicationCommandPermissionsUpdate(_) => {
                "APPLICATION_COMMAND_PERMISSIONS_UPDATE"
            }
            Self::VoiceStateUpdate(_) => "VOICE_STATE_UPDATE",
            Self::VoiceServerUpdate(_) => "VOICE_SERVER_UPDATE",
            Self::EntitlementCreate(_) => "ENTITLEMENT_CREATE",
            Self::EntitlementUpdate(_) => "ENTITLEMENT_UPDATE",
            Self::EntitlementDelete(_) => "ENTITLEMENT_DELETE",
            Self::UserUpdate(_) => "USER_UPDATE",
            Self::WebhooksUpdate(_) => "WEBHOOKS_UPDATE",
            Self::AutoModerationRuleCreate(_) => "AUTO_MODERATION_RULE_CREATE",
            Self::AutoModerationRuleUpdate(_) => "AUTO_MODERATION_RULE_UPDATE",
            Self::AutoModerationRuleDelete(_) => "AUTO_MODERATION_RULE_DELETE",
            Self::AutoModerationActionExecution(_) => "AUTO_MODERATION_ACTION_EXECUTION",
            Self::GuildScheduledEventCreate(_) => "GUILD_SCHEDULED_EVENT_CREATE",
            Self::GuildScheduledEventUpdate(_) => "GUILD_SCHEDULED_EVENT_UPDATE",
            Self::GuildScheduledEventDelete(_) => "GUILD_SCHEDULED_EVENT_DELETE",
            Self::GuildScheduledEventUserAdd(_) => "GUILD_SCHEDULED_EVENT_USER_ADD",
            Self::GuildScheduledEventUserRemove(_) => "GUILD_SCHEDULED_EVENT_USER_REMOVE",
            Self::InviteCreate(_) => "INVITE_CREATE",
            Self::InviteDelete(_) => "INVITE_DELETE",
            Self::StageInstanceCreate(_) => "STAGE_INSTANCE_CREATE",
            Self::StageInstanceUpdate(_) => "STAGE_INSTANCE_UPDATE",
            Self::StageInstanceDelete(_) => "STAGE_INSTANCE_DELETE",
            Self::GuildSoundboardSoundCreate(_) => "GUILD_SOUNDBOARD_SOUND_CREATE",
            Self::GuildSoundboardSoundUpdate(_) => "GUILD_SOUNDBOARD_SOUND_UPDATE",
            Self::GuildSoundboardSoundDelete(_) => "GUILD_SOUNDBOARD_SOUND_DELETE",
            Self::GuildSoundboardSoundsUpdate(_) => "GUILD_SOUNDBOARD_SOUNDS_UPDATE",
            Self::SoundboardSounds(_) => "SOUNDBOARD_SOUNDS",
            Self::PresenceUpdate(_) => "PRESENCE_UPDATE",
            Self::TypingStart(_) => "TYPING_START",
            Self::GuildAuditLogEntryCreate(_) => "GUILD_AUDIT_LOG_ENTRY_CREATE",
            Self::MessagePollVoteAdd(_) => "MESSAGE_POLL_VOTE_ADD",
            Self::MessagePollVoteRemove(_) => "MESSAGE_POLL_VOTE_REMOVE",
            Self::VoiceChannelEffectSend(_) => "VOICE_CHANNEL_EFFECT_SEND",
            Self::VoiceChannelStatusUpdate(_) => "VOICE_CHANNEL_STATUS_UPDATE",
            Self::VoiceChannelStartTimeUpdate(_) => "VOICE_CHANNEL_START_TIME_UPDATE",
            Self::ChannelInfo(_) => "CHANNEL_INFO",
            Self::IntegrationCreate(_) => "INTEGRATION_CREATE",
            Self::IntegrationUpdate(_) => "INTEGRATION_UPDATE",
            Self::IntegrationDelete(_) => "INTEGRATION_DELETE",
            Self::SubscriptionCreate(_) => "SUBSCRIPTION_CREATE",
            Self::SubscriptionUpdate(_) => "SUBSCRIPTION_UPDATE",
            Self::SubscriptionDelete(_) => "SUBSCRIPTION_DELETE",
            Self::RateLimited(_) => "RATE_LIMITED",
            Self::Unknown(event) => &event.name,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gateway::payload::Envelope;

    #[test]
    fn ready_with_all_fields() {
        let json = r#"{"op":0,"s":1,"t":"READY","d":{
            "v": 10,
            "user": {"id":"80351110224678912","username":"zeny","discriminator":"0",
                     "global_name":null,"avatar":null,"bot":true},
            "guilds": [{"id":"41771983423143937","unavailable":true}],
            "session_id": "abc123",
            "resume_gateway_url": "wss://gateway-us-east1-b.discord.gg",
            "shard": [0, 2],
            "application": {"id":"80351110224678912","flags":8192,"flags_new":"8192"},
            "_trace": ["future field"]
        }}"#;
        let env = Envelope::parse(json).unwrap();
        let Event::Ready(ready) = Event::from_dispatch(env.t.as_deref().unwrap(), env.d).unwrap()
        else {
            panic!("expected READY");
        };
        assert_eq!(ready.v, 10);
        assert!(ready.user.bot);
        assert!(ready.guilds[0].unavailable);
        assert_eq!(ready.shard, Some([0, 2]));
        assert_eq!(
            &*ready.resume_gateway_url,
            "wss://gateway-us-east1-b.discord.gg"
        );
        assert!(
            ready
                .application
                .all_flags()
                .contains(crate::application::ApplicationFlags::GATEWAY_PRESENCE_LIMITED)
        );
    }

    #[test]
    fn guild_delete_removed_vs_unavailable() {
        let removed = Event::from_dispatch(
            "GUILD_DELETE",
            Some(&RawValue::from_string(r#"{"id":"1"}"#.into()).unwrap()),
        )
        .unwrap();
        assert!(matches!(removed, Event::GuildDelete(g) if !g.unavailable));
    }

    #[test]
    fn known_event_with_bad_data_is_an_error() {
        let bad = RawValue::from_string(r#"{"id":"not a snowflake","type":0}"#.into()).unwrap();
        assert!(Event::from_dispatch("CHANNEL_CREATE", Some(&bad)).is_err());
    }

    #[test]
    fn message_create_in_guild() {
        let json = r#"{"op":0,"s":5,"t":"MESSAGE_CREATE","d":{
            "id":"334385199974967042","channel_id":"290926798999357250","guild_id":"41771983423143937",
            "author":{"id":"53908099506183680","username":"Mason","discriminator":"0","avatar":null},
            "member":{"roles":[],"joined_at":"2015-04-26T06:26:56.936000+00:00","deaf":false,"mute":false,"flags":0},
            "content":"hi <@1>","timestamp":"2017-07-11T17:27:07.299000+00:00","edited_timestamp":null,
            "tts":false,"mention_everyone":false,
            "mentions":[{"id":"1","username":"x","discriminator":"0","avatar":null,
                         "member":{"roles":[],"joined_at":null,"deaf":false,"mute":false}}],
            "mention_roles":[],"attachments":[],"embeds":[],"pinned":false,"type":0,
            "flags":32768,"components":[{"type":10,"content":"v2"}]
        }}"#;
        let env = Envelope::parse(json).unwrap();
        let Event::MessageCreate(msg) = Event::from_dispatch("MESSAGE_CREATE", env.d).unwrap()
        else {
            panic!("expected MESSAGE_CREATE");
        };
        assert!(msg.is_components_v2());
        assert!(msg.member.is_some());
        assert!(msg.mentions[0].member.is_some());
        assert_eq!(msg.mentions[0].user.id.get(), 1);
    }

    #[test]
    fn invite_delete_and_scheduled_event_user() {
        let deleted = Event::from_dispatch(
            "INVITE_DELETE",
            Some(&RawValue::from_string(r#"{"channel_id":"1","code":"abc"}"#.into()).unwrap()),
        )
        .unwrap();
        assert!(matches!(deleted, Event::InviteDelete(inv) if &*inv.code == "abc"));

        let add = Event::from_dispatch(
            "GUILD_SCHEDULED_EVENT_USER_ADD",
            Some(
                &RawValue::from_string(
                    r#"{"guild_scheduled_event_id":"1","user_id":"2","guild_id":"3"}"#.into(),
                )
                .unwrap(),
            ),
        )
        .unwrap();
        assert!(matches!(add, Event::GuildScheduledEventUserAdd(_)));
    }

    #[test]
    fn poll_vote_and_typing() {
        let vote = Event::from_dispatch(
            "MESSAGE_POLL_VOTE_ADD",
            Some(
                &RawValue::from_string(
                    r#"{"user_id":"1","channel_id":"2","message_id":"3","answer_id":1}"#.into(),
                )
                .unwrap(),
            ),
        )
        .unwrap();
        assert!(matches!(vote, Event::MessagePollVoteAdd(v) if v.answer_id == 1));

        let typing = Event::from_dispatch(
            "TYPING_START",
            Some(
                &RawValue::from_string(
                    r#"{"channel_id":"1","user_id":"2","timestamp":1578056622}"#.into(),
                )
                .unwrap(),
            ),
        )
        .unwrap();
        assert!(matches!(typing, Event::TypingStart(_)));
    }

    #[test]
    fn event_enum_stays_small() {
        assert!(size_of::<Event>() <= 48, "{}", size_of::<Event>());

        let cases = [
            (
                "VOICE_CHANNEL_STATUS_UPDATE",
                r#"{"id":"1","guild_id":"2","status":"live"}"#,
            ),
            (
                "VOICE_CHANNEL_START_TIME_UPDATE",
                r#"{"id":"1","guild_id":"2","voice_start_time":1700000000}"#,
            ),
            (
                "CHANNEL_INFO",
                r#"{"guild_id":"2","channels":[{"id":"1","status":null}]}"#,
            ),
            (
                "INTEGRATION_CREATE",
                r#"{"guild_id":"2","id":"3","name":"x","type":"discord","enabled":true,"account":{"id":"4","name":"a"}}"#,
            ),
            (
                "INTEGRATION_DELETE",
                r#"{"id":"3","guild_id":"2","application_id":"5"}"#,
            ),
            (
                "SUBSCRIPTION_CREATE",
                r#"{"id":"6","user_id":"7","sku_ids":[],"entitlement_ids":[],"current_period_start":"2026-11-01T00:00:00+00:00","current_period_end":"2026-12-01T00:00:00+00:00","status":0,"canceled_at":null}"#,
            ),
            (
                "RATE_LIMITED",
                r#"{"opcode":8,"retry_after":12.5,"meta":{"guild_id":"2","nonce":"n"}}"#,
            ),
        ];
        for (name, json) in cases {
            let raw = serde_json::value::RawValue::from_string(json.to_owned()).unwrap();
            let event = Event::from_dispatch(name, Some(&raw)).unwrap();
            assert_eq!(event.name(), name);
            assert!(!matches!(event, Event::Unknown(_)), "{name}");
        }
    }

    #[test]
    fn guild_create_unavailable_stub() {
        let event = Event::from_dispatch(
            "GUILD_CREATE",
            Some(&RawValue::from_string(r#"{"id":"1","unavailable":true}"#.into()).unwrap()),
        )
        .unwrap();
        assert!(matches!(
            event,
            Event::GuildCreate(GuildCreate::Unavailable(g)) if g.unavailable
        ));
    }

    #[test]
    fn interaction_create_slash() {
        let json = r#"{
            "type": 2,
            "token": "A_UNIQUE_TOKEN",
            "application_id": "1",
            "id": "786008729715212338",
            "guild_id": "290926798626357999",
            "app_permissions": "442368",
            "data": {
                "options": [{"type": 3, "name": "cardname", "value": "The Gitrog Monster"}],
                "type": 1,
                "name": "cardsearch",
                "id": "771825006014889984"
            },
            "channel_id": "645027906669510667",
            "member": {
                "user": {
                    "id": "53908232506183680",
                    "username": "Mason",
                    "avatar": "a_d5efa99b3eeaa7dd43acca82f5692432",
                    "discriminator": "1337",
                    "public_flags": 131141
                },
                "roles": ["539082325061836999"],
                "permissions": "2147483647",
                "pending": false,
                "nick": null,
                "mute": false,
                "joined_at": "2017-03-13T19:19:14.040000+00:00",
                "deaf": false
            }
        }"#;
        let event = Event::from_dispatch(
            "INTERACTION_CREATE",
            Some(&RawValue::from_string(json.into()).unwrap()),
        )
        .unwrap();
        let Event::InteractionCreate(interaction) = event else {
            panic!("expected INTERACTION_CREATE");
        };
        assert_eq!(
            interaction.kind,
            crate::interaction::InteractionType::ApplicationCommand
        );
        assert_eq!(interaction.author().unwrap().username.as_ref(), "Mason");
        assert!(!format!("{interaction:?}").contains("A_UNIQUE_TOKEN"));
    }

    #[test]
    fn voice_server_update_redacts_token() {
        let event = Event::from_dispatch(
            "VOICE_SERVER_UPDATE",
            Some(&RawValue::from_string(
                r#"{"token":"my_token","guild_id":"1","endpoint":"sweetwater-12345.discord.media:2048"}"#
                    .into(),
            )
            .unwrap()),
        )
        .unwrap();
        assert!(!format!("{event:?}").contains("my_token"));
        assert_eq!(event.name(), "VOICE_SERVER_UPDATE");
    }

    #[test]
    fn resumed_and_unknown() {
        let env = Envelope::parse(r#"{"op":0,"s":2,"t":"RESUMED","d":{}}"#).unwrap();
        let event = Event::from_dispatch(env.t.as_deref().unwrap(), env.d).unwrap();
        assert!(matches!(event, Event::Resumed));

        let env = Envelope::parse(r#"{"op":0,"s":3,"t":"BRAND_NEW_EVENT","d":{"x":[1]}}"#).unwrap();
        let event = Event::from_dispatch(env.t.as_deref().unwrap(), env.d).unwrap();
        assert_eq!(event.name(), "BRAND_NEW_EVENT");
        match event {
            Event::Unknown(raw) => assert_eq!(raw.data.get(), r#"{"x":[1]}"#),
            other => panic!("expected unknown, got {}", other.name()),
        }
    }

    #[test]
    fn missing_data_becomes_null() {
        let event = Event::from_dispatch("SOMETHING", None).unwrap();
        match event {
            Event::Unknown(raw) => assert_eq!(raw.data.get(), "null"),
            other => panic!("expected unknown, got {}", other.name()),
        }
    }
}
