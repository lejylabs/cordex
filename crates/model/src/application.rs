//! Applications and teams (resources/application.mdx, topics/teams.mdx).

use serde::{Deserialize, Serialize};

use crate::guild::Guild;
use crate::id::Id;
use crate::id::marker::{ApplicationMarker, GuildMarker, SkuMarker, TeamMarker, UserMarker};
use crate::image_hash::ImageHash;
use crate::permissions::Permissions;
use crate::serde_util::{bitflags_serde_int, bitflags_serde_string, int_enum, str_enum};
use crate::user::User;

/// An application.
///
/// Discord often sends partial applications (Ready: `id` + flags; messages:
/// Rich Presence fields), so everything except `id` is optional/defaulted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Application {
    /// Application ID.
    pub id: Id<ApplicationMarker>,
    /// Name.
    #[serde(default)]
    pub name: Box<str>,
    /// Icon hash.
    pub icon: Option<ImageHash>,
    /// Description.
    #[serde(default)]
    pub description: Box<str>,
    /// RPC origin URLs.
    #[serde(default)]
    pub rpc_origins: Vec<Box<str>>,
    /// When `false`, only the owner can add the bot.
    #[serde(default)]
    pub bot_public: bool,
    /// When `true`, the bot joins only after the full `OAuth2` code grant.
    #[serde(default)]
    pub bot_require_code_grant: bool,
    /// Bot user (partial).
    pub bot: Option<User>,
    /// Terms of service URL.
    pub terms_of_service_url: Option<Box<str>>,
    /// Privacy policy URL.
    pub privacy_policy_url: Option<Box<str>>,
    /// Owner (partial user).
    pub owner: Option<User>,
    /// Hex key for interaction signature verification.
    #[serde(default)]
    pub verify_key: Box<str>,
    /// Team owning the app.
    pub team: Option<Team>,
    /// Associated guild (e.g. developer support server).
    pub guild_id: Option<Id<GuildMarker>>,
    /// Associated guild (partial).
    pub guild: Option<Box<Guild>>,
    /// Game SKU.
    pub primary_sku_id: Option<Id<SkuMarker>>,
    /// Store page slug.
    pub slug: Option<Box<str>>,
    /// Rich presence cover image hash.
    pub cover_image: Option<ImageHash>,
    /// Legacy flags (number, at most 31 bits).
    #[serde(default)]
    pub flags: ApplicationFlags,
    /// Full flags (string; bits beyond 30 only appear here).
    pub flags_new: Option<ApplicationFlagsNew>,
    /// Approximate guild count.
    pub approximate_guild_count: Option<u32>,
    /// Approximate user install count.
    pub approximate_user_install_count: Option<u32>,
    /// Approximate user authorization count.
    pub approximate_user_authorization_count: Option<u32>,
    /// `OAuth2` redirect URIs.
    #[serde(default)]
    pub redirect_uris: Vec<Box<str>>,
    /// Interactions endpoint URL.
    pub interactions_endpoint_url: Option<Box<str>>,
    /// Role connection verification URL.
    pub role_connections_verification_url: Option<Box<str>>,
    /// Event webhooks URL.
    pub event_webhooks_url: Option<Box<str>>,
    /// Event webhooks status.
    pub event_webhooks_status: Option<EventWebhookStatus>,
    /// Subscribed webhook event types.
    #[serde(default)]
    pub event_webhooks_types: Vec<Box<str>>,
    /// Up to 5 tags.
    #[serde(default)]
    pub tags: Vec<Box<str>>,
    /// Default in-app authorization settings.
    pub install_params: Option<InstallParams>,
    /// Default install settings per installation context.
    pub integration_types_config: Option<IntegrationTypesConfig>,
    /// Custom authorization URL.
    pub custom_install_url: Option<Box<str>>,
}

impl Application {
    /// All flags: `flags_new` if present, else the legacy `flags`.
    pub fn all_flags(&self) -> ApplicationFlags {
        self.flags_new.map_or(self.flags, |f| f.0)
    }
}

