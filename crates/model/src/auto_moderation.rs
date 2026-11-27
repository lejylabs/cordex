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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keyword_filter: Vec<Box<str>>,
    /// Rust-flavored regex patterns (`KEYWORD`, `MEMBER_PROFILE`; max 10).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub regex_patterns: Vec<Box<str>>,
    /// Preset word lists (`KEYWORD_PRESET`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub presets: Vec<KeywordPresetType>,
    /// Keywords that never trigger the rule.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allow_list: Vec<Box<str>>,
    /// Max unique role + user mentions per message (`MENTION_SPAM`; max 50).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mention_total_limit: Option<u32>,
    /// Whether mention-raid detection is on (`MENTION_SPAM`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mention_raid_protection_enabled: Option<bool>,
}

impl TriggerMetadata {
    /// Empty trigger metadata.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Keyword filter metadata (`KEYWORD` or `MEMBER_PROFILE`).
    #[must_use]
    pub fn keywords<I, S>(keywords: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<Box<str>>,
    {
        Self::new().with_keywords(keywords)
    }

    /// Regex pattern metadata (`KEYWORD` or `MEMBER_PROFILE`).
    #[must_use]
    pub fn regex_patterns<I, S>(patterns: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<Box<str>>,
    {
        Self::new().with_regex_patterns(patterns)
    }

    /// Preset word list metadata (`KEYWORD_PRESET`).
    #[must_use]
    pub fn presets<I>(presets: I) -> Self
    where
        I: IntoIterator<Item = KeywordPresetType>,
    {
        Self::new().with_presets(presets)
    }

    /// Mention spam metadata (`MENTION_SPAM`; max 50).
    #[must_use]
    pub fn mention_spam(limit: u32) -> Self {
        Self::new().with_mention_total_limit(limit)
    }

    /// Set keyword substrings (`KEYWORD`, `MEMBER_PROFILE`; max 1000).
    #[must_use]
    pub fn with_keywords<I, S>(mut self, keywords: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<Box<str>>,
    {
        self.keyword_filter = keywords.into_iter().map(Into::into).collect();
        self
    }

    /// Set Rust-flavored regex patterns (`KEYWORD`, `MEMBER_PROFILE`; max 10).
    #[must_use]
    pub fn with_regex_patterns<I, S>(mut self, patterns: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<Box<str>>,
    {
        self.regex_patterns = patterns.into_iter().map(Into::into).collect();
        self
    }

    /// Set preset word lists (`KEYWORD_PRESET`).
    #[must_use]
    pub fn with_presets<I>(mut self, presets: I) -> Self
    where
        I: IntoIterator<Item = KeywordPresetType>,
    {
        self.presets = presets.into_iter().collect();
        self
    }

    /// Set allow-listed substrings that never trigger the rule.
    #[must_use]
    pub fn with_allow_list<I, S>(mut self, allow_list: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<Box<str>>,
    {
        self.allow_list = allow_list.into_iter().map(Into::into).collect();
        self
    }

    /// Set max unique role + user mentions per message (`MENTION_SPAM`; max 50).
    #[must_use]
    pub fn with_mention_total_limit(mut self, limit: u32) -> Self {
        self.mention_total_limit = Some(limit);
        self
    }

    /// Enable or disable mention-raid protection (`MENTION_SPAM`).
    #[must_use]
    pub fn with_mention_raid_protection(mut self, enabled: bool) -> Self {
        self.mention_raid_protection_enabled = Some(enabled);
        self
    }

    /// Check Discord's documented limits before sending the metadata to the API.
    pub fn validate(&self) -> Result<(), Box<str>> {
        if self.keyword_filter.len() > 1000 {
            return Err(format!(
                "keyword_filter has {} entries; max is 1000",
                self.keyword_filter.len()
            )
            .into());
        }
        for kw in &self.keyword_filter {
            let len = kw.chars().count();
            if len > 60 {
                return Err(format!("keyword '{kw}' is {len} characters; max is 60").into());
            }
        }
        if self.regex_patterns.len() > 10 {
            return Err(format!(
                "regex_patterns has {} entries; max is 10",
                self.regex_patterns.len()
            )
            .into());
        }
        for pat in &self.regex_patterns {
            let len = pat.chars().count();
            if len > 260 {
                return Err(format!("regex pattern is {len} characters; max is 260").into());
            }
        }
        if self.allow_list.len() > 1000 {
            return Err(format!(
                "allow_list has {} entries; max is 1000",
                self.allow_list.len()
            )
            .into());
        }
        for entry in &self.allow_list {
            let len = entry.chars().count();
            if len > 60 {
                return Err(
                    format!("allow_list entry '{entry}' is {len} characters; max is 60").into(),
                );
            }
        }
        if let Some(limit) = self.mention_total_limit
            && limit > 50
        {
            return Err(format!("mention_total_limit is {limit}; max is 50").into());
        }
        Ok(())
    }
}

/// An action taken when a rule matches.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct AutoModerationAction {
    /// Action type.
    #[serde(rename = "type")]
    pub kind: AutoModerationActionType,
    /// Type-specific data (omitted for some types).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<ActionMetadata>,
}

impl AutoModerationAction {
    /// Block the message with Discord's default explanation.
    #[must_use]
    pub fn block_message() -> Self {
        Self {
            kind: AutoModerationActionType::BlockMessage,
            metadata: None,
        }
    }

    /// Block the message with a custom explanation (max 150 characters).
    #[must_use]
    pub fn block_message_with(custom_message: impl Into<Box<str>>) -> Self {
        Self {
            kind: AutoModerationActionType::BlockMessage,
            metadata: Some(ActionMetadata {
                custom_message: Some(custom_message.into()),
                ..ActionMetadata::default()
            }),
        }
    }

    /// Send an alert message to `channel_id`.
    #[must_use]
    pub fn send_alert(channel_id: Id<ChannelMarker>) -> Self {
        Self {
            kind: AutoModerationActionType::SendAlertMessage,
            metadata: Some(ActionMetadata {
                channel_id: Some(channel_id),
                ..ActionMetadata::default()
            }),
        }
    }

    /// Timeout the member for `duration_seconds` (max 2,419,200 = 4 weeks).
    #[must_use]
    pub fn timeout(duration_seconds: u32) -> Self {
        Self {
            kind: AutoModerationActionType::Timeout,
            metadata: Some(ActionMetadata {
                duration_seconds: Some(duration_seconds),
                ..ActionMetadata::default()
            }),
        }
    }

    /// Block the member from interacting until their profile passes `MEMBER_PROFILE` rules.
    #[must_use]
    pub fn block_member_interaction() -> Self {
        Self {
            kind: AutoModerationActionType::BlockMemberInteraction,
            metadata: None,
        }
    }

    /// Check Discord's documented limits before sending the action to the API.
    pub fn validate(&self) -> Result<(), Box<str>> {
        if let Some(meta) = &self.metadata {
            if let Some(msg) = &meta.custom_message {
                let len = msg.chars().count();
                if len > 150 {
                    return Err(format!("custom_message is {len} characters; max is 150").into());
                }
            }
            if let Some(secs) = meta.duration_seconds
                && secs > 2_419_200
            {
                return Err(format!(
                    "timeout duration is {secs} seconds; max is 2419200 (4 weeks)"
                )
                .into());
            }
        }
        Ok(())
    }
}

/// Extra data for an action type.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ActionMetadata {
    /// Alert channel (`SEND_ALERT_MESSAGE`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel_id: Option<Id<ChannelMarker>>,
    /// Timeout length in seconds (`TIMEOUT`; max 2419200 = 4 weeks).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_seconds: Option<u32>,
    /// Explanation shown when a message is blocked (`BLOCK_MESSAGE`; max 150).
    #[serde(default, skip_serializing_if = "Option::is_none")]
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

    #[test]
    fn action_builders_omit_unused_fields() {
        let block = serde_json::to_value(AutoModerationAction::block_message()).unwrap();
        assert_eq!(block, serde_json::json!({ "type": 1 }));

        let custom =
            serde_json::to_value(AutoModerationAction::block_message_with("No spam")).unwrap();
        assert_eq!(
            custom,
            serde_json::json!({ "type": 1, "metadata": { "custom_message": "No spam" } })
        );

        let alert =
            serde_json::to_value(AutoModerationAction::send_alert(Id::new(42).unwrap())).unwrap();
        assert_eq!(
            alert,
            serde_json::json!({ "type": 2, "metadata": { "channel_id": "42" } })
        );

        let timeout = serde_json::to_value(AutoModerationAction::timeout(60)).unwrap();
        assert_eq!(
            timeout,
            serde_json::json!({ "type": 3, "metadata": { "duration_seconds": 60 } })
        );

        let interaction =
            serde_json::to_value(AutoModerationAction::block_member_interaction()).unwrap();
        assert_eq!(interaction, serde_json::json!({ "type": 4 }));
    }

    #[test]
    fn trigger_metadata_builders_omit_unused_fields() {
        let kw = TriggerMetadata::keywords(["bad", "worse"]).with_allow_list(["badge"]);
        assert_eq!(
            serde_json::to_value(kw).unwrap(),
            serde_json::json!({ "keyword_filter": ["bad", "worse"], "allow_list": ["badge"] })
        );

        let presets =
            TriggerMetadata::presets([KeywordPresetType::Profanity, KeywordPresetType::Slurs]);
        assert_eq!(
            serde_json::to_value(presets).unwrap(),
            serde_json::json!({ "presets": [1, 3] })
        );

        let mention = TriggerMetadata::mention_spam(5).with_mention_raid_protection(true);
        assert_eq!(
            serde_json::to_value(mention).unwrap(),
            serde_json::json!({
                "mention_total_limit": 5,
                "mention_raid_protection_enabled": true
            })
        );
    }

    #[test]
    fn validation_catches_limit_violations() {
        assert!(TriggerMetadata::keywords(["ok"]).validate().is_ok());
        assert!(
            TriggerMetadata::keywords(["x".repeat(61)])
                .validate()
                .is_err()
        );
        assert!(TriggerMetadata::mention_spam(51).validate().is_err());
        assert!(AutoModerationAction::timeout(2_419_200).validate().is_ok());
        assert!(AutoModerationAction::timeout(2_419_201).validate().is_err());
        assert!(
            AutoModerationAction::block_message_with("x".repeat(151))
                .validate()
                .is_err()
        );
    }
}
