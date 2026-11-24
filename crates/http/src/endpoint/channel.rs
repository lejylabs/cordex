//! Channel endpoints.

use std::future::{Future, IntoFuture};
use std::pin::Pin;

use cordex_model::channel::{
    AutoArchiveDuration, Channel, ChannelType, OverwriteType, PermissionOverwrite,
};
use cordex_model::id::Id;
use cordex_model::id::marker::{ChannelMarker, GenericMarker, GuildMarker};
use cordex_model::permissions::Permissions;
use serde::Serialize;

use crate::client::Client;
use crate::error::HttpError;
use crate::route::Route;

impl Client {
    /// `GET /channels/{channel_id}`.
    pub fn get_channel(
        &self,
        channel_id: Id<ChannelMarker>,
    ) -> impl Future<Output = Result<Channel, HttpError>> + Send + 'static {
        let client = self.clone();
        async move {
            client
                .get(Route::raw("/channels/{channel_id}", channel_id))
                .send_json()
                .await
        }
    }

    /// `PATCH /channels/{channel_id}`.
    pub fn update_channel(&self, channel_id: Id<ChannelMarker>) -> UpdateChannel {
        UpdateChannel {
            client: self.clone(),
            channel_id,
            body: UpdateChannelBody::default(),
            reason: None,
        }
    }

    /// `DELETE /channels/{channel_id}` (or close a DM).
    pub fn delete_channel(&self, channel_id: Id<ChannelMarker>) -> DeleteChannel {
        DeleteChannel {
            client: self.clone(),
            channel_id,
            reason: None,
        }
    }

    /// `PUT /channels/{channel_id}/permissions/{overwrite_id}`.
    pub fn edit_channel_permission<T>(
        &self,
        channel_id: Id<ChannelMarker>,
        overwrite_id: Id<T>,
    ) -> EditChannelPermission {
        EditChannelPermission {
            client: self.clone(),
            channel_id,
            overwrite_id: overwrite_id.cast(),
            kind: OverwriteType::Role,
            allow: Permissions::empty(),
            deny: Permissions::empty(),
            reason: None,
        }
    }

    /// `DELETE /channels/{channel_id}/permissions/{overwrite_id}`.
    pub fn delete_channel_permission<T>(
        &self,
        channel_id: Id<ChannelMarker>,
        overwrite_id: Id<T>,
    ) -> DeleteChannelPermission {
        DeleteChannelPermission {
            client: self.clone(),
            channel_id,
            overwrite_id: overwrite_id.cast(),
            reason: None,
        }
    }

    /// `POST /guilds/{guild_id}/channels`.
    pub fn create_guild_channel(
        &self,
        guild_id: Id<GuildMarker>,
        name: impl Into<String>,
    ) -> CreateGuildChannel {
        CreateGuildChannel {
            client: self.clone(),
            guild_id,
            body: CreateChannelBody {
                name: name.into(),
                ..CreateChannelBody::default()
            },
            reason: None,
        }
    }
}

/// Delete or close a channel.
pub struct DeleteChannel {
    client: Client,
    channel_id: Id<ChannelMarker>,
    reason: Option<String>,
}

impl DeleteChannel {
    /// Audit-log reason.
    pub fn reason(mut self, reason: impl Into<String>) -> Result<Self, HttpError> {
        super::set_reason(&mut self.reason, reason)?;
        Ok(self)
    }

    /// Send the request.
    pub async fn send(self) -> Result<Channel, HttpError> {
        self.client
            .delete(Route::raw("/channels/{channel_id}", self.channel_id))
            .maybe_reason(self.reason.as_deref())?
            .send_json()
            .await
    }
}

impl IntoFuture for DeleteChannel {
    type Output = Result<Channel, HttpError>;
    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output> + Send>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(self.send())
    }
}

#[derive(Serialize, Default)]
#[allow(clippy::option_option)] // Discord: omit vs JSON null (detach parent).
struct UpdateChannelBody {
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    kind: Option<ChannelType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    position: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    topic: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    nsfw: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    rate_limit_per_user: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    bitrate: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    user_limit: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    parent_id: Option<Option<Id<ChannelMarker>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    archived: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    auto_archive_duration: Option<AutoArchiveDuration>,
    #[serde(skip_serializing_if = "Option::is_none")]
    locked: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    invitable: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    permission_overwrites: Option<Vec<PermissionOverwrite>>,
}

/// `PATCH /channels/{channel_id}`.
pub struct UpdateChannel {
    client: Client,
    channel_id: Id<ChannelMarker>,
    body: UpdateChannelBody,
    reason: Option<String>,
}

