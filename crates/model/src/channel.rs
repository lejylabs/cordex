//! Channels and threads (resources/channel.mdx, "Channel Object").

use serde::{Deserialize, Serialize};

use crate::guild::Member;
use crate::id::Id;
use crate::id::marker::{
    ApplicationMarker, ChannelMarker, EmojiMarker, GenericMarker, GuildMarker, MessageMarker,
    TagMarker, UserMarker,
};
use crate::image_hash::ImageHash;
use crate::permissions::Permissions;
use crate::serde_util::{bitflags_serde_int, int_enum};
use crate::timestamp::Timestamp;
use crate::user::User;

/// A guild channel, DM, group DM or thread. Discord uses one object for all
/// kinds; fields that do not apply to a kind are `None`/default.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Channel {
    /// Channel ID.
    pub id: Id<ChannelMarker>,
    /// Channel type.
    #[serde(rename = "type")]
    pub kind: ChannelType,
    /// Guild (missing in some gateway payloads).
    pub guild_id: Option<Id<GuildMarker>>,
    /// Sorting position.
    pub position: Option<i32>,
    /// Permission overwrites.
    #[serde(default)]
    pub permission_overwrites: Vec<PermissionOverwrite>,
    /// Name (1–100 characters).
    pub name: Option<Box<str>>,
    /// Topic.
    pub topic: Option<Box<str>>,
    /// Whether the channel is age restricted.
    #[serde(default)]
    pub nsfw: bool,
    /// Last message (or thread for forums); may not point to an existing message.
    pub last_message_id: Option<Id<MessageMarker>>,
    /// Voice bitrate in bits per second.
    pub bitrate: Option<u32>,
    /// Voice user limit (0 = unlimited).
    pub user_limit: Option<u32>,
    /// Slowmode in seconds (0–21600).
    pub rate_limit_per_user: Option<u32>,
    /// DM recipients.
    #[serde(default)]
    pub recipients: Vec<User>,
    /// Group DM icon hash.
    pub icon: Option<ImageHash>,
    /// Creator of the group DM or thread.
    pub owner_id: Option<Id<UserMarker>>,
    /// Application of a bot-created group DM.
    pub application_id: Option<Id<ApplicationMarker>>,
    /// Whether a group DM is managed by an application.
    pub managed: Option<bool>,
    /// Parent category, or parent channel of a thread.
    pub parent_id: Option<Id<ChannelMarker>>,
    /// When the last message was pinned.
    pub last_pin_timestamp: Option<Timestamp>,
    /// Voice region ID; `None` = automatic.
    pub rtc_region: Option<Box<str>>,
    /// Camera video quality.
    pub video_quality_mode: Option<VideoQualityMode>,
    /// Messages in a thread (excluding the initial and deleted messages).
    pub message_count: Option<u32>,
    /// Approximate users in a thread (stops at 50).
    pub member_count: Option<u32>,
    /// Thread-specific fields.
    pub thread_metadata: Option<ThreadMetadata>,
    /// Current user's thread membership.
    pub member: Option<ThreadMember>,
    /// Default auto archive duration for new threads, in minutes.
    pub default_auto_archive_duration: Option<AutoArchiveDuration>,
    /// Invoking user's permissions (interaction payloads).
    pub permissions: Option<Permissions>,
    /// Channel flags.
    #[serde(default)]
    pub flags: ChannelFlags,
    /// Messages ever sent in a thread.
    pub total_message_sent: Option<u32>,
    /// Tags of a forum or media channel.
    #[serde(default)]
    pub available_tags: Vec<ForumTag>,
    /// Tags applied to a forum thread.
    #[serde(default)]
    pub applied_tags: Vec<Id<TagMarker>>,
    /// Default reaction emoji of a forum.
    pub default_reaction_emoji: Option<DefaultReaction>,
    /// Initial slowmode for new threads.
    pub default_thread_rate_limit_per_user: Option<u32>,
    /// Default forum sort order.
    pub default_sort_order: Option<SortOrderType>,
    /// Default forum layout.
    pub default_forum_layout: Option<ForumLayoutType>,
}

impl Channel {
    /// Mention string `<#id>`.
    pub fn mention(&self) -> String {
        format!("<#{}>", self.id)
    }

    /// Whether the channel is a thread.
    pub const fn is_thread(&self) -> bool {
        self.kind.is_thread()
    }
}

