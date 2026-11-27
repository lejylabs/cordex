//! Thread endpoints.

use std::future::{Future, IntoFuture};
use std::pin::Pin;

use cordex_model::channel::{AutoArchiveDuration, Channel, ChannelType, ThreadMember};
use cordex_model::component::Component;
use cordex_model::id::Id;
use cordex_model::id::marker::{
    ChannelMarker, GuildMarker, MessageMarker, StickerMarker, TagMarker, UserMarker,
};
use cordex_model::message::{AllowedMentions, Embed, MessageFlags};
use serde::{Deserialize, Serialize};

use crate::client::Client;
use crate::error::HttpError;
use crate::multipart::{File, Multipart};
use crate::route::Route;
use crate::validate::{
    FILE_COUNT_LIMIT, FILE_SIZE_LIMIT, STICKER_COUNT_LIMIT, content_len, embed_limits,
};

impl Client {
    /// Start a thread in a text or announcement channel (not from a message):
    /// `POST /channels/{channel_id}/threads`.
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

    /// Start a thread from an existing message:
    /// `POST /channels/{channel_id}/messages/{message_id}/threads`.
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

    /// Start a forum or media thread with an initial message:
    /// `POST /channels/{channel_id}/threads`.
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
            files: Vec::new(),
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
    files: Vec<File>,
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

    /// Applied forum tags (max. 5 tags).
    #[must_use]
    pub fn applied_tags(mut self, tags: impl Into<Vec<Id<TagMarker>>>) -> Self {
        self.applied_tags = tags.into();
        self
    }

    /// Append a single forum tag ID.
    #[must_use]
    pub fn tag(mut self, tag: Id<TagMarker>) -> Self {
        self.applied_tags.push(tag);
        self
    }

    /// Starter message content (max. 2000 characters).
    pub fn content(mut self, content: impl Into<String>) -> Result<Self, HttpError> {
        let content = content.into();
        content_len(&content)?;
        self.message.content = Some(content);
        Ok(self)
    }

    /// Add one embed to the starter message (max. 10, 6000 characters total).
    pub fn embed(mut self, embed: Embed) -> Result<Self, HttpError> {
        let embeds = self.message.embeds.get_or_insert_with(Vec::new);
        embeds.push(embed);
        embed_limits(embeds)?;
        Ok(self)
    }

    /// Replace the starter message's embed list.
    pub fn embeds(mut self, embeds: Vec<Embed>) -> Result<Self, HttpError> {
        embed_limits(&embeds)?;
        self.message.embeds = Some(embeds);
        Ok(self)
    }

    /// Allowed mentions for the starter message.
    #[must_use]
    pub fn allowed_mentions(mut self, mentions: AllowedMentions) -> Self {
        self.message.allowed_mentions = Some(mentions);
        self
    }

    /// Components for the starter message.
    #[must_use]
    pub fn components(mut self, components: Vec<Component>) -> Self {
        self.message.components = Some(components);
        self
    }

    /// Stickers for the starter message (max. 3).
    pub fn sticker_ids(
        mut self,
        ids: impl IntoIterator<Item = Id<StickerMarker>>,
    ) -> Result<Self, HttpError> {
        let ids: Vec<_> = ids.into_iter().collect();
        if ids.len() > STICKER_COUNT_LIMIT {
            return Err(HttpError::validation(
                format!("a message can have at most {STICKER_COUNT_LIMIT} stickers")
                    .into_boxed_str(),
            ));
        }
        self.message.sticker_ids = ids;
        Ok(self)
    }

    /// Message flags for the starter message (`SUPPRESS_EMBEDS`, `IS_COMPONENTS_V2`, …).
    #[must_use]
    pub fn flags(mut self, flags: MessageFlags) -> Self {
        self.message.flags = Some(flags);
        self
    }

    /// Attach a file to the starter message (`files[n]` + `attachments` entry).
    pub fn attach(mut self, file: File) -> Result<Self, HttpError> {
        if file.data.len() > FILE_SIZE_LIMIT {
            return Err(HttpError::validation(
                format!(
                    "file `{}` is {} bytes; max {FILE_SIZE_LIMIT}",
                    file.filename,
                    file.data.len()
                )
                .into_boxed_str(),
            ));
        }
        if self.files.len() >= FILE_COUNT_LIMIT {
            return Err(HttpError::validation(
                format!("a message can have at most {FILE_COUNT_LIMIT} files").into_boxed_str(),
            ));
        }
        self.files.push(file);
        Ok(self)
    }

    /// Audit-log reason.
    pub fn reason(mut self, reason: impl Into<String>) -> Result<Self, HttpError> {
        super::set_reason(&mut self.reason, reason)?;
        Ok(self)
    }

