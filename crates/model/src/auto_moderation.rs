//! Auto moderation rules and actions (resources/auto-moderation.mdx).

use serde::{Deserialize, Serialize};

use crate::id::Id;
use crate::id::marker::{
    AutoModerationRuleMarker, ChannelMarker, GuildMarker, MessageMarker, RoleMarker, UserMarker,
};
use crate::serde_util::int_enum;

int_enum! {
    /// When a rule is evaluated.
    pub enum AutoModerationEventType: u8 {
        /// A member sends or edits a message.
        MessageSend = 1,
        /// A member edits their profile.
        MemberUpdate = 2,
    }
}

int_enum! {
    /// What content a rule looks at.
    pub enum AutoModerationTriggerType: u8 {
        /// User-defined keywords.
        Keyword = 1,
        /// Generic spam.
        Spam = 3,
        /// Discord's preset word lists.
        KeywordPreset = 4,
        /// Too many unique mentions.
        MentionSpam = 5,
        /// Keywords in a member profile.
        MemberProfile = 6,
    }
}

int_enum! {
    /// Discord-defined keyword lists.
    pub enum KeywordPresetType: u8 {
        /// Swearing / cursing.
        Profanity = 1,
        /// Sexually explicit language.
        SexualContent = 2,
        /// Slurs / hate speech.
        Slurs = 3,
    }
}

int_enum! {
    /// What happens when a rule matches.
    pub enum AutoModerationActionType: u8 {
        /// Block the message; optional custom explanation.
        BlockMessage = 1,
        /// Log the content to a channel.
        SendAlertMessage = 2,
        /// Timeout the member.
        Timeout = 3,
        /// Block text, voice and other interactions.
        BlockMemberInteraction = 4,
    }
}

/// A guild auto moderation rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct AutoModerationRule {
    /// Rule ID.
    pub id: Id<AutoModerationRuleMarker>,
    /// Guild.
    pub guild_id: Id<GuildMarker>,
    /// Display name.
    pub name: Box<str>,
    /// User who first created the rule.
    pub creator_id: Id<UserMarker>,
    /// When the rule is checked.
    pub event_type: AutoModerationEventType,
    /// What content is scanned.
    pub trigger_type: AutoModerationTriggerType,
    /// Trigger-specific configuration (empty object for some trigger types).
    #[serde(default)]
    pub trigger_metadata: TriggerMetadata,
    /// Actions run when the rule matches.
    #[serde(default)]
    pub actions: Vec<AutoModerationAction>,
    /// Whether the rule is on.
    #[serde(default)]
    pub enabled: bool,
    /// Roles the rule ignores (max 20).
    #[serde(default)]
    pub exempt_roles: Vec<Id<RoleMarker>>,
    /// Channels the rule ignores (max 50).
    #[serde(default)]
    pub exempt_channels: Vec<Id<ChannelMarker>>,
}

/// Extra data for a trigger type. Unused fields are empty / `None`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[non_exhaustive]
pub struct TriggerMetadata {
    /// Keyword substrings (`KEYWORD`, `MEMBER_PROFILE`; max 1000, 60 chars each).
    #[serde(default)]
    pub keyword_filter: Vec<Box<str>>,
    /// Rust-flavored regex patterns (`KEYWORD`, `MEMBER_PROFILE`; max 10).
    #[serde(default)]
    pub regex_patterns: Vec<Box<str>>,
    /// Preset word lists (`KEYWORD_PRESET`).
    #[serde(default)]
    pub presets: Vec<KeywordPresetType>,
    /// Keywords that never trigger the rule.
    #[serde(default)]
    pub allow_list: Vec<Box<str>>,
    /// Max unique role + user mentions per message (`MENTION_SPAM`; max 50).
    pub mention_total_limit: Option<u32>,
    /// Whether mention-raid detection is on (`MENTION_SPAM`).
    pub mention_raid_protection_enabled: Option<bool>,
}

/// An action taken when a rule matches.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct AutoModerationAction {
    /// Action type.
    #[serde(rename = "type")]
    pub kind: AutoModerationActionType,
    /// Type-specific data (omitted for some types).
    pub metadata: Option<ActionMetadata>,
}

/// Extra data for an action type.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ActionMetadata {
    /// Alert channel (`SEND_ALERT_MESSAGE`).
    pub channel_id: Option<Id<ChannelMarker>>,
    /// Timeout length in seconds (`TIMEOUT`; max 2419200 = 4 weeks).
    pub duration_seconds: Option<u32>,
    /// Explanation shown when a message is blocked (`BLOCK_MESSAGE`; max 150).
    pub custom_message: Option<Box<str>>,
}

/// `AUTO_MODERATION_ACTION_EXECUTION` payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct AutoModerationActionExecution {
    /// Guild.
    pub guild_id: Id<GuildMarker>,
    /// Action that ran.
    pub action: AutoModerationAction,
    /// Rule.
    pub rule_id: Id<AutoModerationRuleMarker>,
    /// Trigger type of that rule.
    pub rule_trigger_type: AutoModerationTriggerType,
    /// User whose content matched.
    pub user_id: Id<UserMarker>,
    /// Channel the content was posted in.
    pub channel_id: Option<Id<ChannelMarker>>,
    /// Matching message; absent when the message was blocked.
    pub message_id: Option<Id<MessageMarker>>,
    /// System alert message (`SEND_ALERT_MESSAGE` only).
    pub alert_system_message_id: Option<Id<MessageMarker>>,
    /// User-generated content (empty without `MESSAGE_CONTENT`).
    #[serde(default)]
    pub content: Box<str>,
    /// Configured keyword that matched.
    pub matched_keyword: Option<Box<str>>,
    /// Substring of `content` that matched (empty without `MESSAGE_CONTENT`).
    pub matched_content: Option<Box<str>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_trigger_survives() {
        let json = r#"{"id":"1","guild_id":"2","name":"x","creator_id":"3",
            "event_type":1,"trigger_type":99,"trigger_metadata":{},"actions":[],
            "enabled":false,"exempt_roles":[],"exempt_channels":[]}"#;
        let rule: AutoModerationRule = serde_json::from_str(json).unwrap();
        assert_eq!(rule.trigger_type, AutoModerationTriggerType::Unknown(99));
        let out = serde_json::to_string(&rule).unwrap();
        let again: AutoModerationRule = serde_json::from_str(&out).unwrap();
        assert_eq!(rule, again);
    }
}
