//! Guild members (resources/guild.mdx, "Guild Member Object").

use serde::{Deserialize, Serialize};

use crate::id::Id;
use crate::id::marker::{GuildMarker, RoleMarker};
use crate::image_hash::ImageHash;
use crate::permissions::Permissions;
use crate::serde_util::bitflags_serde_int;
use crate::timestamp::Timestamp;
use crate::user::{AvatarDecorationData, Collectibles, User};

/// A banned user (resources/guild.mdx, "Ban Object").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Ban {
    /// Ban reason, if Discord still has one.
    pub reason: Option<Box<str>>,
    /// Banned user.
    pub user: User,
}

/// A member of a guild.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Member {
    /// Guild ID (`GUILD_MEMBER_ADD` and similar gateway payloads).
    pub guild_id: Option<Id<GuildMarker>>,
    /// The user; missing in `MESSAGE_CREATE`/`MESSAGE_UPDATE` member objects.
    pub user: Option<User>,
    /// Guild nickname.
    pub nick: Option<Box<str>>,
    /// Guild avatar hash.
    pub avatar: Option<ImageHash>,
    /// Guild banner hash.
    pub banner: Option<ImageHash>,
    /// Role IDs.
    #[serde(default)]
    pub roles: Vec<Id<RoleMarker>>,
    /// When the user joined; `None` for guest members.
    pub joined_at: Option<Timestamp>,
    /// When the user started boosting.
    pub premium_since: Option<Timestamp>,
    /// Whether the user is deafened in voice.
    #[serde(default)]
    pub deaf: bool,
    /// Whether the user is muted in voice.
    #[serde(default)]
    pub mute: bool,
    /// Member flags.
    #[serde(default)]
    pub flags: MemberFlags,
    /// Whether the user has not yet passed membership screening.
    #[serde(default)]
    pub pending: bool,
    /// Total permissions in the channel (interaction payloads only).
    pub permissions: Option<Permissions>,
    /// Timeout end.
    pub communication_disabled_until: Option<Timestamp>,
    /// Guild avatar decoration.
    pub avatar_decoration_data: Option<AvatarDecorationData>,
    /// Guild collectibles.
    pub collectibles: Option<Collectibles>,
}

impl Member {
    /// Name to show: nickname, else display name, else username.
    pub fn display_name(&self) -> Option<&str> {
        self.nick
            .as_deref()
            .or_else(|| self.user.as_ref().map(User::display_name))
    }

    /// Whether the member is timed out at `now`.
    pub fn is_timed_out(&self, now: Timestamp) -> bool {
        self.communication_disabled_until
            .is_some_and(|until| until > now)
    }
}

bitflags::bitflags! {
    /// Guild member flags.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
    pub struct MemberFlags: u64 {
        /// Left and rejoined the guild.
        const DID_REJOIN = 1 << 0;
        /// Completed onboarding.
        const COMPLETED_ONBOARDING = 1 << 1;
        /// Exempt from verification requirements.
        const BYPASSES_VERIFICATION = 1 << 2;
        /// Started onboarding.
        const STARTED_ONBOARDING = 1 << 3;
        /// Guest, can only access the voice channel they were invited to.
        const IS_GUEST = 1 << 4;
        /// Started Server Guide new member actions.
        const STARTED_HOME_ACTIONS = 1 << 5;
        /// Completed Server Guide new member actions.
        const COMPLETED_HOME_ACTIONS = 1 << 6;
        /// Username, display name or nickname blocked by AutoMod.
        const AUTOMOD_QUARANTINED_USERNAME = 1 << 7;
        /// Dismissed the DM settings upsell.
        const DM_SETTINGS_UPSELL_ACKNOWLEDGED = 1 << 9;
        /// Guild tag blocked by AutoMod.
        const AUTOMOD_QUARANTINED_GUILD_TAG = 1 << 10;
    }
}

bitflags_serde_int!(MemberFlags, u64);
