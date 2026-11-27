//! Auto moderation, stage instances, monetization (entitlements, SKUs,
//! subscriptions), the current application and polls.

use std::future::Future;

use cordex_model::application::{
    ActivityInstance, Application, EventWebhookStatus, RoleConnectionMetadata,
};
use cordex_model::auto_moderation::{
    AutoModerationAction, AutoModerationEventType, AutoModerationRule, AutoModerationTriggerType,
    TriggerMetadata,
};
use cordex_model::entitlement::Entitlement;
use cordex_model::id::Id;
use cordex_model::id::marker::{
    ApplicationMarker, AutoModerationRuleMarker, ChannelMarker, EntitlementMarker, GuildMarker,
    MessageMarker, RoleMarker, ScheduledEventMarker, SkuMarker, SubscriptionMarker, UserMarker,
};
use cordex_model::message::Message;
use cordex_model::sku::{Sku, Subscription};
use cordex_model::stage::{StageInstance, StagePrivacyLevel};
use cordex_model::user::User;
use http::Method;
use serde::Deserialize;

use crate::client::Client;
use crate::error::HttpError;
use crate::route::Route;

json_request! {
    /// `POST` / `PATCH` of an auto moderation rule.
    AutoModerationRuleRequest -> AutoModerationRule => send_json {
        /// Rule name.
        name: &str,
        /// When the rule runs.
        event_type: AutoModerationEventType,
        /// Trigger details (keywords, presets, mention limit, ...).
        trigger_metadata: &TriggerMetadata,
        /// What happens on a match.
        actions: &[AutoModerationAction],
        /// Rule on or off.
        enabled: bool,
        /// Roles the rule ignores (max. 20).
        exempt_roles: &[Id<RoleMarker>],
        /// Channels the rule ignores (max. 50).
        exempt_channels: &[Id<ChannelMarker>],
    }
}

json_request! {
    /// `POST /stage-instances`: start a stage.
    CreateStageInstance -> StageInstance => send_json {
        /// Who can see it (default guild only).
        privacy_level: StagePrivacyLevel,
        /// Notify `@everyone` (needs `MENTION_EVERYONE`).
        send_start_notification: bool,
        /// Scheduled event this stage belongs to.
        guild_scheduled_event_id: Id<ScheduledEventMarker>,
    }
}

json_request! {
    /// `PATCH /stage-instances/{channel_id}`.
    UpdateStageInstance -> StageInstance => send_json {
        /// New topic (1-120 characters).
        topic: &str,
        /// Who can see it.
        privacy_level: StagePrivacyLevel,
    }
}

json_request! {
    /// `PATCH /applications/@me`: edit the bot's application.
    UpdateCurrentApplication -> Application => send_json {
        /// Description.
        description: &str,
        /// Icon (data URI, `None` removes it).
        icon: Option<&str>,
        /// Cover image (data URI, `None` removes it).
        cover_image: Option<&str>,
        /// HTTP interactions endpoint URL.
        interactions_endpoint_url: Option<&str>,
        /// Linked roles verification URL.
        role_connections_verification_url: Option<&str>,
        /// Custom install URL.
        custom_install_url: Option<&str>,
        /// Up to 5 tags (max. 20 characters each).
        tags: &[&str],
        /// Application flags (only the gateway intent flags can change).
        flags: u64,
        /// Event webhooks URL.
        event_webhooks_url: Option<&str>,
        /// Event webhooks on or off.
        event_webhooks_status: EventWebhookStatus,
        /// Event types to receive.
        event_webhooks_types: &[&str],
    }
}

query_request! {
    /// `GET /skus/{sku_id}/subscriptions`.
    GetSkuSubscriptions -> Vec<Subscription> {
        /// Subscriptions before this ID.
        before: Id<SubscriptionMarker>,
        /// Subscriptions after this ID.
        after: Id<SubscriptionMarker>,
        /// Max. subscriptions (1-100, default 50).
        limit: u8,
        /// Subscriptions of this user (required for bot tokens).
        user_id: Id<UserMarker>,
    }
}

query_request! {
    /// `GET /channels/{channel_id}/polls/{message_id}/answers/{answer_id}`.
    GetAnswerVoters -> AnswerVoters {
        /// Users after this user ID.
        after: Id<UserMarker>,
        /// Max. users (1-100, default 25).
        limit: u8,
    }
}

/// Users who voted for a poll answer.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[non_exhaustive]
pub struct AnswerVoters {
    /// Voters.
    pub users: Vec<User>,
}

impl Client {
    // ---- auto moderation ---------------------------------------------------

