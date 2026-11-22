//! Guild integrations (resources/guild.mdx, "Integration Object").

use serde::{Deserialize, Serialize};

use crate::application::Application;
use crate::id::Id;
use crate::id::marker::{ApplicationMarker, GuildMarker, IntegrationMarker, RoleMarker};
use crate::serde_util::{int_enum, str_enum};
use crate::timestamp::Timestamp;
use crate::user::User;

str_enum! {
    /// Integration service.
    pub enum IntegrationType {
        /// Twitch.
        Twitch = "twitch",
        /// `YouTube`.
        Youtube = "youtube",
        /// Discord bot.
        Discord = "discord",
        /// Guild subscription (role subscriptions).
        GuildSubscription = "guild_subscription",
    }
}

int_enum! {
    /// What happens when a subscriber expires.
    pub enum IntegrationExpireBehavior: u8 {
        /// Remove the subscriber role.
        RemoveRole = 0,
        /// Kick the member.
        Kick = 1,
    }
}

/// A guild integration. Partial payloads (audit log) omit most fields.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Integration {
    /// Integration ID.
    pub id: Id<IntegrationMarker>,
    /// Name.
    #[serde(default)]
    pub name: Box<str>,
    /// Service type.
    #[serde(rename = "type")]
    pub kind: IntegrationType,
    /// Whether the integration is enabled. Defaulted: audit-log partials omit it.
    #[serde(default)]
    pub enabled: bool,
    /// Whether the integration is syncing (not sent for Discord bots).
    pub syncing: Option<bool>,
    /// Subscriber role (not sent for Discord bots).
    pub role_id: Option<Id<RoleMarker>>,
    /// Whether Twitch emoticons are synced.
    pub enable_emoticons: Option<bool>,
    /// Expire behavior.
    pub expire_behavior: Option<IntegrationExpireBehavior>,
    /// Grace period in days.
    pub expire_grace_period: Option<u32>,
    /// Linked user (omitted on some older integrations and gateway events).
    pub user: Option<User>,
    /// Linked account.
    pub account: Option<IntegrationAccount>,
    /// Last sync.
    pub synced_at: Option<Timestamp>,
    /// Subscriber count.
    pub subscriber_count: Option<u32>,
    /// Whether the integration was revoked.
    pub revoked: Option<bool>,
    /// Bot application (Discord integrations).
    pub application: Option<Box<Application>>,
    /// Authorized `OAuth2` scopes.
    #[serde(default)]
    pub scopes: Vec<Box<str>>,
    /// Guild (gateway `INTEGRATION_CREATE` / `UPDATE`).
    pub guild_id: Option<Id<GuildMarker>>,
    /// Application ID on some partials (audit log example).
    pub application_id: Option<Id<ApplicationMarker>>,
}

/// Account linked to an integration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct IntegrationAccount {
    /// Account ID on the service.
    pub id: Box<str>,
    /// Account name.
    pub name: Box<str>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audit_log_partial() {
        let json = r#"{
            "id":"33590653072239123","name":"A Name","type":"twitch",
            "account":{"name":"twitchusername","id":"1234567"},
            "application_id":"94651234501213162"
        }"#;
        let integration: Integration = serde_json::from_str(json).unwrap();
        assert_eq!(integration.kind, IntegrationType::Twitch);
        assert_eq!(
            integration.account.as_ref().unwrap().name.as_ref(),
            "twitchusername"
        );
        let out = serde_json::to_string(&integration).unwrap();
        let again: Integration = serde_json::from_str(&out).unwrap();
        assert_eq!(integration, again);
    }
}