    /// Send the request.
    pub async fn send(mut self) -> Result<Channel, HttpError> {
        thread_name_len(&self.name)?;
        if self.message.content.as_ref().is_none_or(String::is_empty)
            && self.message.embeds.as_ref().is_none_or(Vec::is_empty)
            && self.message.components.as_ref().is_none_or(Vec::is_empty)
            && self.message.sticker_ids.is_empty()
            && self.files.is_empty()
        {
            return Err(HttpError::validation(
                "forum thread needs content, an embed, components, a sticker or a file",
            ));
        }
        if let Some(components) = &self.message.components {
            super::mark_components_v2(&mut self.message.flags, components);
        }
        if !self.files.is_empty() {
            self.message.attachments = Some(
                self.files
                    .iter()
                    .enumerate()
                    .map(|(i, f)| super::message::PartialAttachment {
                        id: i as u64,
                        filename: f.filename.clone(),
                        description: f.description.clone(),
                    })
                    .collect(),
            );
        }
        let payload = ForumThreadBody {
            name: &self.name,
            auto_archive_duration: self.auto_archive_duration,
            rate_limit_per_user: self.rate_limit_per_user,
            applied_tags: self.applied_tags,
            message: &self.message,
        };
        let req = self
            .client
            .post(Route::raw(
                "/channels/{channel_id}/threads",
                self.channel_id,
            ))
            .maybe_reason(self.reason.as_deref())?;
        if self.files.is_empty() {
            req.json(&payload)?.send_json().await
        } else {
            let payload_json = bytes::Bytes::from(
                serde_json::to_vec(&payload)
                    .map_err(|e| HttpError::validation(e.to_string().into_boxed_str()))?,
            );
            let mp = Multipart {
                payload_json,
                files: self.files,
            };
            req.multipart(&mp)?.send_json().await
        }
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

query_request! {
    /// Archived threads, newest first.
    GetArchivedThreads -> ThreadList {
        /// Threads archived before this ISO 8601 timestamp (for joined
        /// private threads: before this thread ID).
        before: String,
        /// Max. threads to return.
        limit: u8,
    }
}

query_request! {
    /// `GET /channels/{channel_id}/thread-members`.
    GetThreadMembers -> Vec<ThreadMember> {
        /// Include the guild member in each entry.
        with_member: bool,
        /// Members after this user ID (paging, needs `with_member`).
        after: Id<cordex_model::id::marker::UserMarker>,
        /// Max. members (1-100, default 100).
        limit: u8,
    }
}

query_request! {
    /// `GET /channels/{channel_id}/thread-members/{user_id}`.
    GetThreadMember -> ThreadMember {
        /// Include the guild member.
        with_member: bool,
    }
}

impl Client {
    simple! {
        /// `DELETE /channels/{channel_id}/thread-members/{user_id}`.
        fn remove_thread_member(channel_id: Id<ChannelMarker>, user_id: Id<cordex_model::id::marker::UserMarker>)
            -> () = delete "/channels/{channel_id}/thread-members/{user_id}" => send_empty
    }

    /// `GET /channels/{channel_id}/thread-members/{user_id}`.
    pub fn get_thread_member(
        &self,
        channel_id: Id<ChannelMarker>,
        user_id: Id<cordex_model::id::marker::UserMarker>,
    ) -> GetThreadMember {
        GetThreadMember::new(
            self,
            Route::raw(
                "/channels/{channel_id}/thread-members/{user_id}",
                (channel_id, user_id),
            ),
        )
    }

    /// `GET /channels/{channel_id}/thread-members`.
    pub fn get_thread_members(&self, channel_id: Id<ChannelMarker>) -> GetThreadMembers {
        GetThreadMembers::new(
            self,
            Route::raw("/channels/{channel_id}/thread-members", channel_id),
        )
    }

    /// `GET /channels/{channel_id}/threads/archived/public`.
    pub fn get_public_archived_threads(&self, channel_id: Id<ChannelMarker>) -> GetArchivedThreads {
        GetArchivedThreads::new(
            self,
            Route::raw("/channels/{channel_id}/threads/archived/public", channel_id),
        )
    }

    /// `GET /channels/{channel_id}/threads/archived/private`. Needs
    /// `MANAGE_THREADS`.
    pub fn get_private_archived_threads(
        &self,
        channel_id: Id<ChannelMarker>,
    ) -> GetArchivedThreads {
        GetArchivedThreads::new(
            self,
            Route::raw(
                "/channels/{channel_id}/threads/archived/private",
                channel_id,
            ),
        )
    }

    /// `GET /channels/{channel_id}/users/@me/threads/archived/private`:
    /// private archived threads the bot has joined.
    pub fn get_joined_private_archived_threads(
        &self,
        channel_id: Id<ChannelMarker>,
    ) -> GetArchivedThreads {
        GetArchivedThreads::new(
            self,
            Route::raw(
                "/channels/{channel_id}/users/@me/threads/archived/private",
                channel_id,
            ),
        )
    }
}
