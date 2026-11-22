//! Guild member mutators.

use std::future::{Future, IntoFuture};
use std::pin::Pin;

use cordex_model::guild::Member;
use cordex_model::id::Id;
use cordex_model::id::marker::{ChannelMarker, GuildMarker, RoleMarker, UserMarker};
use cordex_model::timestamp::Timestamp;
use serde::Serialize;

use crate::client::Client;
use crate::error::HttpError;
use crate::route::Route;

impl Client {
    /// `PATCH /guilds/{guild_id}/members/{user_id}`.
    pub fn update_guild_member(
        &self,
        guild_id: Id<GuildMarker>,
        user_id: Id<UserMarker>,
    ) -> UpdateGuildMember {
        UpdateGuildMember {
            client: self.clone(),
            guild_id,
            user_id,
            body: UpdateMemberBody::default(),
            reason: None,
        }
    }

    /// `PUT /guilds/{guild_id}/members/{user_id}/roles/{role_id}`.
    pub fn add_guild_member_role(
        &self,
        guild_id: Id<GuildMarker>,
        user_id: Id<UserMarker>,
        role_id: Id<RoleMarker>,
    ) -> AddGuildMemberRole {
        AddGuildMemberRole {
            client: self.clone(),
            guild_id,
            user_id,
            role_id,
            reason: None,
        }
    }

    /// `DELETE /guilds/{guild_id}/members/{user_id}/roles/{role_id}`.
    pub fn remove_guild_member_role(
        &self,
        guild_id: Id<GuildMarker>,
        user_id: Id<UserMarker>,
        role_id: Id<RoleMarker>,
    ) -> RemoveGuildMemberRole {
        RemoveGuildMemberRole {
            client: self.clone(),
            guild_id,
            user_id,
            role_id,
            reason: None,
        }
    }

    /// `DELETE /guilds/{guild_id}/members/{user_id}`.
    pub fn remove_guild_member(
        &self,
        guild_id: Id<GuildMarker>,
        user_id: Id<UserMarker>,
    ) -> RemoveGuildMember {
        RemoveGuildMember {
            client: self.clone(),
            guild_id,
            user_id,
            reason: None,
        }
    }
}

#[derive(Serialize, Default)]
#[allow(clippy::option_option)] // Discord: omit vs JSON null (clear nick/voice/timeout).
struct UpdateMemberBody {
    #[serde(skip_serializing_if = "Option::is_none")]
    nick: Option<Option<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    roles: Option<Vec<Id<RoleMarker>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mute: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    deaf: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    channel_id: Option<Option<Id<ChannelMarker>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    communication_disabled_until: Option<Option<Timestamp>>,
}

/// `PATCH /guilds/{guild_id}/members/{user_id}`.
pub struct UpdateGuildMember {
    client: Client,
    guild_id: Id<GuildMarker>,
    user_id: Id<UserMarker>,
    body: UpdateMemberBody,
    reason: Option<String>,
}

impl UpdateGuildMember {
    /// Guild nickname.
    #[must_use]
    pub fn nick(mut self, nick: impl Into<String>) -> Self {
        self.body.nick = Some(Some(nick.into()));
        self
    }

    /// Clear the nickname.
    #[must_use]
    pub fn clear_nick(mut self) -> Self {
        self.body.nick = Some(None);
        self
    }

    /// Replace the member's roles.
    #[must_use]
    pub fn roles(mut self, roles: Vec<Id<RoleMarker>>) -> Self {
        self.body.roles = Some(roles);
        self
    }

    /// Server mute.
    #[must_use]
    pub fn mute(mut self, mute: bool) -> Self {
        self.body.mute = Some(mute);
        self
    }

    /// Server deafen.
    #[must_use]
    pub fn deaf(mut self, deaf: bool) -> Self {
        self.body.deaf = Some(deaf);
        self
    }

    /// Move the member into a voice channel.
    #[must_use]
    pub fn channel_id(mut self, channel_id: Id<ChannelMarker>) -> Self {
        self.body.channel_id = Some(Some(channel_id));
        self
    }

    /// Disconnect the member from voice.
    #[must_use]
    pub fn disconnect(mut self) -> Self {
        self.body.channel_id = Some(None);
        self
    }