    simple! {
        /// `GET /guilds/{guild_id}/auto-moderation/rules`. Needs `MANAGE_GUILD`.
        fn get_auto_moderation_rules(guild_id: Id<GuildMarker>)
            -> Vec<AutoModerationRule> = get "/guilds/{guild_id}/auto-moderation/rules" => send_json
    }

    simple! {
        /// `GET /guilds/{guild_id}/auto-moderation/rules/{rule_id}`.
        fn get_auto_moderation_rule(guild_id: Id<GuildMarker>, rule_id: Id<AutoModerationRuleMarker>)
            -> AutoModerationRule = get "/guilds/{guild_id}/auto-moderation/rules/{rule_id}" => send_json
    }

    /// `POST /guilds/{guild_id}/auto-moderation/rules`.
    pub fn create_auto_moderation_rule(
        &self,
        guild_id: Id<GuildMarker>,
        name: &str,
        event_type: AutoModerationEventType,
        trigger_type: AutoModerationTriggerType,
        actions: &[AutoModerationAction],
    ) -> AutoModerationRuleRequest {
        AutoModerationRuleRequest::new(
            self,
            Method::POST,
            Route::raw("/guilds/{guild_id}/auto-moderation/rules", guild_id),
        )
        .name(name)
        .event_type(event_type)
        .set("trigger_type", trigger_type)
        .actions(actions)
    }

    /// Create a `KEYWORD` auto moderation rule on `MessageSend`.
    pub fn create_keyword_rule(
        &self,
        guild_id: Id<GuildMarker>,
        name: &str,
        metadata: &TriggerMetadata,
        actions: &[AutoModerationAction],
    ) -> AutoModerationRuleRequest {
        self.create_auto_moderation_rule(
            guild_id,
            name,
            AutoModerationEventType::MessageSend,
            AutoModerationTriggerType::Keyword,
            actions,
        )
        .trigger_metadata(metadata)
    }

    /// Create a `SPAM` auto moderation rule on `MessageSend`.
    pub fn create_spam_rule(
        &self,
        guild_id: Id<GuildMarker>,
        name: &str,
        actions: &[AutoModerationAction],
    ) -> AutoModerationRuleRequest {
        self.create_auto_moderation_rule(
            guild_id,
            name,
            AutoModerationEventType::MessageSend,
            AutoModerationTriggerType::Spam,
            actions,
        )
    }

    /// Create a `KEYWORD_PRESET` auto moderation rule on `MessageSend`.
    pub fn create_keyword_preset_rule(
        &self,
        guild_id: Id<GuildMarker>,
        name: &str,
        presets: impl IntoIterator<Item = cordex_model::auto_moderation::KeywordPresetType>,
        actions: &[AutoModerationAction],
    ) -> AutoModerationRuleRequest {
        let metadata = TriggerMetadata::presets(presets);
        self.create_auto_moderation_rule(
            guild_id,
            name,
            AutoModerationEventType::MessageSend,
            AutoModerationTriggerType::KeywordPreset,
            actions,
        )
        .trigger_metadata(&metadata)
    }

    /// Create a `MENTION_SPAM` auto moderation rule on `MessageSend`.
    pub fn create_mention_spam_rule(
        &self,
        guild_id: Id<GuildMarker>,
        name: &str,
        mention_total_limit: u32,
        actions: &[AutoModerationAction],
    ) -> AutoModerationRuleRequest {
        let metadata = TriggerMetadata::mention_spam(mention_total_limit);
        self.create_auto_moderation_rule(
            guild_id,
            name,
            AutoModerationEventType::MessageSend,
            AutoModerationTriggerType::MentionSpam,
            actions,
        )
        .trigger_metadata(&metadata)
    }

    /// Create a `MEMBER_PROFILE` auto moderation rule on `MemberUpdate`.
    pub fn create_member_profile_rule(
        &self,
        guild_id: Id<GuildMarker>,
        name: &str,
        metadata: &TriggerMetadata,
        actions: &[AutoModerationAction],
    ) -> AutoModerationRuleRequest {
        self.create_auto_moderation_rule(
            guild_id,
            name,
            AutoModerationEventType::MemberUpdate,
            AutoModerationTriggerType::MemberProfile,
            actions,
        )
        .trigger_metadata(metadata)
    }

