# Unreleased

Changes currently on `master` and scheduled for the next release.

- **Auto Moderation Helpers (`cordex-model`, `cordex-http`, `cordex`):**
  - Added typed constructors on `AutoModerationAction` (`block_message`, `block_message_with`, `send_alert`, `timeout`, `block_member_interaction`), fluent builders on `TriggerMetadata` (`new`, `keywords`, `regex_patterns`, `presets`, `mention_spam`, `with_*`), and local Discord limit validation via `TriggerMetadata::validate` / `AutoModerationAction::validate`.
  - Added `#[serde(skip_serializing_if = ...)]` on `TriggerMetadata`, `AutoModerationAction::metadata`, and `ActionMetadata` so unused fields are omitted when serializing payloads to Discord.
  - Added convenience methods on `cordex_http::Client`: `create_keyword_rule`, `create_spam_rule`, `create_keyword_preset_rule`, `create_mention_spam_rule`, `create_member_profile_rule`, and idempotent `upsert_auto_moderation_rule`.
  - Added typed AutoMod event handlers on `Bot` and `Module`: `on_auto_moderation_action_execution`, `on_auto_moderation_rule_create`, `on_auto_moderation_rule_update`, and `on_auto_moderation_rule_delete` (automatically enabling `AUTO_MODERATION_EXECUTION` / `AUTO_MODERATION_CONFIGURATION` intents).
