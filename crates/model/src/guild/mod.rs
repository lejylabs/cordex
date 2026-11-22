//! Guilds (resources/guild.mdx, "Guild Object").

pub mod integration;
pub mod member;
pub mod role;

use serde::{Deserialize, Serialize};

pub use integration::{
    Integration, IntegrationAccount, IntegrationExpireBehavior, IntegrationType,
};
pub use member::{Ban, Member, MemberFlags};
pub use role::{Role, RoleColors, RoleFlags, RoleTags};

use crate::emoji::Emoji;
use crate::id::Id;
use crate::id::marker::{ApplicationMarker, ChannelMarker, EmojiMarker, GuildMarker, UserMarker};
use crate::image_hash::ImageHash;
use crate::permissions::Permissions;
use crate::serde_util::{bitflags_serde_int, int_enum, str_enum};
use crate::sticker::Sticker;
use crate::timestamp::Timestamp;

/// A guild ("server").
///
/// Fields the docs mark as always present but that are missing in some
/// payloads (e.g. the docs' own example) default instead of failing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Guild {
    /// Guild ID.
    pub id: Id<GuildMarker>,
    /// Name (2–100 characters).
    pub name: Box<str>,
    /// Icon hash.
    pub icon: Option<ImageHash>,
    /// Icon hash in template objects.
    pub icon_hash: Option<ImageHash>,
    /// Splash hash.
    pub splash: Option<ImageHash>,
    /// Discovery splash hash (discoverable guilds).
    pub discovery_splash: Option<ImageHash>,
    /// Whether the current user owns the guild (`GET /users/@me/guilds`).
    pub owner: Option<bool>,
    /// Owner ID; missing in partial guilds (`GET /users/@me/guilds`,
    /// application and invite objects).
    pub owner_id: Option<Id<UserMarker>>,
    /// Current user's permissions (`GET /users/@me/guilds`).
    pub permissions: Option<Permissions>,
    /// Deprecated voice region.
    pub region: Option<Box<str>>,
    /// AFK channel.
    pub afk_channel_id: Option<Id<ChannelMarker>>,
    /// AFK timeout in seconds.
    #[serde(default)]
    pub afk_timeout: u32,
    /// Whether the widget is enabled.
    pub widget_enabled: Option<bool>,
    /// Widget channel.
    pub widget_channel_id: Option<Id<ChannelMarker>>,
    /// Verification level.
    #[serde(default = "VerificationLevel::default_value")]
    pub verification_level: VerificationLevel,
    /// Default notification level.
    #[serde(default = "DefaultMessageNotificationLevel::default_value")]
    pub default_message_notifications: DefaultMessageNotificationLevel,
    /// Explicit content filter.
    #[serde(default = "ExplicitContentFilterLevel::default_value")]
    pub explicit_content_filter: ExplicitContentFilterLevel,
    /// Roles.
    #[serde(default)]
    pub roles: Vec<Role>,
    /// Custom emojis.
    #[serde(default)]
    pub emojis: Vec<Emoji>,
    /// Enabled features.
    #[serde(default)]
    pub features: Vec<GuildFeature>,
    /// MFA requirement for moderation.
    #[serde(default = "MfaLevel::default_value")]
    pub mfa_level: MfaLevel,
    /// Application that created the guild (bot-created guilds).
    pub application_id: Option<Id<ApplicationMarker>>,
    /// Channel for system messages (joins, boosts).
    pub system_channel_id: Option<Id<ChannelMarker>>,
    /// System channel flags.
    #[serde(default)]
    pub system_channel_flags: SystemChannelFlags,
    /// Rules channel (community guilds).
    pub rules_channel_id: Option<Id<ChannelMarker>>,
    /// Maximum presences (`None` except for the largest guilds).
    pub max_presences: Option<u32>,
    /// Maximum members.
    pub max_members: Option<u32>,
    /// Vanity invite code.
    pub vanity_url_code: Option<Box<str>>,
    /// Description (community guilds).
    pub description: Option<Box<str>>,
    /// Banner hash.
    pub banner: Option<ImageHash>,
    /// Boost tier.
    #[serde(default = "PremiumTier::default_value")]
    pub premium_tier: PremiumTier,
    /// Number of boosts.
    pub premium_subscription_count: Option<u32>,
    /// Preferred locale of a community guild (default `en-US`).
    #[serde(default)]
    pub preferred_locale: Box<str>,
    /// Channel for Discord's community updates.
    pub public_updates_channel_id: Option<Id<ChannelMarker>>,
    /// Maximum users in a video channel.
    pub max_video_channel_users: Option<u32>,
    /// Maximum users in a stage video channel.
    pub max_stage_video_channel_users: Option<u32>,
    /// Approximate members (`with_counts=true`).
    pub approximate_member_count: Option<u32>,
    /// Approximate online members (`with_counts=true`).
    pub approximate_presence_count: Option<u32>,
    /// Welcome screen (invites).
    pub welcome_screen: Option<WelcomeScreen>,
    /// Age restriction level.
    #[serde(default = "NsfwLevel::default_value")]
    pub nsfw_level: NsfwLevel,
    /// Custom stickers.
    #[serde(default)]
    pub stickers: Vec<Sticker>,
    /// Whether the boost progress bar is shown.
    #[serde(default)]
    pub premium_progress_bar_enabled: bool,
    /// Channel for safety alerts (community guilds).
    pub safety_alerts_channel_id: Option<Id<ChannelMarker>>,
    /// Incident actions (raids, DM spam).
    pub incidents_data: Option<IncidentsData>,
}

