//! Audit logs (resources/audit-log.mdx).

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::auto_moderation::AutoModerationRule;
use crate::channel::Channel;
use crate::command::ApplicationCommand;
use crate::guild::Integration;
use crate::id::Id;
use crate::id::marker::{
    ApplicationMarker, AuditLogEntryMarker, ChannelMarker, GenericMarker, GuildMarker,
    MessageMarker, UserMarker,
};
use crate::scheduled_event::ScheduledEvent;
use crate::serde_util::int_enum;
use crate::user::User;
use crate::webhook::Webhook;

int_enum! {
    /// Audit log event (`action_type`).
    pub enum AuditLogEvent: u16 {
        /// Guild settings updated.
        GuildUpdate = 1,
        /// Channel created.
        ChannelCreate = 10,
        /// Channel updated.
        ChannelUpdate = 11,
        /// Channel deleted.
        ChannelDelete = 12,
        /// Channel overwrite created.
        ChannelOverwriteCreate = 13,
        /// Channel overwrite updated.
        ChannelOverwriteUpdate = 14,
        /// Channel overwrite deleted.
        ChannelOverwriteDelete = 15,
        /// Channels reordered.
        ChannelPositionUpdate = 16,
        /// Member kicked.
        MemberKick = 20,
        /// Members pruned.
        MemberPrune = 21,
        /// Member banned.
        MemberBanAdd = 22,
        /// Member unbanned.
        MemberBanRemove = 23,
        /// Member updated.
        MemberUpdate = 24,
        /// Member roles updated.
        MemberRoleUpdate = 25,
        /// Member moved in voice.
        MemberMove = 26,
        /// Member disconnected from voice.
        MemberDisconnect = 27,
        /// Bot added.
        BotAdd = 28,
        /// Role created.
        RoleCreate = 30,
        /// Role updated.
        RoleUpdate = 31,
        /// Role deleted.
        RoleDelete = 32,
        /// Roles reordered.
        RolePositionUpdate = 33,
        /// Invite created.
        InviteCreate = 40,
        /// Invite updated.
        InviteUpdate = 41,
        /// Invite deleted.
        InviteDelete = 42,
        /// Webhook created.
        WebhookCreate = 50,
        /// Webhook updated.
        WebhookUpdate = 51,
        /// Webhook deleted.
        WebhookDelete = 52,
        /// Emoji created.
        EmojiCreate = 60,
        /// Emoji updated.
        EmojiUpdate = 61,
        /// Emoji deleted.
        EmojiDelete = 62,
        /// Message deleted.
        MessageDelete = 72,
        /// Messages bulk-deleted.
        MessageBulkDelete = 73,
        /// Message pinned.
        MessagePin = 74,
        /// Message unpinned.
        MessageUnpin = 75,
        /// Integration created.
        IntegrationCreate = 80,
        /// Integration updated.
        IntegrationUpdate = 81,
        /// Integration deleted.
        IntegrationDelete = 82,
        /// Stage instance created.
        StageInstanceCreate = 83,
        /// Stage instance updated.
        StageInstanceUpdate = 84,
        /// Stage instance deleted.
        StageInstanceDelete = 85,
        /// Sticker created.
        StickerCreate = 90,
        /// Sticker updated.
        StickerUpdate = 91,
        /// Sticker deleted.
        StickerDelete = 92,
        /// Scheduled event created.
        GuildScheduledEventCreate = 100,
        /// Scheduled event updated.
        GuildScheduledEventUpdate = 101,
        /// Scheduled event deleted.
        GuildScheduledEventDelete = 102,
        /// Thread created.
        ThreadCreate = 110,
        /// Thread updated.
        ThreadUpdate = 111,
        /// Thread deleted.
        ThreadDelete = 112,
        /// Command permissions updated.
        ApplicationCommandPermissionUpdate = 121,
        /// Soundboard sound created.
        SoundboardSoundCreate = 130,
        /// Soundboard sound updated.
        SoundboardSoundUpdate = 131,
        /// Soundboard sound deleted.
        SoundboardSoundDelete = 132,
        /// Auto moderation rule created.
        AutoModerationRuleCreate = 140,
        /// Auto moderation rule updated.
        AutoModerationRuleUpdate = 141,
        /// Auto moderation rule deleted.
        AutoModerationRuleDelete = 142,
        /// Message blocked by auto moderation.
        AutoModerationBlockMessage = 143,
        /// Message flagged to a channel.
        AutoModerationFlagToChannel = 144,
        /// Member timed out by auto moderation.
        AutoModerationUserCommunicationDisabled = 145,
        /// Member quarantined by auto moderation.
        AutoModerationQuarantineUser = 146,
        /// Creator monetization request created.
        CreatorMonetizationRequestCreated = 150,
        /// Creator monetization terms accepted.
        CreatorMonetizationTermsAccepted = 151,
        /// Onboarding prompt created.
        OnboardingPromptCreate = 163,
        /// Onboarding prompt updated.
        OnboardingPromptUpdate = 164,
        /// Onboarding prompt deleted.
        OnboardingPromptDelete = 165,
        /// Onboarding created.
        OnboardingCreate = 166,
        /// Onboarding updated.
        OnboardingUpdate = 167,
        /// Server guide created.
        HomeSettingsCreate = 190,
        /// Server guide updated.
        HomeSettingsUpdate = 191,
        /// Voice channel status set.
        VoiceChannelStatusCreate = 192,
        /// Voice channel status deleted.
        VoiceChannelStatusDelete = 193,
    }
}

