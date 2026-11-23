//! Webhook events: what Discord POSTs to an app's event webhook URL
//! (events/webhook-events.mdx). Verify the request with the Ed25519
//! signature first (`cordex-interactions` feature `verify`).

use serde::{Deserialize, Serialize};

use crate::entitlement::Entitlement;
use crate::guild::Guild;
use crate::id::Id;
use crate::id::marker::ApplicationMarker;
use crate::lobby::{LobbyMessage, LobbyMessageDelete, SdkDmMessage};
use crate::serde_util::int_enum;
use crate::timestamp::Timestamp;
use crate::user::User;

/// Body of a webhook event request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct WebhookEventPayload {
    /// Version scheme (currently 1).
    pub version: u8,
    /// Your application.
    pub application_id: Id<ApplicationMarker>,
    /// `Ping` (answer with 204) or `Event`.
    #[serde(rename = "type")]
    pub kind: WebhookEventType,
    /// The event (only for `Event`).
    #[serde(default)]
    pub event: Option<WebhookEventBody>,
}

int_enum! {
    /// Webhook event request type.
    pub enum WebhookEventType: u8 {
        /// Discord checks that the URL works; answer with 204.
        Ping = 0,
        /// An event; see [`WebhookEventPayload::event`].
        Event = 1,
    }
}

/// The event inside a webhook event request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct WebhookEventBody {
    /// Event name (`APPLICATION_AUTHORIZED`, `ENTITLEMENT_CREATE`, …).
    #[serde(rename = "type")]
    pub kind: Box<str>,
    /// When the event happened.
    pub timestamp: Timestamp,
    /// Event data; read it typed with [`WebhookEventBody::parse`].
    #[serde(default)]
    pub data: Option<serde_json::Value>,
}

/// A webhook event with typed data.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum WebhookEvent {
    /// The app was installed to a user or guild.
    ApplicationAuthorized(Box<ApplicationAuthorized>),
    /// A user removed the app.
    ApplicationDeauthorized(Box<ApplicationDeauthorized>),
    /// An entitlement was created.
    EntitlementCreate(Box<Entitlement>),
    /// An entitlement was updated.
    EntitlementUpdate(Box<Entitlement>),
    /// An entitlement was deleted.
    EntitlementDelete(Box<Entitlement>),
    /// A lobby message was created.
    LobbyMessageCreate(Box<LobbyMessage>),
    /// A lobby message was updated.
    LobbyMessageUpdate(Box<LobbyMessage>),
    /// A lobby message was deleted.
    LobbyMessageDelete(Box<LobbyMessageDelete>),
    /// A Social SDK DM was created.
    GameDirectMessageCreate(Box<SdkDmMessage>),
    /// A Social SDK DM was updated.
    GameDirectMessageUpdate(Box<SdkDmMessage>),
    /// A Social SDK DM was deleted.
    GameDirectMessageDelete(Box<SdkDmMessage>),
    /// Another event (quests, …); see the raw body.
    Other,
}

impl WebhookEventBody {
    /// Parse `data` by event name; unknown names give [`WebhookEvent::Other`].
    pub fn parse(&self) -> serde_json::Result<WebhookEvent> {
        fn typed<T: serde::de::DeserializeOwned>(
            data: Option<&serde_json::Value>,
        ) -> serde_json::Result<Box<T>> {
            T::deserialize(data.unwrap_or(&serde_json::Value::Null)).map(Box::new)
        }
        let data = self.data.as_ref();
        Ok(match &*self.kind {
            "APPLICATION_AUTHORIZED" => WebhookEvent::ApplicationAuthorized(typed(data)?),
            "APPLICATION_DEAUTHORIZED" => WebhookEvent::ApplicationDeauthorized(typed(data)?),
            "ENTITLEMENT_CREATE" => WebhookEvent::EntitlementCreate(typed(data)?),
            "ENTITLEMENT_UPDATE" => WebhookEvent::EntitlementUpdate(typed(data)?),
            "ENTITLEMENT_DELETE" => WebhookEvent::EntitlementDelete(typed(data)?),
            "LOBBY_MESSAGE_CREATE" => WebhookEvent::LobbyMessageCreate(typed(data)?),
            "LOBBY_MESSAGE_UPDATE" => WebhookEvent::LobbyMessageUpdate(typed(data)?),
            "LOBBY_MESSAGE_DELETE" => WebhookEvent::LobbyMessageDelete(typed(data)?),
            "GAME_DIRECT_MESSAGE_CREATE" => WebhookEvent::GameDirectMessageCreate(typed(data)?),
            "GAME_DIRECT_MESSAGE_UPDATE" => WebhookEvent::GameDirectMessageUpdate(typed(data)?),
            "GAME_DIRECT_MESSAGE_DELETE" => WebhookEvent::GameDirectMessageDelete(typed(data)?),
            _ => WebhookEvent::Other,
        })
    }
}

/// `APPLICATION_AUTHORIZED` data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ApplicationAuthorized {
    /// Installation context (0 guild, 1 user).
    #[serde(default)]
    pub integration_type: Option<crate::application::ApplicationIntegrationType>,
    /// User who installed the app.
    pub user: User,
    /// Granted `OAuth2` scopes.
    #[serde(default)]
    pub scopes: Vec<Box<str>>,
    /// Guild the app was installed to (guild installs).
    #[serde(default)]
    pub guild: Option<Box<Guild>>,
}

/// `APPLICATION_DEAUTHORIZED` data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ApplicationDeauthorized {
    /// User who removed the app.
    pub user: User,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ping_and_authorized_event_parse() {
        let ping: WebhookEventPayload =
            serde_json::from_str(r#"{"version":1,"application_id":"1","type":0}"#).unwrap();
        assert_eq!(ping.kind, WebhookEventType::Ping);
        assert!(ping.event.is_none());

        let json = r#"{"version":1,"application_id":"1","type":1,"event":{
            "type":"APPLICATION_AUTHORIZED","timestamp":"2026-11-23T10:00:00+00:00",
            "data":{"integration_type":1,"user":{"id":"2","username":"a","discriminator":"0","global_name":null,"avatar":null},
            "scopes":["applications.commands"]}}}"#;
        let payload: WebhookEventPayload = serde_json::from_str(json).unwrap();
        let event = payload.event.unwrap().parse().unwrap();
        let WebhookEvent::ApplicationAuthorized(auth) = event else {
            panic!("wrong event");
        };
        assert_eq!(auth.scopes.len(), 1);
        assert!(auth.guild.is_none());
    }
}
