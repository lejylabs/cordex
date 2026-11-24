//! Invites (resources/invite.mdx).

use serde::{Deserialize, Serialize};

use crate::application::Application;
use crate::channel::Channel;
use crate::error::ParseError;
use crate::guild::{Guild, Role};
use crate::id::Id;
use crate::id::marker::{ChannelMarker, GuildMarker, RoleMarker};
use crate::scheduled_event::ScheduledEvent;
use crate::serde_util::{bitflags_serde_int, int_enum};
use crate::timestamp::Timestamp;
use crate::user::User;

int_enum! {
    /// Kind of invite.
    pub enum InviteType: u8 {
        /// Guild channel.
        Guild = 0,
        /// Group DM.
        GroupDm = 1,
        /// Friend invite.
        Friend = 2,
    }
}

int_enum! {
    /// Target of a voice-channel invite.
    pub enum InviteTargetType: u8 {
        /// A user's stream.
        Stream = 1,
        /// An embedded application.
        EmbeddedApplication = 2,
    }
}

bitflags::bitflags! {
    /// Flags on a guild invite.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
    pub struct GuildInviteFlags: u64 {
        /// Guest invite for a voice channel.
        const IS_GUEST_INVITE = 1 << 0;
    }
}

bitflags_serde_int!(GuildInviteFlags, u64);

/// An invite to a guild, group DM or user.
///
/// Metadata fields (`uses`, `max_uses`, `max_age`, `temporary`, `created_at`)
/// are present on guild invite lists; they are `None` on `GET /invites/{code}`
/// unless Discord includes them.
#[allow(clippy::struct_excessive_bools)] // Discord sends these as separate fields
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Invite {
    /// Invite type. Omitted on some payloads; treated as guild.
    #[serde(rename = "type", default = "InviteType::default_guild")]
    pub kind: InviteType,
    /// Invite code.
    pub code: Box<str>,
    /// Partial guild (guild invites).
    pub guild: Option<Guild>,
    /// Partial channel; `None` for some friend invites.
    pub channel: Option<Channel>,
    /// Creator.
    pub inviter: Option<User>,
    /// Voice-channel target type.
    pub target_type: Option<InviteTargetType>,
    /// User whose stream is shown.
    pub target_user: Option<User>,
    /// Embedded application for an activity invite.
    pub target_application: Option<Application>,
    /// Approximate online members (`with_counts`).
    pub approximate_presence_count: Option<u32>,
    /// Approximate member count (`with_counts`).
    pub approximate_member_count: Option<u32>,
    /// Expiration; the docs table marks this required, the example omits it.
    #[serde(default)]
    pub expires_at: Option<Timestamp>,
    /// Scheduled event attached to the invite.
    pub guild_scheduled_event: Option<ScheduledEvent>,
    /// Guild invite flags.
    #[serde(default)]
    pub flags: GuildInviteFlags,
    /// Roles granted on accept (partial: id, name, position, colors, icon).
    #[serde(default)]
    pub roles: Vec<Role>,
    /// Times used (invite metadata).
    pub uses: Option<u32>,
    /// Max uses; `0` = unlimited (invite metadata).
    pub max_uses: Option<u32>,
    /// Lifetime in seconds; `0` = never (invite metadata).
    pub max_age: Option<u32>,
    /// Whether membership is temporary (invite metadata).
    pub temporary: Option<bool>,
    /// When the invite was created (invite metadata).
    pub created_at: Option<Timestamp>,
}

impl InviteType {
    const fn default_guild() -> Self {
        Self::Guild
    }
}

/// `INVITE_CREATE` payload (not the REST invite object).
#[allow(
    clippy::struct_excessive_bools,
    reason = "Discord sends these as separate fields"
)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct InviteCreate {
    /// Channel.
    pub channel_id: Id<ChannelMarker>,
    /// Invite code.
    pub code: Box<str>,
    /// When the invite was created.
    pub created_at: Timestamp,
    /// Guild.
    pub guild_id: Option<Id<GuildMarker>>,
    /// Creator.
    pub inviter: Option<User>,
    /// Lifetime in seconds (`0` = never).
    pub max_age: u32,
    /// Max uses (`0` = unlimited).
    pub max_uses: u32,
    /// Voice-channel target type.
    pub target_type: Option<InviteTargetType>,
    /// User whose stream is shown.
    pub target_user: Option<User>,
    /// Embedded application.
    pub target_application: Option<Application>,
    /// Temporary membership.
    #[serde(default)]
    pub temporary: bool,
    /// Use count (always 0 on create).
    #[serde(default)]
    pub uses: u32,
    /// Expiration.
    pub expires_at: Option<Timestamp>,
    /// Roles granted on accept.
    #[serde(default)]
    pub role_ids: Vec<Id<RoleMarker>>,
}

