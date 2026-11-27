# Unreleased

Changes currently on `master` and scheduled for the next release.

- **Auto Moderation Helpers (`cordex-model` & `cordex-http`):**
  - Added typed constructors on `AutoModerationAction` (`block_message`, `block_message_with`, `send_alert`, `timeout`, `block_member_interaction`) and fluent builders on `TriggerMetadata` (`new`, `keywords`, `regex_patterns`, `presets`, `mention_spam`, `with_keywords`, `with_regex_patterns`, `with_presets`, `with_allow_list`, `with_mention_total_limit`, `with_mention_raid_protection`).
  - Added `#[serde(skip_serializing_if = ...)]` on `TriggerMetadata`, `AutoModerationAction::metadata`, and `ActionMetadata` so unused fields are omitted when serializing payloads to Discord.
  - Added convenience methods on `cordex_http::Client`: `create_keyword_rule`, `create_spam_rule`, `create_keyword_preset_rule`, `create_mention_spam_rule`, and `create_member_profile_rule`.
