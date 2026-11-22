//! Webhooks (resources/webhook.mdx).

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::id::Id;
use crate::id::marker::{ApplicationMarker, ChannelMarker, GuildMarker, WebhookMarker};
use crate::image_hash::ImageHash;
use crate::serde_util::int_enum;
use crate::token::WebhookToken;
use crate::user::User;

int_enum! {
    /// Kind of webhook.
    pub enum WebhookType: u8 {
        /// Posts messages with a generated token.
        Incoming = 1,
        /// Channel-following webhook.
        ChannelFollower = 2,
        /// Application / interaction webhook.
        Application = 3,
    }
}

/// A webhook. `Debug` redacts [`Webhook::token`] and [`Webhook::url`].
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Webhook {
    /// Webhook ID.
    pub id: Id<WebhookMarker>,
    /// Webhook type.
    #[serde(rename = "type")]
    pub kind: WebhookType,
    /// Guild; `None` for some application webhooks.
    pub guild_id: Option<Id<GuildMarker>>,
    /// Channel; `None` for some application webhooks.
    pub channel_id: Option<Id<ChannelMarker>>,
    /// Creator; omitted when fetched with the webhook token.
    pub user: Option<User>,
    /// Default name.
    pub name: Option<Box<str>>,
    /// Default avatar hash.
    pub avatar: Option<ImageHash>,
    /// Incoming-webhook token.
    pub token: Option<WebhookToken>,
    /// Application that created the webhook.
    pub application_id: Option<Id<ApplicationMarker>>,
    /// Followed guild (channel-follower webhooks).
    pub source_guild: Option<WebhookSourceGuild>,
    /// Followed channel (channel-follower webhooks).
    pub source_channel: Option<WebhookSourceChannel>,
    /// Execute URL from the webhooks `OAuth2` flow (contains the token).
    pub url: Option<Box<str>>,
}

impl fmt::Debug for Webhook {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Webhook")
            .field("id", &self.id)
            .field("kind", &self.kind)
            .field("guild_id", &self.guild_id)
            .field("channel_id", &self.channel_id)
            .field("user", &self.user)
            .field("name", &self.name)
            .field("avatar", &self.avatar)
            .field("token", &self.token.as_ref().map(|_| "***"))
            .field("application_id", &self.application_id)
            .field("source_guild", &self.source_guild)
            .field("source_channel", &self.source_channel)
            .field("url", &self.url.as_ref().map(|_| "***"))
            .finish()
    }
}

/// Partial guild on a channel-follower webhook (`id`, `name`, `icon`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct WebhookSourceGuild {
    /// Guild ID.
    pub id: Id<GuildMarker>,
    /// Guild name.
    #[serde(default)]
    pub name: Box<str>,
    /// Icon hash.
    pub icon: Option<ImageHash>,
}

/// Partial channel on a channel-follower webhook (`id`, `name`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct WebhookSourceChannel {
    /// Channel ID.
    pub id: Id<ChannelMarker>,
    /// Channel name.
    pub name: Option<Box<str>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_never_prints() {
        let json = r#"{
            "id":"1","type":1,"channel_id":"2","guild_id":"2",
            "name":"hook","avatar":null,"application_id":null,
            "token":"super-secret-webhook-token"
        }"#;
        let hook: Webhook = serde_json::from_str(json).unwrap();
        assert_eq!(
            hook.token.as_ref().unwrap().expose(),
            "super-secret-webhook-token"
        );
        let debug = format!("{hook:?}");
        assert!(!debug.contains("super-secret"));
        assert!(debug.contains("***"));
        let out = serde_json::to_string(&hook).unwrap();
        assert!(out.contains("super-secret-webhook-token"));
    }
}