/// `INVITE_DELETE` payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct InviteDelete {
    /// Channel.
    pub channel_id: Id<ChannelMarker>,
    /// Guild.
    pub guild_id: Option<Id<GuildMarker>>,
    /// Invite code.
    pub code: Box<str>,
}

/// Parse a Discord invite URL into its invite code.
///
/// Accepts `discord.gg/{code}` and `discord.com/invite/{code}` (plus
/// `discordapp.com`, optional `www.`/`ptb.`/`canary.` subdomain, `http://`
/// or `https://`, and optional trailing slash, query string or fragment).
pub fn parse_invite_url(url: &str) -> Result<&str, ParseError> {
    let trimmed = url.trim();
    let rest = trimmed
        .strip_prefix("https://")
        .or_else(|| trimmed.strip_prefix("http://"))
        .ok_or(ParseError::InviteUrl)?;
    let rest = rest.split(['?', '#']).next().unwrap_or(rest);
    let (host, path) = rest.split_once('/').ok_or(ParseError::InviteUrl)?;
    let path = path.trim_end_matches('/');

    let code = if matches!(host, "discord.gg" | "www.discord.gg") {
        path
    } else if matches!(
        host,
        "discord.com"
            | "www.discord.com"
            | "ptb.discord.com"
            | "canary.discord.com"
            | "discordapp.com"
            | "www.discordapp.com"
            | "ptb.discordapp.com"
            | "canary.discordapp.com"
    ) {
        path.strip_prefix("invite/").ok_or(ParseError::InviteUrl)?
    } else {
        return Err(ParseError::InviteUrl);
    };

    if code.is_empty() || !code.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-') {
        return Err(ParseError::InviteUrl);
    }

    Ok(code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_round_trip() {
        let json = r#"{
            "uses":0,"max_uses":0,"max_age":0,"temporary":false,
            "created_at":"2016-03-31T19:15:39.954000+00:00",
            "code":"abc","channel":null
        }"#;
        let invite: Invite = serde_json::from_str(json).unwrap();
        assert_eq!(invite.kind, InviteType::Guild);
        assert_eq!(invite.uses, Some(0));
        assert!(!invite.temporary.unwrap());
    }

    #[test]
    fn guest_flag() {
        let json = r#"{"code":"x","channel":null,"flags":1}"#;
        let invite: Invite = serde_json::from_str(json).unwrap();
        assert!(invite.flags.contains(GuildInviteFlags::IS_GUEST_INVITE));
    }

    #[test]
    fn parses_invite_urls() {
        assert_eq!(
            parse_invite_url("https://discord.gg/abc123").unwrap(),
            "abc123"
        );
        assert_eq!(
            parse_invite_url("https://www.discord.gg/my-vanity/?event=42#top").unwrap(),
            "my-vanity"
        );
        assert_eq!(
            parse_invite_url("https://discord.com/invite/rust-lang").unwrap(),
            "rust-lang"
        );
        assert_eq!(
            parse_invite_url("https://ptb.discord.com/invite/xyz789/").unwrap(),
            "xyz789"
        );
        assert_eq!(
            parse_invite_url("http://canary.discordapp.com/invite/AbC-123?foo=bar").unwrap(),
            "AbC-123"
        );
    }

    #[test]
    fn rejects_invalid_invite_urls() {
        assert_eq!(
            parse_invite_url("https://example.com/invite/abc"),
            Err(ParseError::InviteUrl)
        );
        assert_eq!(
            parse_invite_url("https://discord.gg/"),
            Err(ParseError::InviteUrl)
        );
        assert_eq!(
            parse_invite_url("https://discord.gg/a/b"),
            Err(ParseError::InviteUrl)
        );
        assert_eq!(
            parse_invite_url("https://discord.com/abc"),
            Err(ParseError::InviteUrl)
        );
        assert_eq!(
            parse_invite_url("https://discord.com/invite/"),
            Err(ParseError::InviteUrl)
        );
        assert_eq!(
            parse_invite_url("discord.gg/abc"),
            Err(ParseError::InviteUrl)
        );
    }
}
