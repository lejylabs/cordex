//! Thread endpoints.

use std::future::{Future, IntoFuture};
use std::pin::Pin;

use cordex_model::channel::{AutoArchiveDuration, Channel, ChannelType, ThreadMember};
use cordex_model::id::Id;
use cordex_model::id::marker::{ChannelMarker, GuildMarker, MessageMarker, TagMarker, UserMarker};
use serde::{Deserialize, Serialize};

use crate::client::Client;
use crate::error::HttpError;
use crate::route::Route;

impl Client {
    /// Start a thread in a text or announcement channel (not from a message).
    pub fn create_thread(
        &self,
        channel_id: Id<ChannelMarker>,
        name: impl Into<String>,
    ) -> CreateThread {
        CreateThread {
            client: self.clone(),
            channel_id,
            message_id: None,
            body: CreateThreadBody {
                name: name.into(),
                ..CreateThreadBody::default()
            },
            reason: None,
        }
    }

    /// Start a thread from an existing message.
    pub fn create_thread_from_message(
        &self,
        channel_id: Id<ChannelMarker>,
        message_id: Id<MessageMarker>,
        name: impl Into<String>,
    ) -> CreateThread {
        CreateThread {
            client: self.clone(),
            channel_id,
            message_id: Some(message_id),
            body: CreateThreadBody {
                name: name.into(),
                ..CreateThreadBody::default()
            },
            reason: None,
        }
    }

    /// Start a forum or media thread with an initial message.
    pub fn create_forum_thread(
        &self,
        channel_id: Id<ChannelMarker>,
        name: impl Into<String>,
    ) -> CreateForumThread {
        CreateForumThread {
            client: self.clone(),
            channel_id,
            name: name.into(),
            auto_archive_duration: None,
            rate_limit_per_user: None,
            applied_tags: Vec::new(),
            message: super::message::MessageBody::default(),
            reason: None,
        }
    }

    /// `PUT /channels/{channel_id}/thread-members/@me`.
    pub fn join_thread(
        &self,
        channel_id: Id<ChannelMarker>,
    ) -> impl Future<Output = Result<(), HttpError>> + Send + 'static {
        let client = self.clone();
        async move {
            client
                .put(Route::raw(
                    "/channels/{channel_id}/thread-members/@me",
                    channel_id,
                ))
                .send_empty()
                .await
        }
    }

    /// `DELETE /channels/{channel_id}/thread-members/@me`.
    pub fn leave_thread(
        &self,
        channel_id: Id<ChannelMarker>,
    ) -> impl Future<Output = Result<(), HttpError>> + Send + 'static {
        let client = self.clone();
        async move {
            client
                .delete(Route::raw(
                    "/channels/{channel_id}/thread-members/@me",
                    channel_id,
                ))
                .send_empty()
                .await
        }
    }

    /// `PUT /channels/{channel_id}/thread-members/{user_id}`.
    pub fn add_thread_member(
        &self,
        channel_id: Id<ChannelMarker>,
        user_id: Id<UserMarker>,
    ) -> impl Future<Output = Result<(), HttpError>> + Send + 'static {
        let client = self.clone();
        async move {
            client
                .put(Route::raw(
                    "/channels/{channel_id}/thread-members/{user_id}",
                    (channel_id, user_id),
                ))
                .send_empty()
                .await
        }
    }

    /// `GET /guilds/{guild_id}/threads/active`.
    pub fn get_active_guild_threads(
        &self,
        guild_id: Id<GuildMarker>,
    ) -> impl Future<Output = Result<ThreadList, HttpError>> + Send + 'static {
        let client = self.clone();
        async move {
            client
                .get(Route::raw("/guilds/{guild_id}/threads/active", guild_id))
                .send_json()
                .await
        }
    }
}

/// Active or archived thread list.
#[derive(Debug, Clone, Deserialize)]
#[non_exhaustive]
pub struct ThreadList {
    /// Threads.
    pub threads: Vec<Channel>,
    /// Thread members for the current user.
    pub members: Vec<ThreadMember>,
    /// Whether another page of archived threads exists.
    #[serde(default)]
    pub has_more: bool,
}

#[derive(Serialize, Default)]
struct CreateThreadBody {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    auto_archive_duration: Option<AutoArchiveDuration>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    kind: Option<ChannelType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    invitable: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    rate_limit_per_user: Option<u32>,
}

/// `POST /channels/{channel_id}/threads` or from a message.
pub struct CreateThread {
    client: Client,
    channel_id: Id<ChannelMarker>,
    message_id: Option<Id<MessageMarker>>,
    body: CreateThreadBody,
    reason: Option<String>,
}