/// `GET /guilds/{guild.id}/audit-logs` response.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct AuditLog {
    /// Commands referenced by entries.
    #[serde(default)]
    pub application_commands: Vec<ApplicationCommand>,
    /// Entries, newest first (unless `after` was used).
    #[serde(default)]
    pub audit_log_entries: Vec<AuditLogEntry>,
    /// Auto moderation rules referenced by entries.
    #[serde(default)]
    pub auto_moderation_rules: Vec<AutoModerationRule>,
    /// Scheduled events referenced by entries.
    #[serde(default)]
    pub guild_scheduled_events: Vec<ScheduledEvent>,
    /// Partial integrations referenced by entries.
    #[serde(default)]
    pub integrations: Vec<Integration>,
    /// Threads referenced by thread create/update entries.
    #[serde(default)]
    pub threads: Vec<Channel>,
    /// Users referenced by entries.
    #[serde(default)]
    pub users: Vec<User>,
    /// Webhooks referenced by entries.
    #[serde(default)]
    pub webhooks: Vec<Webhook>,
}

/// One administrative action. Gateway `GUILD_AUDIT_LOG_ENTRY_CREATE` adds
/// [`AuditLogEntry::guild_id`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct AuditLogEntry {
    /// Affected entity ID (webhook, user, role, …); string, not always a snowflake.
    pub target_id: Option<Box<str>>,
    /// Field changes.
    #[serde(default)]
    pub changes: Vec<AuditLogChange>,
    /// User or app that made the change.
    pub user_id: Option<Id<UserMarker>>,
    /// Entry ID.
    pub id: Id<AuditLogEntryMarker>,
    /// Action type.
    pub action_type: AuditLogEvent,
    /// Extra info for some action types.
    pub options: Option<OptionalAuditEntryInfo>,
    /// Reason (`X-Audit-Log-Reason`, 1–512 characters).
    pub reason: Option<Box<str>>,
    /// Guild (gateway event only).
    pub guild_id: Option<Id<GuildMarker>>,
}

/// One changed field. `old_value` / `new_value` keep the raw JSON because
/// the type depends on the key and action.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct AuditLogChange {
    /// New value; omitted when the field was reset to null.
    pub new_value: Option<Value>,
    /// Old value; omitted when the field was previously null.
    pub old_value: Option<Value>,
    /// Changed field name (`$add` / `$remove` for member roles).
    pub key: Box<str>,
}

/// Extra fields on some audit log events. Numeric counts arrive as strings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct OptionalAuditEntryInfo {
    /// App whose command permissions were targeted.
    pub application_id: Option<Id<ApplicationMarker>>,
    /// Auto moderation rule name.
    pub auto_moderation_rule_name: Option<Box<str>>,
    /// Auto moderation trigger type (string).
    pub auto_moderation_rule_trigger_type: Option<Box<str>>,
    /// Channel of the targeted entities.
    pub channel_id: Option<Id<ChannelMarker>>,
    /// Number of entities or position updates (string).
    pub count: Option<Box<str>>,
    /// Inactive days for a prune (string).
    pub delete_member_days: Option<Box<str>>,
    /// Overwritten entity (role or member).
    pub id: Option<Id<GenericMarker>>,
    /// Members removed by a prune (string).
    pub members_removed: Option<Box<str>>,
    /// Pinned or unpinned message.
    pub message_id: Option<Id<MessageMarker>>,
    /// Role name when overwrite `type` is `"0"`.
    pub role_name: Option<Box<str>>,
    /// Overwrite target: `"0"` role, `"1"` member.
    #[serde(rename = "type")]
    pub kind: Option<Box<str>>,
    /// Integration type that performed a kick or role update.
    pub integration_type: Option<Box<str>>,
    /// New voice channel status.
    pub status: Option<Box<str>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entry_round_trip_and_unknown_action() {
        let json = r#"{
            "target_id":"1","user_id":"2","id":"3","action_type":999,
            "changes":[{"key":"name","old_value":"a","new_value":"b"}],
            "reason":"cleanup","guild_id":"4"
        }"#;
        let entry: AuditLogEntry = serde_json::from_str(json).unwrap();
        assert_eq!(entry.action_type, AuditLogEvent::Unknown(999));
        assert_eq!(entry.changes[0].key.as_ref(), "name");
        assert_eq!(entry.reason.as_deref(), Some("cleanup"));
        let out = serde_json::to_string(&entry).unwrap();
        let again: AuditLogEntry = serde_json::from_str(&out).unwrap();
        assert_eq!(entry, again);
    }
}