int_enum! {
    /// Channel type.
    pub enum ChannelType: u8 {
        /// Text channel in a guild.
        GuildText = 0,
        /// Direct message.
        Dm = 1,
        /// Voice channel in a guild.
        GuildVoice = 2,
        /// Group direct message.
        GroupDm = 3,
        /// Category.
        GuildCategory = 4,
        /// Announcement channel.
        GuildAnnouncement = 5,
        /// Thread in an announcement channel.
        AnnouncementThread = 10,
        /// Public thread.
        PublicThread = 11,
        /// Private thread.
        PrivateThread = 12,
        /// Stage channel.
        GuildStageVoice = 13,
        /// Channel in a hub directory.
        GuildDirectory = 14,
        /// Forum (threads only).
        GuildForum = 15,
        /// Media channel (threads only).
        GuildMedia = 16,
    }
}

impl ChannelType {
    /// Whether this is a thread type.
    pub const fn is_thread(self) -> bool {
        matches!(
            self,
            Self::AnnouncementThread | Self::PublicThread | Self::PrivateThread
        )
    }
}

int_enum! {
    /// Camera video quality.
    pub enum VideoQualityMode: u8 {
        /// Discord chooses.
        Auto = 1,
        /// 720p.
        Full = 2,
    }
}

int_enum! {
    /// Forum post sort order.
    pub enum SortOrderType: u8 {
        /// By activity.
        LatestActivity = 0,
        /// By creation time.
        CreationDate = 1,
    }
}

int_enum! {
    /// Forum layout.
    pub enum ForumLayoutType: u8 {
        /// Not set.
        NotSet = 0,
        /// List.
        ListView = 1,
        /// Gallery.
        GalleryView = 2,
    }
}

int_enum! {
    /// Thread auto archive duration in minutes.
    pub enum AutoArchiveDuration: u16 {
        /// One hour.
        Hour = 60,
        /// One day.
        Day = 1440,
        /// Three days.
        ThreeDays = 4320,
        /// One week.
        Week = 10080,
    }
}

bitflags::bitflags! {
    /// Channel flags.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
    pub struct ChannelFlags: u64 {
        /// Thread is pinned in its forum or media channel.
        const PINNED = 1 << 1;
        /// A tag is required when creating a forum thread.
        const REQUIRE_TAG = 1 << 4;
        /// Hides media download options (media channels).
        const HIDE_MEDIA_DOWNLOAD_OPTIONS = 1 << 15;
        /// Metadata obfuscated for the current user.
        const CHANNEL_OBFUSCATED = 1 << 17;
        /// Spoiler channel; users must opt in to view it.
        const IS_SPOILER_CHANNEL = 1 << 21;
    }
}

bitflags_serde_int!(ChannelFlags, u64);

/// Permission overwrite for a role or member.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct PermissionOverwrite {
    /// Role or user ID (see `kind`).
    pub id: Id<GenericMarker>,
    /// Whether `id` is a role or a member.
    #[serde(rename = "type")]
    pub kind: OverwriteType,
    /// Allowed permissions.
    pub allow: Permissions,
    /// Denied permissions.
    pub deny: Permissions,
}

int_enum! {
    /// Target of a permission overwrite.
    pub enum OverwriteType: u8 {
        /// Role.
        Role = 0,
        /// Member.
        Member = 1,
    }
}

/// Thread-specific fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ThreadMetadata {
    /// Whether the thread is archived.
    pub archived: bool,
    /// Auto archive duration.
    pub auto_archive_duration: AutoArchiveDuration,
    /// When the archive status last changed.
    pub archive_timestamp: Timestamp,
    /// Whether only moderators can unarchive the thread.
    pub locked: bool,
    /// Whether non-moderators can add non-moderators (private threads).
    pub invitable: Option<bool>,
    /// When the thread was created (threads created after 2022-01-09).
    pub create_timestamp: Option<Timestamp>,
}

/// A user's membership in a thread.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ThreadMember {
    /// Thread ID (omitted in some payloads).
    pub id: Option<Id<ChannelMarker>>,
    /// User ID (omitted in some payloads).
    pub user_id: Option<Id<UserMarker>>,
    /// When the user last joined.
    pub join_timestamp: Timestamp,
    /// User-thread settings (notifications).
    #[serde(default)]
    pub flags: u64,
    /// Guild member (`with_member=true`).
    pub member: Option<Box<Member>>,
}

/// Forum or media channel tag.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ForumTag {
    /// Tag ID.
    pub id: Id<TagMarker>,
    /// Name (0–20 characters).
    pub name: Box<str>,
    /// Whether only moderators can apply it.
    #[serde(default)]
    pub moderated: bool,
    /// Custom emoji ID.
    pub emoji_id: Option<Id<EmojiMarker>>,
    /// Unicode emoji.
    pub emoji_name: Option<Box<str>>,
}

/// Default reaction emoji of a forum.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct DefaultReaction {
    /// Custom emoji ID.
    pub emoji_id: Option<Id<EmojiMarker>>,
    /// Unicode emoji.
    pub emoji_name: Option<Box<str>>,
}