impl Guild {
    /// Whether the guild has a feature.
    pub fn has_feature(&self, feature: &GuildFeature) -> bool {
        self.features.contains(feature)
    }
}

/// A guild that is offline or not sent yet (Ready, Guild Delete).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct UnavailableGuild {
    /// Guild ID.
    pub id: Id<GuildMarker>,
    /// `true` while the guild is unavailable (outage, not yet loaded);
    /// missing in Guild Delete when the bot was removed.
    #[serde(default)]
    pub unavailable: bool,
}

/// Default value for documented-but-sometimes-missing enum fields.
macro_rules! default_value {
    ($($ty:ident => $value:ident),* $(,)?) => {
        $(
            impl $ty {
                const fn default_value() -> Self {
                    Self::$value
                }
            }
        )*
    };
}

default_value! {
    VerificationLevel => None,
    DefaultMessageNotificationLevel => AllMessages,
    ExplicitContentFilterLevel => Disabled,
    MfaLevel => None,
    PremiumTier => None,
    NsfwLevel => Default,
}

int_enum! {
    /// Default notification level.
    pub enum DefaultMessageNotificationLevel: u8 {
        /// Notify for all messages.
        AllMessages = 0,
        /// Notify only for mentions.
        OnlyMentions = 1,
    }
}

int_enum! {
    /// Explicit content filter level.
    pub enum ExplicitContentFilterLevel: u8 {
        /// Media is not scanned.
        Disabled = 0,
        /// Media of members without roles is scanned.
        MembersWithoutRoles = 1,
        /// Media of all members is scanned.
        AllMembers = 2,
    }
}

int_enum! {
    /// MFA requirement for moderation actions.
    pub enum MfaLevel: u8 {
        /// No requirement.
        None = 0,
        /// 2FA required.
        Elevated = 1,
    }
}

int_enum! {
    /// Verification level.
    pub enum VerificationLevel: u8 {
        /// Unrestricted.
        None = 0,
        /// Verified email.
        Low = 1,
        /// Registered for longer than 5 minutes.
        Medium = 2,
        /// Member for longer than 10 minutes.
        High = 3,
        /// Verified phone number.
        VeryHigh = 4,
    }
}

int_enum! {
    /// Age restriction level.
    pub enum NsfwLevel: u8 {
        /// Default.
        Default = 0,
        /// Explicit.
        Explicit = 1,
        /// Safe.
        Safe = 2,
        /// Age restricted.
        AgeRestricted = 3,
    }
}

int_enum! {
    /// Server boost tier.
    pub enum PremiumTier: u8 {
        /// No perks.
        None = 0,
        /// Level 1.
        Tier1 = 1,
        /// Level 2.
        Tier2 = 2,
        /// Level 3.
        Tier3 = 3,
    }
}

bitflags::bitflags! {
    /// System channel flags.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
    pub struct SystemChannelFlags: u64 {
        /// Suppress join notifications.
        const SUPPRESS_JOIN_NOTIFICATIONS = 1 << 0;
        /// Suppress boost notifications.
        const SUPPRESS_PREMIUM_SUBSCRIPTIONS = 1 << 1;
        /// Suppress server setup tips.
        const SUPPRESS_GUILD_REMINDER_NOTIFICATIONS = 1 << 2;
        /// Hide join sticker reply buttons.
        const SUPPRESS_JOIN_NOTIFICATION_REPLIES = 1 << 3;
        /// Suppress role subscription purchase notifications.
        const SUPPRESS_ROLE_SUBSCRIPTION_PURCHASE_NOTIFICATIONS = 1 << 4;
        /// Hide role subscription sticker reply buttons.
        const SUPPRESS_ROLE_SUBSCRIPTION_PURCHASE_NOTIFICATION_REPLIES = 1 << 5;
    }
}