impl UpdateChannel {
    /// Channel or thread name (1–100 characters).
    pub fn name(mut self, name: impl Into<String>) -> Result<Self, HttpError> {
        let name = name.into();
        channel_name_len(&name)?;
        self.body.name = Some(name);
        Ok(self)
    }

    /// Channel type conversion (text ↔ announcement only).
    #[must_use]
    pub fn kind(mut self, kind: ChannelType) -> Self {
        self.body.kind = Some(kind);
        self
    }

    /// Replace permission overwrites (an empty list removes all overwrites).
    #[must_use]
    pub fn permission_overwrites(mut self, overwrites: Vec<PermissionOverwrite>) -> Self {
        self.body.permission_overwrites = Some(overwrites);
        self
    }

    /// Sorting position.
    #[must_use]
    pub fn position(mut self, position: i32) -> Self {
        self.body.position = Some(position);
        self
    }

    /// Topic / guidelines.
    #[must_use]
    pub fn topic(mut self, topic: impl Into<String>) -> Self {
        self.body.topic = Some(topic.into());
        self
    }

    /// Age-restricted channel.
    #[must_use]
    pub fn nsfw(mut self, nsfw: bool) -> Self {
        self.body.nsfw = Some(nsfw);
        self
    }

    /// Slowmode in seconds.
    #[must_use]
    pub fn rate_limit_per_user(mut self, seconds: u32) -> Self {
        self.body.rate_limit_per_user = Some(seconds);
        self
    }

    /// Voice bitrate.
    #[must_use]
    pub fn bitrate(mut self, bitrate: u32) -> Self {
        self.body.bitrate = Some(bitrate);
        self
    }

    /// Voice user limit.
    #[must_use]
    pub fn user_limit(mut self, limit: u32) -> Self {
        self.body.user_limit = Some(limit);
        self
    }

    /// Parent category (or `None` to detach).
    #[must_use]
    pub fn parent_id(mut self, parent_id: Option<Id<ChannelMarker>>) -> Self {
        self.body.parent_id = Some(parent_id);
        self
    }

    /// Archive or unarchive a thread.
    #[must_use]
    pub fn archived(mut self, archived: bool) -> Self {
        self.body.archived = Some(archived);
        self
    }

    /// Thread auto-archive duration.
    #[must_use]
    pub fn auto_archive_duration(mut self, duration: AutoArchiveDuration) -> Self {
        self.body.auto_archive_duration = Some(duration);
        self
    }

    /// Lock a thread.
    #[must_use]
    pub fn locked(mut self, locked: bool) -> Self {
        self.body.locked = Some(locked);
        self
    }

    /// Whether non-moderators can add members (private threads).
    #[must_use]
    pub fn invitable(mut self, invitable: bool) -> Self {
        self.body.invitable = Some(invitable);
        self
    }

    /// Audit-log reason.
    pub fn reason(mut self, reason: impl Into<String>) -> Result<Self, HttpError> {
        super::set_reason(&mut self.reason, reason)?;
        Ok(self)
    }

    /// Send the request.
    pub async fn send(self) -> Result<Channel, HttpError> {
        self.client
            .patch(Route::raw("/channels/{channel_id}", self.channel_id))
            .maybe_reason(self.reason.as_deref())?
            .json(&self.body)?
            .send_json()
            .await
    }
}

impl IntoFuture for UpdateChannel {
    type Output = Result<Channel, HttpError>;
    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output> + Send>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(self.send())
    }
}

#[derive(Serialize)]
struct OverwriteBody {
    #[serde(rename = "type")]
    kind: OverwriteType,
    allow: Permissions,
    deny: Permissions,
}

/// `PUT /channels/{channel_id}/permissions/{overwrite_id}`.
pub struct EditChannelPermission {
    client: Client,
    channel_id: Id<ChannelMarker>,
    overwrite_id: Id<GenericMarker>,
    kind: OverwriteType,
    allow: Permissions,
    deny: Permissions,
    reason: Option<String>,
}

impl EditChannelPermission {
    /// Overwrite targets a member instead of a role.
    #[must_use]
    pub fn member(mut self) -> Self {
        self.kind = OverwriteType::Member;
        self
    }

    /// Overwrite targets a role (default).
    #[must_use]
    pub fn role(mut self) -> Self {
        self.kind = OverwriteType::Role;
        self
    }

    /// Allowed permissions.
    #[must_use]
    pub fn allow(mut self, allow: Permissions) -> Self {
        self.allow = allow;
        self
    }

    /// Denied permissions.
    #[must_use]
    pub fn deny(mut self, deny: Permissions) -> Self {
        self.deny = deny;
        self
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
                "/channels/{channel_id}/permissions/{overwrite_id}",
                (self.channel_id, self.overwrite_id),
            ))
            .maybe_reason(self.reason.as_deref())?
            .json(&OverwriteBody {
                kind: self.kind,
                allow: self.allow,
                deny: self.deny,
            })?
            .send_empty()
            .await
    }
}