    /// Create or update an auto moderation rule idempotently.
    ///
    /// For singleton trigger types (`Spam`, `KeywordPreset`, `MentionSpam`),
    /// matches an existing rule of the same `trigger_type`. For multi-rule
    /// types (`Keyword`, `MemberProfile`), matches by `trigger_type` and `name`.
    /// Validates `trigger_metadata` and `actions` locally before sending.
    pub async fn upsert_auto_moderation_rule(
        &self,
        guild_id: Id<GuildMarker>,
        name: &str,
        event_type: AutoModerationEventType,
        trigger_type: AutoModerationTriggerType,
        trigger_metadata: &TriggerMetadata,
        actions: &[AutoModerationAction],
        enabled: bool,
    ) -> Result<AutoModerationRule, HttpError> {
        trigger_metadata.validate().map_err(HttpError::validation)?;
        for action in actions {
            action.validate().map_err(HttpError::validation)?;
        }

        let existing = self.get_auto_moderation_rules(guild_id).await?;
        let singleton = matches!(
            trigger_type,
            AutoModerationTriggerType::Spam
                | AutoModerationTriggerType::KeywordPreset
                | AutoModerationTriggerType::MentionSpam
        );
        let matched = existing.into_iter().find(|r| {
            r.trigger_type == trigger_type && (singleton || r.name.as_ref() == name)
        });

        if let Some(rule) = matched {
            self.update_auto_moderation_rule(guild_id, rule.id)
                .name(name)
                .event_type(event_type)
                .trigger_metadata(trigger_metadata)
                .actions(actions)
                .enabled(enabled)
                .await
        } else {
            self.create_auto_moderation_rule(guild_id, name, event_type, trigger_type, actions)
                .trigger_metadata(trigger_metadata)
                .enabled(enabled)
                .await
        }
    }

    /// `PATCH /guilds/{guild_id}/auto-moderation/rules/{rule_id}`.
    pub fn update_auto_moderation_rule(
        &self,
        guild_id: Id<GuildMarker>,
        rule_id: Id<AutoModerationRuleMarker>,
    ) -> AutoModerationRuleRequest {
        AutoModerationRuleRequest::new(
            self,
            Method::PATCH,
            Route::raw(
                "/guilds/{guild_id}/auto-moderation/rules/{rule_id}",
                (guild_id, rule_id),
            ),
        )
    }

    simple! {
        /// `DELETE /guilds/{guild_id}/auto-moderation/rules/{rule_id}`.
        fn delete_auto_moderation_rule(guild_id: Id<GuildMarker>, rule_id: Id<AutoModerationRuleMarker>)
            -> () = delete "/guilds/{guild_id}/auto-moderation/rules/{rule_id}" => send_empty
    }

    // ---- stage instances ---------------------------------------------------

    /// `POST /stage-instances`: open a stage in `channel_id` with a topic
    /// (1-120 characters). Needs to be a stage moderator.
    pub fn create_stage_instance(
        &self,
        channel_id: Id<ChannelMarker>,
        topic: &str,
    ) -> CreateStageInstance {
        CreateStageInstance::new(self, Method::POST, Route::raw("/stage-instances", ()))
            .set("channel_id", channel_id)
            .set("topic", topic)
    }

    simple! {
        /// `GET /stage-instances/{channel_id}`.
        fn get_stage_instance(channel_id: Id<ChannelMarker>)
            -> StageInstance = get "/stage-instances/{channel_id}" => send_json
    }

    /// `PATCH /stage-instances/{channel_id}`.
    pub fn update_stage_instance(&self, channel_id: Id<ChannelMarker>) -> UpdateStageInstance {
        UpdateStageInstance::new(
            self,
            Method::PATCH,
            Route::raw("/stage-instances/{channel_id}", channel_id),
        )
    }

    simple! {
        /// `DELETE /stage-instances/{channel_id}`: end the stage.
        fn delete_stage_instance(channel_id: Id<ChannelMarker>)
            -> () = delete "/stage-instances/{channel_id}" => send_empty
    }

    // ---- monetization ------------------------------------------------------

    simple! {
        /// `GET /applications/{application_id}/entitlements/{entitlement_id}`.
        fn get_entitlement(application_id: Id<ApplicationMarker>, entitlement_id: Id<EntitlementMarker>)
            -> Entitlement = get "/applications/{application_id}/entitlements/{entitlement_id}" => send_json
    }

    simple! {
        /// `POST /applications/{application_id}/entitlements/{entitlement_id}/consume`:
        /// mark a consumable one-time purchase as used.
        fn consume_entitlement(application_id: Id<ApplicationMarker>, entitlement_id: Id<EntitlementMarker>)
            -> () = post "/applications/{application_id}/entitlements/{entitlement_id}/consume" => send_empty
    }