impl CreateThread {
    /// Auto-archive duration.
    #[must_use]
    pub fn auto_archive_duration(mut self, duration: AutoArchiveDuration) -> Self {
        self.body.auto_archive_duration = Some(duration);
        self
    }

    /// Thread type (public / private / announcement). Ignored when started from a message.
    #[must_use]
    pub fn kind(mut self, kind: ChannelType) -> Self {
        self.body.kind = Some(kind);
        self
    }

    /// Whether non-moderators can add members (private threads).
    #[must_use]
    pub fn invitable(mut self, invitable: bool) -> Self {
        self.body.invitable = Some(invitable);
        self
    }

    /// Slowmode in seconds.
    #[must_use]
    pub fn rate_limit_per_user(mut self, seconds: u32) -> Self {
        self.body.rate_limit_per_user = Some(seconds);
        self
    }

    /// Audit-log reason.
    pub fn reason(mut self, reason: impl Into<String>) -> Result<Self, HttpError> {
        super::set_reason(&mut self.reason, reason)?;
        Ok(self)
    }

    /// Send the request.
    pub async fn send(self) -> Result<Channel, HttpError> {
        thread_name_len(&self.body.name)?;
        let route = match self.message_id {
            Some(message_id) => Route::raw(
                "/channels/{channel_id}/messages/{message_id}/threads",
                (self.channel_id, message_id),
            ),
            None => Route::raw("/channels/{channel_id}/threads", self.channel_id),
        };
        self.client
            .post(route)
            .maybe_reason(self.reason.as_deref())?
            .json(&self.body)?
            .send_json()
            .await
    }
}

impl IntoFuture for CreateThread {
    type Output = Result<Channel, HttpError>;
    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output> + Send>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(self.send())
    }
}

#[derive(Serialize)]
struct ForumThreadBody<'a> {
    name: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    auto_archive_duration: Option<AutoArchiveDuration>,
    #[serde(skip_serializing_if = "Option::is_none")]
    rate_limit_per_user: Option<u32>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    applied_tags: Vec<Id<TagMarker>>,
    message: &'a super::message::MessageBody,
}

/// Forum / media thread with starter message.
pub struct CreateForumThread {
    client: Client,
    channel_id: Id<ChannelMarker>,
    name: String,
    auto_archive_duration: Option<AutoArchiveDuration>,
    rate_limit_per_user: Option<u32>,
    applied_tags: Vec<Id<TagMarker>>,
    message: super::message::MessageBody,
    reason: Option<String>,
}

impl CreateForumThread {
    /// Auto-archive duration.
    #[must_use]
    pub fn auto_archive_duration(mut self, duration: AutoArchiveDuration) -> Self {
        self.auto_archive_duration = Some(duration);
        self
    }

    /// Slowmode in seconds.
    #[must_use]
    pub fn rate_limit_per_user(mut self, seconds: u32) -> Self {
        self.rate_limit_per_user = Some(seconds);
        self
    }

    /// Applied forum tags.
    #[must_use]
    pub fn applied_tags(mut self, tags: Vec<Id<TagMarker>>) -> Self {
        self.applied_tags = tags;
        self
    }

    /// Starter message content.
    pub fn content(mut self, content: impl Into<String>) -> Result<Self, HttpError> {
        let content = content.into();
        crate::validate::content_len(&content)?;
        self.message.content = Some(content);
        Ok(self)
    }

    /// Audit-log reason.
    pub fn reason(mut self, reason: impl Into<String>) -> Result<Self, HttpError> {
        super::set_reason(&mut self.reason, reason)?;
        Ok(self)
    }

    /// Send the request.
    pub async fn send(self) -> Result<Channel, HttpError> {
        thread_name_len(&self.name)?;
        if self.message.content.as_ref().is_none_or(String::is_empty)
            && self.message.embeds.is_empty()
            && self.message.components.is_empty()
        {
            return Err(HttpError::validation(
                "forum thread needs content, an embed or components",
            ));
        }
        self.client
            .post(Route::raw(
                "/channels/{channel_id}/threads",
                self.channel_id,
            ))
            .maybe_reason(self.reason.as_deref())?
            .json(&ForumThreadBody {
                name: &self.name,
                auto_archive_duration: self.auto_archive_duration,
                rate_limit_per_user: self.rate_limit_per_user,
                applied_tags: self.applied_tags,
                message: &self.message,
            })?
            .send_json()
            .await
    }
}

impl IntoFuture for CreateForumThread {
    type Output = Result<Channel, HttpError>;
    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output> + Send>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(self.send())
    }
}

fn thread_name_len(name: &str) -> Result<(), HttpError> {
    let n = name.chars().count();
    if (1..=100).contains(&n) {
        Ok(())
    } else {
        Err(HttpError::validation(
            format!("thread name is {n} characters; must be 1–100").into_boxed_str(),
        ))
    }
}