bitflags::bitflags! {
    /// Public application flags.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
    pub struct ApplicationFlags: u64 {
        /// Uses the auto moderation API (badge).
        const APPLICATION_AUTO_MODERATION_RULE_CREATE_BADGE = 1 << 6;
        /// Presence intent approved (100+ guilds).
        const GATEWAY_PRESENCE = 1 << 12;
        /// Presence intent enabled (under 100 guilds).
        const GATEWAY_PRESENCE_LIMITED = 1 << 13;
        /// Guild members intent approved (100+ guilds).
        const GATEWAY_GUILD_MEMBERS = 1 << 14;
        /// Guild members intent enabled (under 100 guilds).
        const GATEWAY_GUILD_MEMBERS_LIMITED = 1 << 15;
        /// Verification pending, guild limit reached.
        const VERIFICATION_PENDING_GUILD_LIMIT = 1 << 16;
        /// Embedded application (activity).
        const EMBEDDED = 1 << 17;
        /// Message content intent approved (100+ guilds).
        const GATEWAY_MESSAGE_CONTENT = 1 << 18;
        /// Message content intent enabled (under 100 guilds).
        const GATEWAY_MESSAGE_CONTENT_LIMITED = 1 << 19;
        /// Has at least one global application command (badge).
        const APPLICATION_COMMAND_BADGE = 1 << 23;
    }
}

bitflags_serde_int!(ApplicationFlags, u64);

/// [`ApplicationFlags`] in the string form of `flags_new`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ApplicationFlagsNew(pub ApplicationFlags);

impl ApplicationFlagsNew {
    const fn bits(self) -> u64 {
        self.0.bits()
    }

    const fn from_bits_retain(bits: u64) -> Self {
        Self(ApplicationFlags::from_bits_retain(bits))
    }
}

bitflags_serde_string!(ApplicationFlagsNew);

int_enum! {
    /// Event webhook status.
    pub enum EventWebhookStatus: u8 {
        /// Disabled by the developer.
        Disabled = 1,
        /// Enabled by the developer.
        Enabled = 2,
        /// Disabled by Discord (usually inactivity).
        DisabledByDiscord = 3,
    }
}

int_enum! {
    /// Where an app can be installed.
    pub enum ApplicationIntegrationType: u8 {
        /// Installable to servers.
        GuildInstall = 0,
        /// Installable to users.
        UserInstall = 1,
    }
}

/// `OAuth2` install parameters.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct InstallParams {
    /// Scopes.
    pub scopes: Vec<Box<str>>,
    /// Permissions requested for the bot role.
    pub permissions: Permissions,
}

/// Install settings per installation context (`"0"` guild, `"1"` user).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[non_exhaustive]
pub struct IntegrationTypesConfig {
    /// Guild install.
    #[serde(rename = "0", default, skip_serializing_if = "Option::is_none")]
    pub guild_install: Option<IntegrationTypeConfig>,
    /// User install.
    #[serde(rename = "1", default, skip_serializing_if = "Option::is_none")]
    pub user_install: Option<IntegrationTypeConfig>,
}

/// Install settings of one installation context.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[non_exhaustive]
pub struct IntegrationTypeConfig {
    /// Install params.
    pub oauth2_install_params: Option<InstallParams>,
}

/// A developer team.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Team {
    /// Icon hash.
    pub icon: Option<ImageHash>,
    /// Team ID.
    pub id: Id<TeamMarker>,
    /// Members.
    #[serde(default)]
    pub members: Vec<TeamMember>,
    /// Name.
    pub name: Box<str>,
    /// Owner.
    pub owner_user_id: Id<UserMarker>,
}

/// Member of a team.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct TeamMember {
    /// Membership state.
    pub membership_state: MembershipState,
    /// Team ID.
    pub team_id: Id<TeamMarker>,
    /// User (partial).
    pub user: User,
    /// Role in the team; `None` for the owner (the owner role has no value).
    pub role: Option<TeamMemberRole>,
}

int_enum! {
    /// Team membership state.
    pub enum MembershipState: u8 {
        /// Invited.
        Invited = 1,
        /// Accepted.
        Accepted = 2,
    }
}

str_enum! {
    /// Role in a team (the owner has no role value).
    pub enum TeamMemberRole {
        /// Admin.
        Admin = "admin",
        /// Developer.
        Developer = "developer",
        /// Read only.
        ReadOnly = "read_only",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ready_partial_application() {
        let app: Application =
            serde_json::from_str(r#"{"id":"1","flags":8192,"flags_new":"8192"}"#).unwrap();
        assert!(
            app.all_flags()
                .contains(ApplicationFlags::GATEWAY_PRESENCE_LIMITED)
        );
        assert!(app.name.is_empty());
    }
}