impl IntoFuture for EditChannelPermission {
    type Output = Result<(), HttpError>;
    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output> + Send>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(self.send())
    }
}

/// `DELETE /channels/{channel_id}/permissions/{overwrite_id}`.
pub struct DeleteChannelPermission {
    client: Client,
    channel_id: Id<ChannelMarker>,
    overwrite_id: Id<GenericMarker>,
    reason: Option<String>,
}

impl DeleteChannelPermission {
    /// Audit-log reason.
    pub fn reason(mut self, reason: impl Into<String>) -> Result<Self, HttpError> {
        super::set_reason(&mut self.reason, reason)?;
        Ok(self)
    }

    /// Send the request.
    pub async fn send(self) -> Result<(), HttpError> {
        self.client
            .delete(Route::raw(
                "/channels/{channel_id}/permissions/{overwrite_id}",
                (self.channel_id, self.overwrite_id),
            ))
            .maybe_reason(self.reason.as_deref())?
            .send_empty()
            .await
    }
}

impl IntoFuture for DeleteChannelPermission {
    type Output = Result<(), HttpError>;
    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output> + Send>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(self.send())
    }
}

#[derive(Serialize, Default)]
struct CreateChannelBody {
    name: String,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    kind: Option<ChannelType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    topic: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    bitrate: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    user_limit: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    rate_limit_per_user: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    position: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    parent_id: Option<Id<ChannelMarker>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    nsfw: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    permission_overwrites: Option<Vec<PermissionOverwrite>>,
}

/// `POST /guilds/{guild_id}/channels`.
pub struct CreateGuildChannel {
    client: Client,
    guild_id: Id<GuildMarker>,
    body: CreateChannelBody,
    reason: Option<String>,
}

impl CreateGuildChannel {
    /// Channel type (default guild text).
    #[must_use]
    pub fn kind(mut self, kind: ChannelType) -> Self {
        self.body.kind = Some(kind);
        self
    }

    /// Initial permission overwrites for the new channel.
    #[must_use]
    pub fn permission_overwrites(mut self, overwrites: Vec<PermissionOverwrite>) -> Self {
        self.body.permission_overwrites = Some(overwrites);
        self
    }

    /// Topic.
    #[must_use]
    pub fn topic(mut self, topic: impl Into<String>) -> Self {
        self.body.topic = Some(topic.into());
        self
    }

    /// Voice bitrate.
    #[must_use]
    pub fn bitrate(mut self, bitrate: u32) -> Self {
        self.body.bitrate = Some(bitrate);
        self
    }

    /// Voice user limit.
    #[must_use]
    pub fn user_limit(mut self, limit: u32) -> Self {
        self.body.user_limit = Some(limit);
        self
    }

    /// Slowmode in seconds.
    #[must_use]
    pub fn rate_limit_per_user(mut self, seconds: u32) -> Self {
        self.body.rate_limit_per_user = Some(seconds);
        self
    }

    /// Sorting position.
    #[must_use]
    pub fn position(mut self, position: i32) -> Self {
        self.body.position = Some(position);
        self
    }

    /// Parent category.
    #[must_use]
    pub fn parent_id(mut self, parent_id: Id<ChannelMarker>) -> Self {
        self.body.parent_id = Some(parent_id);
        self
    }

    /// Age-restricted channel.
    #[must_use]
    pub fn nsfw(mut self, nsfw: bool) -> Self {
        self.body.nsfw = Some(nsfw);
        self
    }

    /// Audit-log reason.
    pub fn reason(mut self, reason: impl Into<String>) -> Result<Self, HttpError> {
        super::set_reason(&mut self.reason, reason)?;
        Ok(self)
    }

    /// Send the request.
    pub async fn send(self) -> Result<Channel, HttpError> {
        channel_name_len(&self.body.name)?;
        self.client
            .post(Route::raw("/guilds/{guild_id}/channels", self.guild_id))
            .maybe_reason(self.reason.as_deref())?
            .json(&self.body)?
            .send_json()
            .await
    }
}

impl IntoFuture for CreateGuildChannel {
    type Output = Result<Channel, HttpError>;
    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output> + Send>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(self.send())
    }
}

fn channel_name_len(name: &str) -> Result<(), HttpError> {
    let n = name.chars().count();
    if (1..=100).contains(&n) {
        Ok(())
    } else {
        Err(HttpError::validation(
            format!("channel name is {n} characters; must be 1–100").into_boxed_str(),
        ))
    }
}