    /// Timeout until `until`.
    #[must_use]
    pub fn communication_disabled_until(mut self, until: Timestamp) -> Self {
        self.body.communication_disabled_until = Some(Some(until));
        self
    }

    /// Clear a timeout.
    #[must_use]
    pub fn clear_timeout(mut self) -> Self {
        self.body.communication_disabled_until = Some(None);
        self
    }

    /// Audit-log reason.
    pub fn reason(mut self, reason: impl Into<String>) -> Result<Self, HttpError> {
        super::set_reason(&mut self.reason, reason)?;
        Ok(self)
    }

    /// Send the request.
    pub async fn send(self) -> Result<Member, HttpError> {
        self.client
            .patch(Route::raw(
                "/guilds/{guild_id}/members/{user_id}",
                (self.guild_id, self.user_id),
            ))
            .maybe_reason(self.reason.as_deref())?
            .json(&self.body)?
            .send_json()
            .await
    }
}

impl IntoFuture for UpdateGuildMember {
    type Output = Result<Member, HttpError>;
    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output> + Send>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(self.send())
    }
}

/// `PUT /guilds/{guild_id}/members/{user_id}/roles/{role_id}`.
pub struct AddGuildMemberRole {
    client: Client,
    guild_id: Id<GuildMarker>,
    user_id: Id<UserMarker>,
    role_id: Id<RoleMarker>,
    reason: Option<String>,
}

impl AddGuildMemberRole {
    /// Audit-log reason.
    pub fn reason(mut self, reason: impl Into<String>) -> Result<Self, HttpError> {
        super::set_reason(&mut self.reason, reason)?;
        Ok(self)
    }

    /// Send the request.
    pub async fn send(self) -> Result<(), HttpError> {
        self.client
            .put(Route::raw(
                "/guilds/{guild_id}/members/{user_id}/roles/{role_id}",
                (self.guild_id, self.user_id, self.role_id),
            ))
            .maybe_reason(self.reason.as_deref())?
            .send_empty()
            .await
    }
}

impl IntoFuture for AddGuildMemberRole {
    type Output = Result<(), HttpError>;
    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output> + Send>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(self.send())
    }
}

/// `DELETE /guilds/{guild_id}/members/{user_id}/roles/{role_id}`.
pub struct RemoveGuildMemberRole {
    client: Client,
    guild_id: Id<GuildMarker>,
    user_id: Id<UserMarker>,
    role_id: Id<RoleMarker>,
    reason: Option<String>,
}

impl RemoveGuildMemberRole {
    /// Audit-log reason.
    pub fn reason(mut self, reason: impl Into<String>) -> Result<Self, HttpError> {
        super::set_reason(&mut self.reason, reason)?;
        Ok(self)
    }

    /// Send the request.
    pub async fn send(self) -> Result<(), HttpError> {
        self.client
            .delete(Route::raw(
                "/guilds/{guild_id}/members/{user_id}/roles/{role_id}",
                (self.guild_id, self.user_id, self.role_id),
            ))
            .maybe_reason(self.reason.as_deref())?
            .send_empty()
            .await
    }
}

impl IntoFuture for RemoveGuildMemberRole {
    type Output = Result<(), HttpError>;
    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output> + Send>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(self.send())
    }
}

/// Kick a member.
pub struct RemoveGuildMember {
    client: Client,
    guild_id: Id<GuildMarker>,
    user_id: Id<UserMarker>,
    reason: Option<String>,
}

impl RemoveGuildMember {
    /// Audit-log reason.
    pub fn reason(mut self, reason: impl Into<String>) -> Result<Self, HttpError> {
        super::set_reason(&mut self.reason, reason)?;
        Ok(self)
    }

    /// Send the request.
    pub async fn send(self) -> Result<(), HttpError> {
        self.client
            .delete(Route::raw(
                "/guilds/{guild_id}/members/{user_id}",
                (self.guild_id, self.user_id),
            ))
            .maybe_reason(self.reason.as_deref())?
            .send_empty()
            .await
    }
}

impl IntoFuture for RemoveGuildMember {
    type Output = Result<(), HttpError>;
    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output> + Send>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(self.send())
    }
}
