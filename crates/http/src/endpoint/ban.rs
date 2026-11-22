//! Guild bans.

use std::future::{Future, IntoFuture};
use std::pin::Pin;

use cordex_model::guild::Ban;
use cordex_model::id::Id;
use cordex_model::id::marker::{GuildMarker, UserMarker};
use serde::Serialize;

use crate::client::Client;
use crate::error::HttpError;
use crate::route::Route;

impl Client {
    /// `GET /guilds/{guild_id}/bans/{user_id}`.
    pub fn get_guild_ban(
        &self,
        guild_id: Id<GuildMarker>,
        user_id: Id<UserMarker>,
    ) -> impl Future<Output = Result<Ban, HttpError>> + Send + 'static {
        let client = self.clone();
        async move {
            client
                .get(Route::raw(
                    "/guilds/{guild_id}/bans/{user_id}",
                    (guild_id, user_id),
                ))
                .send_json()
                .await
        }
    }

    /// `GET /guilds/{guild_id}/bans`.
    pub fn get_guild_bans(&self, guild_id: Id<GuildMarker>) -> GetGuildBans {
        GetGuildBans {
            client: self.clone(),
            guild_id,
            limit: None,
            before: None,
            after: None,
        }
    }

    /// `PUT /guilds/{guild_id}/bans/{user_id}`.
    pub fn create_guild_ban(
        &self,
        guild_id: Id<GuildMarker>,
        user_id: Id<UserMarker>,
    ) -> CreateGuildBan {
        CreateGuildBan {
            client: self.clone(),
            guild_id,
            user_id,
            delete_message_seconds: None,
            reason: None,
        }
    }

    /// `DELETE /guilds/{guild_id}/bans/{user_id}`.
    pub fn remove_guild_ban(
        &self,
        guild_id: Id<GuildMarker>,
        user_id: Id<UserMarker>,
    ) -> RemoveGuildBan {
        RemoveGuildBan {
            client: self.clone(),
            guild_id,
            user_id,
            reason: None,
        }
    }
}

/// `GET /guilds/{guild_id}/bans`.
pub struct GetGuildBans {
    client: Client,
    guild_id: Id<GuildMarker>,
    limit: Option<u16>,
    before: Option<Id<UserMarker>>,
    after: Option<Id<UserMarker>>,
}

impl GetGuildBans {
    /// Page size (1–1000, Discord default 1000).
    #[must_use]
    pub fn limit(mut self, limit: u16) -> Self {
        self.limit = Some(limit.clamp(1, 1000));
        self
    }

    /// Bans before this user ID.
    #[must_use]
    pub fn before(mut self, user_id: Id<UserMarker>) -> Self {
        self.before = Some(user_id);
        self
    }

    /// Bans after this user ID.
    #[must_use]
    pub fn after(mut self, user_id: Id<UserMarker>) -> Self {
        self.after = Some(user_id);
        self
    }

    /// Send the request.
    pub async fn send(self) -> Result<Vec<Ban>, HttpError> {
        let mut req = self
            .client
            .get(Route::raw("/guilds/{guild_id}/bans", self.guild_id));
        if let Some(limit) = self.limit {
            req = req.query("limit", limit);
        }
        if let Some(id) = self.before {
            req = req.query("before", id);
        }
        if let Some(id) = self.after {
            req = req.query("after", id);
        }
        req.send_json().await
    }
}

impl IntoFuture for GetGuildBans {
    type Output = Result<Vec<Ban>, HttpError>;
    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output> + Send>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(self.send())
    }
}

#[derive(Serialize)]
struct CreateBanBody {
    #[serde(skip_serializing_if = "Option::is_none")]
    delete_message_seconds: Option<u32>,
}

/// `PUT /guilds/{guild_id}/bans/{user_id}`.
pub struct CreateGuildBan {
    client: Client,
    guild_id: Id<GuildMarker>,
    user_id: Id<UserMarker>,
    delete_message_seconds: Option<u32>,
    reason: Option<String>,
}

impl CreateGuildBan {
    /// Delete the user's messages from the last `seconds` (0–604800).
    pub fn delete_message_seconds(mut self, seconds: u32) -> Result<Self, HttpError> {
        if seconds > 604_800 {
            return Err(HttpError::validation(
                "delete_message_seconds must be at most 604800 (7 days)",
            ));
        }
        self.delete_message_seconds = Some(seconds);
        Ok(self)
    }

    /// Audit-log reason.
    pub fn reason(mut self, reason: impl Into<String>) -> Result<Self, HttpError> {
        super::set_reason(&mut self.reason, reason)?;
        Ok(self)
    }

    /// Send the request.
    pub async fn send(self) -> Result<(), HttpError> {
        self.client
            .put(Route::raw(
                "/guilds/{guild_id}/bans/{user_id}",
                (self.guild_id, self.user_id),
            ))
            .maybe_reason(self.reason.as_deref())?
            .json(&CreateBanBody {
                delete_message_seconds: self.delete_message_seconds,
            })?
            .send_empty()
            .await
    }
}

impl IntoFuture for CreateGuildBan {
    type Output = Result<(), HttpError>;
    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output> + Send>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(self.send())
    }
}

/// `DELETE /guilds/{guild_id}/bans/{user_id}`.
pub struct RemoveGuildBan {
    client: Client,
    guild_id: Id<GuildMarker>,
    user_id: Id<UserMarker>,
    reason: Option<String>,
}

impl RemoveGuildBan {
    /// Audit-log reason.
    pub fn reason(mut self, reason: impl Into<String>) -> Result<Self, HttpError> {
        super::set_reason(&mut self.reason, reason)?;
        Ok(self)
    }

    /// Send the request.
    pub async fn send(self) -> Result<(), HttpError> {
        self.client
            .delete(Route::raw(
                "/guilds/{guild_id}/bans/{user_id}",
                (self.guild_id, self.user_id),
            ))
            .maybe_reason(self.reason.as_deref())?
            .send_empty()
            .await
    }
}

impl IntoFuture for RemoveGuildBan {
    type Output = Result<(), HttpError>;
    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output> + Send>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(self.send())
    }
}