impl crate::client::Client {
    simple! {
        /// `GET /channels/{channel_id}/invites`. Needs `MANAGE_CHANNELS`.
        fn get_channel_invites(channel_id: cordex_model::id::Id<cordex_model::id::marker::ChannelMarker>)
            -> Vec<cordex_model::invite::Invite> = get "/channels/{channel_id}/invites" => send_json
    }

    simple! {
        /// `POST /channels/{channel_id}/typing`: show "is typing" for about 10 s.
        fn trigger_typing(channel_id: cordex_model::id::Id<cordex_model::id::marker::ChannelMarker>)
            -> () = post "/channels/{channel_id}/typing" => send_empty
    }

    simple! {
        /// `DELETE /channels/{channel_id}/recipients/{user_id}`: remove a user
        /// from a group DM.
        fn remove_group_dm_recipient(
            channel_id: cordex_model::id::Id<cordex_model::id::marker::ChannelMarker>,
            user_id: cordex_model::id::Id<cordex_model::id::marker::UserMarker>,
        ) -> () = delete "/channels/{channel_id}/recipients/{user_id}" => send_empty
    }

    /// `PUT /channels/{channel_id}/recipients/{user_id}`: add a user to a
    /// group DM with their `OAuth2` access token (`gdm.join` scope).
    pub fn add_group_dm_recipient(
        &self,
        channel_id: cordex_model::id::Id<cordex_model::id::marker::ChannelMarker>,
        user_id: cordex_model::id::Id<cordex_model::id::marker::UserMarker>,
        access_token: impl Into<String>,
        nick: Option<&str>,
    ) -> impl std::future::Future<Output = Result<(), crate::error::HttpError>> + Send + 'static
    {
        let client = self.clone();
        let body = serde_json::json!({ "access_token": access_token.into(), "nick": nick });
        async move {
            client
                .put(crate::route::Route::raw(
                    "/channels/{channel_id}/recipients/{user_id}",
                    (channel_id, user_id),
                ))
                .json(&body)?
                .send_empty()
                .await
        }
    }

    /// `PUT /channels/{channel_id}/voice-status`: set (or with `None`
    /// clear) a voice channel's status, max. 500 characters.
    pub fn set_voice_channel_status(
        &self,
        channel_id: cordex_model::id::Id<cordex_model::id::marker::ChannelMarker>,
        status: Option<String>,
    ) -> impl std::future::Future<Output = Result<(), crate::error::HttpError>> + Send + 'static
    {
        let client = self.clone();
        async move {
            if status.as_ref().is_some_and(|s| s.chars().count() > 500) {
                return Err(crate::error::HttpError::Validation(
                    "voice channel status is longer than 500 characters".into(),
                ));
            }
            client
                .put(crate::route::Route::raw(
                    "/channels/{channel_id}/voice-status",
                    channel_id,
                ))
                .json(&serde_json::json!({ "status": status }))?
                .send_empty()
                .await
        }
    }

    /// `POST /channels/{channel_id}/followers`: follow an announcement
    /// channel into `target` (needs `MANAGE_WEBHOOKS` there).
    pub fn follow_announcement_channel(
        &self,
        channel_id: cordex_model::id::Id<cordex_model::id::marker::ChannelMarker>,
        target: cordex_model::id::Id<cordex_model::id::marker::ChannelMarker>,
    ) -> impl std::future::Future<
        Output = Result<cordex_model::channel::FollowedChannel, crate::error::HttpError>,
    > + Send
    + 'static {
        let client = self.clone();
        async move {
            client
                .post(crate::route::Route::raw(
                    "/channels/{channel_id}/followers",
                    channel_id,
                ))
                .json(&serde_json::json!({ "webhook_channel_id": target }))?
                .send_json()
                .await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_bodies_serialize_permission_overwrites_and_empty_clear() {
        let ov = PermissionOverwrite::role(
            Id::new(1).unwrap(),
            Permissions::VIEW_CHANNEL,
            Permissions::empty(),
        );
        let create = CreateChannelBody {
            name: "private".into(),
            permission_overwrites: Some(vec![ov]),
            ..CreateChannelBody::default()
        };
        let create_json = serde_json::to_value(&create).unwrap();
        assert_eq!(
            create_json["permission_overwrites"],
            serde_json::json!([{"id": "1", "type": 0, "allow": "1024", "deny": "0"}])
        );

        let update = UpdateChannelBody {
            permission_overwrites: Some(Vec::new()),
            ..UpdateChannelBody::default()
        };
        let update_json = serde_json::to_value(&update).unwrap();
        assert_eq!(update_json["permission_overwrites"], serde_json::json!([]));
    }
}
