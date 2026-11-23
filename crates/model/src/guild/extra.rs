//! Guild sub-resources: preview, widget, onboarding, prune, vanity URL,
//! bulk bans (resources/guild.mdx).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::GuildFeature;
use crate::emoji::Emoji;
use crate::id::Id;
use crate::id::marker::{
    ChannelMarker, EmojiMarker, GenericMarker, GuildMarker, RoleMarker, UserMarker,
};
use crate::image_hash::ImageHash;
use crate::serde_util::int_enum;
use crate::sticker::Sticker;

/// Public preview of a discoverable guild.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct GuildPreview {
    /// Guild ID.
    pub id: Id<GuildMarker>,
    /// Name.
    pub name: Box<str>,
    /// Icon hash.
    pub icon: Option<ImageHash>,
    /// Splash hash.
    pub splash: Option<ImageHash>,
    /// Discovery splash hash.
    pub discovery_splash: Option<ImageHash>,
    /// Custom emojis.
    #[serde(default)]
    pub emojis: Vec<Emoji>,
    /// Enabled features.
    #[serde(default)]
    pub features: Vec<GuildFeature>,
    /// Approximate member count.
    #[serde(default)]
    pub approximate_member_count: u64,
    /// Approximate online count.
    #[serde(default)]
    pub approximate_presence_count: u64,
    /// Description.
    pub description: Option<Box<str>>,
    /// Custom stickers.
    #[serde(default)]
    pub stickers: Vec<Sticker>,
}

/// Widget settings (`GET/PATCH /guilds/{id}/widget`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[non_exhaustive]
pub struct GuildWidgetSettings {
    /// Whether the widget is on.
    pub enabled: bool,
    /// Channel the widget invite points to.
    pub channel_id: Option<Id<ChannelMarker>>,
}

/// Public widget data (`GET /guilds/{id}/widget.json`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct GuildWidget {
    /// Guild ID.
    pub id: Id<GuildMarker>,
    /// Guild name.
    pub name: Box<str>,
    /// Invite URL of the widget channel.
    pub instant_invite: Option<Box<str>>,
    /// Voice and stage channels everyone can join.
    #[serde(default)]
    pub channels: Vec<GuildWidgetChannel>,
    /// Online members (max. 100, anonymized IDs).
    #[serde(default)]
    pub members: Vec<GuildWidgetMember>,
    /// Number of online members.
    #[serde(default)]
    pub presence_count: u64,
}

/// Channel in the widget.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct GuildWidgetChannel {
    /// Channel ID.
    pub id: Id<ChannelMarker>,
    /// Name.
    pub name: Box<str>,
    /// Sort position.
    #[serde(default)]
    pub position: i32,
}

/// Member in the widget (IDs are anonymized by Discord).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct GuildWidgetMember {
    /// Anonymized index, not a real user ID.
    pub id: Box<str>,
    /// Username.
    pub username: Box<str>,
    /// Status (`online`, `idle`, `dnd`).
    #[serde(default)]
    pub status: Box<str>,
    /// Avatar URL.
    pub avatar_url: Option<Box<str>>,
}

/// Onboarding flow of a guild.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Onboarding {
    /// Guild ID.
    pub guild_id: Id<GuildMarker>,
    /// Prompts shown during onboarding and in "Channels & Roles".
    #[serde(default)]
    pub prompts: Vec<OnboardingPrompt>,
    /// Channels members get opted into automatically.
    #[serde(default)]
    pub default_channel_ids: Vec<Id<ChannelMarker>>,
    /// Whether onboarding is on.
    pub enabled: bool,
    /// Which channels count towards the constraints.
    pub mode: OnboardingMode,
}

/// One onboarding question.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct OnboardingPrompt {
    /// Prompt ID (any unique snowflake for new prompts).
    pub id: Id<GenericMarker>,
    /// Multiple choice or dropdown.
    #[serde(rename = "type")]
    pub kind: OnboardingPromptType,
    /// Answers.
    #[serde(default)]
    pub options: Vec<OnboardingPromptOption>,
    /// Question.
    pub title: Box<str>,
    /// Only one answer allowed.
    #[serde(default)]
    pub single_select: bool,
    /// Must be answered.
    #[serde(default)]
    pub required: bool,
    /// Shown in onboarding (otherwise only in "Channels & Roles").
    #[serde(default)]
    pub in_onboarding: bool,
}

/// One answer of an onboarding prompt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct OnboardingPromptOption {
    /// Option ID.
    pub id: Id<GenericMarker>,
    /// Channels the member is added to.
    #[serde(default)]
    pub channel_ids: Vec<Id<ChannelMarker>>,
    /// Roles the member gets.
    #[serde(default)]
    pub role_ids: Vec<Id<RoleMarker>>,
    /// Emoji (in responses).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub emoji: Option<Emoji>,
    /// Emoji ID (in requests).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub emoji_id: Option<Id<EmojiMarker>>,
    /// Emoji name (in requests).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub emoji_name: Option<Box<str>>,
    /// Emoji animated (in requests).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub emoji_animated: Option<bool>,
    /// Title.
    pub title: Box<str>,
    /// Description.
    pub description: Option<Box<str>>,
}

int_enum! {
    /// Onboarding mode.
    pub enum OnboardingMode: u8 {
        /// Only default channels count towards the constraints.
        Default = 0,
        /// Default channels and questions count.
        Advanced = 1,
    }
}

int_enum! {
    /// Onboarding prompt type.
    pub enum OnboardingPromptType: u8 {
        /// Multiple choice.
        MultipleChoice = 0,
        /// Dropdown.
        Dropdown = 1,
    }
}

/// Result of a prune (`pruned` is `None` when `compute_prune_count` was
/// off).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct PruneCount {
    /// Members that were (or would be) removed.
    pub pruned: Option<u64>,
}

/// Vanity invite of a guild.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct VanityUrl {
    /// Invite code, if set.
    pub code: Option<Box<str>>,
    /// Number of uses.
    #[serde(default)]
    pub uses: u64,
}

/// Result of a bulk ban.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct BulkBan {
    /// Users that were banned.
    #[serde(default)]
    pub banned_users: Vec<Id<UserMarker>>,
    /// Users that could not be banned.
    #[serde(default)]
    pub failed_users: Vec<Id<UserMarker>>,
}

/// Members per role (`GET /guilds/{id}/roles/member-counts`).
pub type RoleMemberCounts = BTreeMap<Id<RoleMarker>, u64>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn onboarding_round_trip() {
        let json = r#"{"guild_id":"1","prompts":[{"id":"2","type":0,"options":[{"id":"3","channel_ids":["4"],"role_ids":[],"emoji":{"id":null,"name":"x"},"title":"T","description":null}],"title":"Q","single_select":false,"required":false,"in_onboarding":true}],"default_channel_ids":["4"],"enabled":true,"mode":1}"#;
        let onboarding: Onboarding = serde_json::from_str(json).unwrap();
        assert_eq!(onboarding.mode, OnboardingMode::Advanced);
        assert_eq!(onboarding.prompts[0].options[0].channel_ids.len(), 1);
        let back: Onboarding =
            serde_json::from_value(serde_json::to_value(&onboarding).unwrap()).unwrap();
        assert_eq!(back, onboarding);
    }

    #[test]
    fn role_member_counts_parse() {
        let counts: RoleMemberCounts = serde_json::from_str(r#"{"5":10,"6":0}"#).unwrap();
        assert_eq!(counts.len(), 2);
    }
}