bitflags_serde_int!(SystemChannelFlags, u64);

str_enum! {
    /// Guild feature.
    pub enum GuildFeature {
        /// Animated banner.
        AnimatedBanner = "ANIMATED_BANNER",
        /// Animated icon.
        AnimatedIcon = "ANIMATED_ICON",
        /// New command permission behavior.
        ApplicationCommandPermissionsV2 = "APPLICATION_COMMAND_PERMISSIONS_V2",
        /// Auto moderation.
        AutoModeration = "AUTO_MODERATION",
        /// Banner.
        Banner = "BANNER",
        /// Community guild.
        Community = "COMMUNITY",
        /// Monetization enabled provisionally.
        CreatorMonetizableProvisional = "CREATOR_MONETIZABLE_PROVISIONAL",
        /// Role subscription promo page.
        CreatorStorePage = "CREATOR_STORE_PAGE",
        /// Developer support server.
        DeveloperSupportServer = "DEVELOPER_SUPPORT_SERVER",
        /// Discoverable.
        Discoverable = "DISCOVERABLE",
        /// Enhanced role colors (gradients).
        EnhancedRoleColors = "ENHANCED_ROLE_COLORS",
        /// Can be featured in the directory.
        Featurable = "FEATURABLE",
        /// Server tags.
        GuildTags = "GUILD_TAGS",
        /// Guest invites.
        GuestsEnabled = "GUESTS_ENABLED",
        /// Invites paused.
        InvitesDisabled = "INVITES_DISABLED",
        /// Invite splash.
        InviteSplash = "INVITE_SPLASH",
        /// Membership screening.
        MemberVerificationGateEnabled = "MEMBER_VERIFICATION_GATE_ENABLED",
        /// More soundboard slots.
        MoreSoundboard = "MORE_SOUNDBOARD",
        /// More sticker slots.
        MoreStickers = "MORE_STICKERS",
        /// Announcement channels.
        News = "NEWS",
        /// Partnered.
        Partnered = "PARTNERED",
        /// Preview enabled.
        PreviewEnabled = "PREVIEW_ENABLED",
        /// Pruning requires administrator.
        PruneRequiresAdmin = "PRUNE_REQUIRES_ADMIN",
        /// Raid alerts disabled.
        RaidAlertsDisabled = "RAID_ALERTS_DISABLED",
        /// Role icons.
        RoleIcons = "ROLE_ICONS",
        /// Role subscriptions purchasable.
        RoleSubscriptionsAvailableForPurchase = "ROLE_SUBSCRIPTIONS_AVAILABLE_FOR_PURCHASE",
        /// Role subscriptions enabled.
        RoleSubscriptionsEnabled = "ROLE_SUBSCRIPTIONS_ENABLED",
        /// Soundboard.
        Soundboard = "SOUNDBOARD",
        /// Ticketed events.
        TicketedEventsEnabled = "TICKETED_EVENTS_ENABLED",
        /// Vanity URL.
        VanityUrl = "VANITY_URL",
        /// Verified.
        Verified = "VERIFIED",
        /// 384 kbps voice.
        VipRegions = "VIP_REGIONS",
        /// Welcome screen.
        WelcomeScreenEnabled = "WELCOME_SCREEN_ENABLED",
    }
}

/// Welcome screen of a community guild.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct WelcomeScreen {
    /// Description.
    pub description: Option<Box<str>>,
    /// Up to 5 channels.
    #[serde(default)]
    pub welcome_channels: Vec<WelcomeScreenChannel>,
}

/// Channel shown in the welcome screen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct WelcomeScreenChannel {
    /// Channel ID.
    pub channel_id: Id<ChannelMarker>,
    /// Description.
    pub description: Box<str>,
    /// Custom emoji ID.
    pub emoji_id: Option<Id<EmojiMarker>>,
    /// Emoji name or unicode character.
    pub emoji_name: Option<Box<str>>,
}

/// Incident actions of a guild.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct IncidentsData {
    /// When invites are enabled again.
    pub invites_disabled_until: Option<Timestamp>,
    /// When DMs are enabled again.
    pub dms_disabled_until: Option<Timestamp>,
    /// When DM spam was detected.
    pub dm_spam_detected_at: Option<Timestamp>,
    /// When a raid was detected.
    pub raid_detected_at: Option<Timestamp>,
}