    /// `POST /applications/{application_id}/entitlements`: test entitlement
    /// for a user or guild (owner `owner_type` 1 = guild, 2 = user).
    pub fn create_test_entitlement(
        &self,
        application_id: Id<ApplicationMarker>,
        sku_id: Id<SkuMarker>,
        owner_id: Id<cordex_model::id::marker::GenericMarker>,
        owner_type: u8,
    ) -> impl Future<Output = Result<Entitlement, HttpError>> + Send + 'static {
        let client = self.clone();
        let body =
            serde_json::json!({ "sku_id": sku_id, "owner_id": owner_id, "owner_type": owner_type });
        async move {
            if !matches!(owner_type, 1 | 2) {
                return Err(HttpError::Validation(
                    "owner_type is 1 (guild) or 2 (user)".into(),
                ));
            }
            client
                .post(Route::raw(
                    "/applications/{application_id}/entitlements",
                    application_id,
                ))
                .json(&body)?
                .send_json()
                .await
        }
    }

    simple! {
        /// `DELETE /applications/{application_id}/entitlements/{entitlement_id}`:
        /// remove a test entitlement.
        fn delete_test_entitlement(application_id: Id<ApplicationMarker>, entitlement_id: Id<EntitlementMarker>)
            -> () = delete "/applications/{application_id}/entitlements/{entitlement_id}" => send_empty
    }

    simple! {
        /// `GET /applications/{application_id}/skus`.
        fn get_skus(application_id: Id<ApplicationMarker>)
            -> Vec<Sku> = get "/applications/{application_id}/skus" => send_json
    }

    /// `GET /skus/{sku_id}/subscriptions`.
    pub fn get_sku_subscriptions(&self, sku_id: Id<SkuMarker>) -> GetSkuSubscriptions {
        GetSkuSubscriptions::new(self, Route::raw("/skus/{sku_id}/subscriptions", sku_id))
    }

    simple! {
        /// `GET /skus/{sku_id}/subscriptions/{subscription_id}`.
        fn get_sku_subscription(sku_id: Id<SkuMarker>, subscription_id: Id<SubscriptionMarker>)
            -> Subscription = get "/skus/{sku_id}/subscriptions/{subscription_id}" => send_json
    }

    // ---- current application -----------------------------------------------

    simple! {
        /// `GET /applications/@me`.
        fn get_current_application() -> Application = get "/applications/@me" => send_json
    }

    /// `PATCH /applications/@me`.
    pub fn update_current_application(&self) -> UpdateCurrentApplication {
        UpdateCurrentApplication::new(self, Method::PATCH, Route::raw("/applications/@me", ()))
    }

    simple! {
        /// `GET /applications/{application_id}/activity-instances/{instance_id}`.
        fn get_activity_instance(application_id: Id<ApplicationMarker>, instance_id: String)
            -> ActivityInstance = get "/applications/{application_id}/activity-instances/{instance_id}" => send_json
    }

    simple! {
        /// `GET /applications/{application_id}/role-connections/metadata`.
        fn get_role_connection_metadata(application_id: Id<ApplicationMarker>)
            -> Vec<RoleConnectionMetadata> = get "/applications/{application_id}/role-connections/metadata" => send_json
    }

    /// `PUT /applications/{application_id}/role-connections/metadata`:
    /// replace the linked-role metadata (max. 5 records).
    pub fn update_role_connection_metadata(
        &self,
        application_id: Id<ApplicationMarker>,
        records: Vec<RoleConnectionMetadata>,
    ) -> impl Future<Output = Result<Vec<RoleConnectionMetadata>, HttpError>> + Send + 'static {
        let client = self.clone();
        async move {
            if records.len() > 5 {
                return Err(HttpError::Validation(
                    "at most 5 role connection metadata records".into(),
                ));
            }
            client
                .put(Route::raw(
                    "/applications/{application_id}/role-connections/metadata",
                    application_id,
                ))
                .json(&records)?
                .send_json()
                .await
        }
    }

    // ---- polls -------------------------------------------------------------

    /// `GET /channels/{channel_id}/polls/{message_id}/answers/{answer_id}`.
    pub fn get_answer_voters(
        &self,
        channel_id: Id<ChannelMarker>,
        message_id: Id<MessageMarker>,
        answer_id: u32,
    ) -> GetAnswerVoters {
        GetAnswerVoters::new(
            self,
            Route::raw(
                "/channels/{channel_id}/polls/{message_id}/answers/{answer_id}",
                (channel_id, message_id, answer_id),
            ),
        )
    }

    simple! {
        /// `POST /channels/{channel_id}/polls/{message_id}/expire`: end a poll
        /// the bot created.
        fn end_poll(channel_id: Id<ChannelMarker>, message_id: Id<MessageMarker>)
            -> Message = post "/channels/{channel_id}/polls/{message_id}/expire" => send_json
    }
}
