//! Guild scheduled events.

use std::future::{Future, IntoFuture};
use std::pin::Pin;

use cordex_model::id::Id;
use cordex_model::id::marker::{ChannelMarker, GuildMarker, ScheduledEventMarker};
use cordex_model::scheduled_event::{
    ScheduledEvent, ScheduledEventEntityMetadata, ScheduledEventEntityType,
    ScheduledEventPrivacyLevel, ScheduledEventStatus,
};
use cordex_model::timestamp::Timestamp;
use serde::Serialize;

use crate::client::Client;
use crate::error::HttpError;
use crate::route::Route;

impl Client {
    /// `POST /guilds/{guild_id}/scheduled-events`.
    pub fn create_scheduled_event(
        &self,
        guild_id: Id<GuildMarker>,
        name: impl Into<String>,
    ) -> CreateScheduledEvent {
        CreateScheduledEvent {
            client: self.clone(),
            guild_id,
            body: ScheduledEventBody {
                name: Some(name.into()),
                privacy_level: Some(ScheduledEventPrivacyLevel::GuildOnly),
                ..ScheduledEventBody::default()
            },
            reason: None,
        }
    }

    /// `PATCH /guilds/{guild_id}/scheduled-events/{event_id}`.
    pub fn update_scheduled_event(
        &self,
        guild_id: Id<GuildMarker>,
        event_id: Id<ScheduledEventMarker>,
    ) -> UpdateScheduledEvent {
        UpdateScheduledEvent {
            client: self.clone(),
            guild_id,
            event_id,
            body: ScheduledEventBody::default(),
            reason: None,
        }
    }

    /// `DELETE /guilds/{guild_id}/scheduled-events/{event_id}`.
    pub fn delete_scheduled_event(
        &self,
        guild_id: Id<GuildMarker>,
        event_id: Id<ScheduledEventMarker>,
    ) -> impl Future<Output = Result<(), HttpError>> + Send + 'static {
        let client = self.clone();
        async move {
            client
                .delete(Route::raw(
                    "/guilds/{guild_id}/scheduled-events/{guild_scheduled_event_id}",
                    (guild_id, event_id),
                ))
                .send_empty()
                .await
        }
    }
}

#[derive(Serialize, Default)]
struct ScheduledEventBody {
    #[serde(skip_serializing_if = "Option::is_none")]
    channel_id: Option<Id<ChannelMarker>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    entity_metadata: Option<ScheduledEventEntityMetadata>,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    privacy_level: Option<ScheduledEventPrivacyLevel>,
    #[serde(skip_serializing_if = "Option::is_none")]
    scheduled_start_time: Option<Timestamp>,
    #[serde(skip_serializing_if = "Option::is_none")]
    scheduled_end_time: Option<Timestamp>,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    entity_type: Option<ScheduledEventEntityType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    status: Option<ScheduledEventStatus>,
}

macro_rules! scheduled_event_setters {
    ($ty:ty) => {
        impl $ty {
            /// Event name (1–100 characters).
            pub fn name(mut self, name: impl Into<String>) -> Result<Self, HttpError> {
                let name = name.into();
                let n = name.chars().count();
                if !(1..=100).contains(&n) {
                    return Err(HttpError::validation(
                        format!("scheduled event name is {n} characters; must be 1–100")
                            .into_boxed_str(),
                    ));
                }
                self.body.name = Some(name);
                Ok(self)
            }

            /// Description (1–1000 characters).
            pub fn description(
                mut self,
                description: impl Into<String>,
            ) -> Result<Self, HttpError> {
                let description = description.into();
                let n = description.chars().count();
                if !(1..=1000).contains(&n) {
                    return Err(HttpError::validation(
                        format!("scheduled event description is {n} characters; must be 1–1000")
                            .into_boxed_str(),
                    ));
                }
                self.body.description = Some(description);
                Ok(self)
            }

            /// Host channel (stage or voice).
            #[must_use]
            pub fn channel_id(mut self, channel_id: Id<ChannelMarker>) -> Self {
                self.body.channel_id = Some(channel_id);
                self
            }

            /// External location (sets entity metadata).
            #[must_use]
            pub fn location(mut self, location: impl Into<String>) -> Self {
                self.body.entity_metadata =
                    Some(ScheduledEventEntityMetadata::location(location.into()));
                self
            }

            /// Where the event takes place.
            #[must_use]
            pub fn entity_type(mut self, entity_type: ScheduledEventEntityType) -> Self {
                self.body.entity_type = Some(entity_type);
                self
            }

            /// Start time.
            #[must_use]
            pub fn scheduled_start_time(mut self, time: Timestamp) -> Self {
                self.body.scheduled_start_time = Some(time);
                self
            }

            /// End time (required for external events).
            #[must_use]
            pub fn scheduled_end_time(mut self, time: Timestamp) -> Self {
                self.body.scheduled_end_time = Some(time);
                self
            }

            /// Audit-log reason.
            pub fn reason(mut self, reason: impl Into<String>) -> Result<Self, HttpError> {
                super::set_reason(&mut self.reason, reason)?;
                Ok(self)
            }
        }
    };
}

/// `POST /guilds/{guild_id}/scheduled-events`.
pub struct CreateScheduledEvent {
    client: Client,
    guild_id: Id<GuildMarker>,
    body: ScheduledEventBody,
    reason: Option<String>,
}

scheduled_event_setters!(CreateScheduledEvent);

impl CreateScheduledEvent {
    /// Send the request.
    pub async fn send(self) -> Result<ScheduledEvent, HttpError> {
        if self.body.scheduled_start_time.is_none() {
            return Err(HttpError::validation("scheduled event needs a start time"));
        }
        if self.body.entity_type.is_none() {
            return Err(HttpError::validation(
                "scheduled event needs an entity type",
            ));
        }
        self.client
            .post(Route::raw(
                "/guilds/{guild_id}/scheduled-events",
                self.guild_id,
            ))
            .maybe_reason(self.reason.as_deref())?
            .json(&self.body)?
            .send_json()
            .await
    }
}

impl IntoFuture for CreateScheduledEvent {
    type Output = Result<ScheduledEvent, HttpError>;
    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output> + Send>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(self.send())
    }
}

/// `PATCH /guilds/{guild_id}/scheduled-events/{event_id}`.
pub struct UpdateScheduledEvent {
    client: Client,
    guild_id: Id<GuildMarker>,
    event_id: Id<ScheduledEventMarker>,
    body: ScheduledEventBody,
    reason: Option<String>,
}

scheduled_event_setters!(UpdateScheduledEvent);

impl UpdateScheduledEvent {
    /// Lifecycle status (e.g. cancel).
    #[must_use]
    pub fn status(mut self, status: ScheduledEventStatus) -> Self {
        self.body.status = Some(status);
        self
    }

    /// Send the request.
    pub async fn send(self) -> Result<ScheduledEvent, HttpError> {
        self.client
            .patch(Route::raw(
                "/guilds/{guild_id}/scheduled-events/{guild_scheduled_event_id}",
                (self.guild_id, self.event_id),
            ))
            .maybe_reason(self.reason.as_deref())?
            .json(&self.body)?
            .send_json()
            .await
    }
}

impl IntoFuture for UpdateScheduledEvent {
    type Output = Result<ScheduledEvent, HttpError>;
    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output> + Send>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(self.send())
    }
}
