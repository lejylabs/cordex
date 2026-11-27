//! Channel message endpoints.

use std::future::{Future, IntoFuture};
use std::pin::Pin;

use cordex_model::component::Component;
use cordex_model::id::Id;
use cordex_model::id::marker::{ChannelMarker, MessageMarker, StickerMarker};
use cordex_model::message::{
    AllowedMentions, ChannelPins, Embed, Message, MessageFlags, MessageReference,
    MessageReferenceType,
};
use cordex_model::timestamp::Timestamp;
use http::Method;
use serde::Serialize;

use crate::client::Client;
use crate::error::HttpError;
use crate::multipart::{File, Multipart};
use crate::route::Route;
use crate::validate::{
    FILE_COUNT_LIMIT, FILE_SIZE_LIMIT, STICKER_COUNT_LIMIT, content_len, embed_limits,
};

#[derive(Serialize, Default)]
pub(crate) struct MessageBody {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    nonce: Option<String>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    tts: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) embeds: Option<Vec<Embed>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) allowed_mentions: Option<AllowedMentions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    message_reference: Option<MessageReference>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) components: Option<Vec<Component>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) sticker_ids: Vec<Id<StickerMarker>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) attachments: Option<Vec<PartialAttachment>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) flags: Option<MessageFlags>,
}

#[derive(Serialize)]
pub(crate) struct PartialAttachment {
    pub(crate) id: u64,
    pub(crate) filename: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) description: Option<String>,
}

/// `POST /channels/{channel_id}/messages`.
pub struct CreateMessage {
    client: Client,
    channel_id: Id<ChannelMarker>,
    body: MessageBody,
    files: Vec<File>,
    reason: Option<String>,
}

/// `PATCH /channels/{channel_id}/messages/{message_id}`.
pub struct EditMessage {
    client: Client,
    channel_id: Id<ChannelMarker>,
    message_id: Id<MessageMarker>,
    body: MessageBody,
    files: Vec<File>,
    reason: Option<String>,
}

/// Async paginator over a channel's message history, from [`Client::messages_iter`].
pub struct MessageHistory {
    client: Client,
    channel_id: Id<ChannelMarker>,
    before: Option<Id<MessageMarker>>,
    remaining: Option<usize>,
    buffer: std::collections::VecDeque<Message>,
    done: bool,
}

impl MessageHistory {
    /// Start fetching messages before this message ID.
    #[must_use]
    pub fn before(mut self, id: Id<MessageMarker>) -> Self {
        self.before = Some(id);
        self
    }

    /// Stop after yielding at most `max` messages.
    #[must_use]
    pub fn limit(mut self, max: usize) -> Self {
        self.remaining = Some(max);
        self
    }

    /// Next message (newest to oldest), fetching another page when needed.
    pub async fn next(&mut self) -> Option<Result<Message, HttpError>> {
        if let Some(0) = self.remaining {
            return None;
        }
        if let Some(msg) = self.buffer.pop_front() {
            if let Some(rem) = &mut self.remaining {
                *rem = rem.saturating_sub(1);
            }
            return Some(Ok(msg));
        }
        if self.done {
            return None;
        }
        let page_size = self
            .remaining
            .map_or(100_u8, |r| u8::try_from(r.min(100)).unwrap_or(100));
        let mut req = self
            .client
            .get_channel_messages(self.channel_id)
            .limit(page_size);
        if let Some(before) = self.before {
            req = req.before(before);
        }
        match req.send().await {
            Ok(page) => {
                if page.len() < usize::from(page_size) {
                    self.done = true;
                }
                if let Some(last) = page.last() {
                    self.before = Some(last.id);
                }
                self.buffer.extend(page);
                let msg = self.buffer.pop_front()?;
                if let Some(rem) = &mut self.remaining {
                    *rem = rem.saturating_sub(1);
                }
                Some(Ok(msg))
            }
            Err(err) => {
                self.done = true;
                Some(Err(err))
            }
        }
    }
}

/// `GET /channels/{channel_id}/messages`.
pub struct GetChannelMessages {
    client: Client,
    channel_id: Id<ChannelMarker>,
    limit: Option<u8>,
    before: Option<Id<MessageMarker>>,
    after: Option<Id<MessageMarker>>,
    around: Option<Id<MessageMarker>>,
}

impl Client {
    /// Create a message in a channel: `POST /channels/{channel_id}/messages`.
    pub fn create_message(&self, channel_id: Id<ChannelMarker>) -> CreateMessage {
        CreateMessage {
            client: self.clone(),
            channel_id,
            body: MessageBody::default(),
            files: Vec::new(),
            reason: None,
        }
    }

    /// Forward an existing message into `destination_channel_id` using a
    /// [`MessageReference::forward`] payload.
    pub fn forward_message(
        &self,
        destination_channel_id: Id<ChannelMarker>,
        source_channel_id: Id<ChannelMarker>,
        message_id: Id<MessageMarker>,
    ) -> CreateMessage {
        self.create_message(destination_channel_id)
            .message_reference(MessageReference::forward(source_channel_id, message_id))
    }

    /// Fetch a single message: `GET /channels/{channel_id}/messages/{message_id}`.
    pub fn get_message(
        &self,
        channel_id: Id<ChannelMarker>,
        message_id: Id<MessageMarker>,
    ) -> impl Future<Output = Result<Message, HttpError>> + Send + 'static {
        let client = self.clone();
        async move {
            client
                .get(Route::raw(
                    "/channels/{channel_id}/messages/{message_id}",
                    (channel_id, message_id),
                ))
                .send_json()
                .await
        }
    }

    /// List messages in a channel: `GET /channels/{channel_id}/messages`.
    pub fn get_channel_messages(&self, channel_id: Id<ChannelMarker>) -> GetChannelMessages {
        GetChannelMessages {
            client: self.clone(),
            channel_id,
            limit: None,
            before: None,
            after: None,
            around: None,
        }
    }

    /// Paginate backwards through a channel's message history in batches of
    /// up to 100 messages.
    pub fn messages_iter(&self, channel_id: Id<ChannelMarker>) -> MessageHistory {
        MessageHistory {
            client: self.clone(),
            channel_id,
            before: None,
            remaining: None,
            buffer: std::collections::VecDeque::new(),
            done: false,
        }
    }

    /// Keep Discord's "bot is typing…" indicator active in `channel_id` until
    /// the returned [`Typing`](crate::Typing) guard is dropped.
    pub fn start_typing(&self, channel_id: Id<ChannelMarker>) -> crate::Typing {
        crate::Typing::start(self.clone(), channel_id)
    }

    /// Edit a message the bot sent: `PATCH /channels/{channel_id}/messages/{message_id}`.
    pub fn update_message(
        &self,
        channel_id: Id<ChannelMarker>,
        message_id: Id<MessageMarker>,
    ) -> EditMessage {
        EditMessage {
            client: self.clone(),
            channel_id,
            message_id,
            body: MessageBody::default(),
            files: Vec::new(),
            reason: None,
        }
    }

    /// Delete a message: `DELETE /channels/{channel_id}/messages/{message_id}`.
    pub fn delete_message(
        &self,
        channel_id: Id<ChannelMarker>,
        message_id: Id<MessageMarker>,
    ) -> DeleteMessage {
        DeleteMessage {
            client: self.clone(),
            channel_id,
            message_id,
            reason: None,
        }
    }

    /// Bulk-delete 2–100 messages: `POST /channels/{channel_id}/messages/bulk-delete`.
    pub fn delete_messages(
        &self,
        channel_id: Id<ChannelMarker>,
        message_ids: impl IntoIterator<Item = Id<MessageMarker>>,
    ) -> DeleteMessages {
        DeleteMessages {
            client: self.clone(),
            channel_id,
            message_ids: message_ids.into_iter().collect(),
            reason: None,
        }
    }

    /// Pin a message (`PUT /channels/{channel_id}/messages/pins/{message_id}`).
    pub fn pin_message(
        &self,
        channel_id: Id<ChannelMarker>,
        message_id: Id<MessageMarker>,
    ) -> PinMessage {
        PinMessage {
            client: self.clone(),
            channel_id,
            message_id,
            reason: None,
        }
    }

    /// Unpin a message: `DELETE /channels/{channel_id}/messages/pins/{message_id}`.
    pub fn unpin_message(
        &self,
        channel_id: Id<ChannelMarker>,
        message_id: Id<MessageMarker>,
    ) -> UnpinMessage {
        UnpinMessage {
            client: self.clone(),
            channel_id,
            message_id,
            reason: None,
        }
    }

    /// List pins in a channel (`GET /channels/{channel_id}/messages/pins`).
    pub fn get_channel_pins(&self, channel_id: Id<ChannelMarker>) -> GetChannelPins {
        GetChannelPins {
            client: self.clone(),
            channel_id,
            before: None,
            limit: None,
        }
    }
}

/// `DELETE /channels/{channel_id}/messages/{message_id}`.
pub struct DeleteMessage {
    client: Client,
    channel_id: Id<ChannelMarker>,
    message_id: Id<MessageMarker>,
    reason: Option<String>,
}

macro_rules! message_setters {
    ($ty:ty) => {
        impl $ty {
            /// Message content (max. 2000 characters).
            pub fn content(mut self, content: impl Into<String>) -> Result<Self, HttpError> {
                let content = content.into();
                content_len(&content)?;
                self.body.content = Some(content);
                Ok(self)
            }

            /// One embed (max. 10, 6000 characters of text in total).
            pub fn embed(mut self, embed: Embed) -> Result<Self, HttpError> {
                let embeds = self.body.embeds.get_or_insert_with(Vec::new);
                embeds.push(embed);
                embed_limits(embeds)?;
                Ok(self)
            }

            /// Replace the embed list.
            pub fn embeds(mut self, embeds: Vec<Embed>) -> Result<Self, HttpError> {
                embed_limits(&embeds)?;
                self.body.embeds = Some(embeds);
                Ok(self)
            }

            /// Clear all embeds on the message (sends `"embeds": []`).
            #[must_use]
            pub fn clear_embeds(mut self) -> Self {
                self.body.embeds = Some(Vec::new());
                self
            }

            /// Allowed mentions.
            #[must_use]
            pub fn allowed_mentions(mut self, mentions: AllowedMentions) -> Self {
                self.body.allowed_mentions = Some(mentions);
                self
            }

            /// Message components (an empty list removes them on edit).
            #[must_use]
            pub fn components(mut self, components: Vec<Component>) -> Self {
                self.body.components = Some(components);
                self
            }

            /// Message flags (`SUPPRESS_EMBEDS`, `IS_COMPONENTS_V2`, …).
            #[must_use]
            pub fn flags(mut self, flags: MessageFlags) -> Self {
                self.body.flags = Some(flags);
                self
            }

            /// Attach a file (`files[n]` + `attachments` entry).
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
                        format!("a message can have at most {FILE_COUNT_LIMIT} files")
                            .into_boxed_str(),
                    ));
                }
                self.files.push(file);
                Ok(self)
            }

            /// Clear all attachments on the message (sends `"attachments": []`).
            #[must_use]
            pub fn clear_attachments(mut self) -> Self {
                self.files.clear();
                self.body.attachments = Some(Vec::new());
                self
            }

            /// Audit-log reason (unused on create; applied on edit).
            pub fn reason(mut self, reason: impl Into<String>) -> Result<Self, HttpError> {
                super::set_reason(&mut self.reason, reason)?;
                Ok(self)
            }
        }
    };
}

message_setters!(CreateMessage);
message_setters!(EditMessage);

impl CreateMessage {
    /// Text-to-speech.
    #[must_use]
    pub fn tts(mut self, tts: bool) -> Self {
        self.body.tts = tts;
        self
    }

    /// Reply to a message.
    #[must_use]
    pub fn reply(mut self, message_id: Id<MessageMarker>) -> Self {
        self.body.message_reference = Some(MessageReference::reply(message_id));
        self
    }

    /// Full message reference (reply or forward).
    #[must_use]
    pub fn message_reference(mut self, reference: MessageReference) -> Self {
        self.body.message_reference = Some(reference);
        self
    }

    /// Stickers (max. 3).
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
        self.body.sticker_ids = ids;
        Ok(self)
    }

    /// Unique nonce (up to 25 characters).
    #[must_use]
    pub fn nonce(mut self, nonce: impl Into<String>) -> Self {
        self.body.nonce = Some(nonce.into());
        self
    }

    /// Send the request.
    pub async fn send(self) -> Result<Message, HttpError> {
        send_message(
            self.client,
            Method::POST,
            Route::raw("/channels/{channel_id}/messages", self.channel_id),
            self.body,
            self.files,
            self.reason,
            true,
        )
        .await
    }
}

impl EditMessage {
    /// Send the request.
    pub async fn send(self) -> Result<Message, HttpError> {
        send_message(
            self.client,
            Method::PATCH,
            Route::raw(
                "/channels/{channel_id}/messages/{message_id}",
                (self.channel_id, self.message_id),
            ),
            self.body,
            self.files,
            self.reason,
            false,
        )
        .await
    }
}

impl GetChannelMessages {
    /// Page size (1–100, Discord default 50).
    #[must_use]
    pub fn limit(mut self, limit: u8) -> Self {
        self.limit = Some(limit.clamp(1, 100));
        self
    }

    /// Messages before this ID.
    #[must_use]
    pub fn before(mut self, id: Id<MessageMarker>) -> Self {
        self.before = Some(id);
        self
    }

    /// Messages after this ID.
    #[must_use]
    pub fn after(mut self, id: Id<MessageMarker>) -> Self {
        self.after = Some(id);
        self
    }

    /// Messages around this ID.
    #[must_use]
    pub fn around(mut self, id: Id<MessageMarker>) -> Self {
        self.around = Some(id);
        self
    }

    /// Send the request.
    pub async fn send(self) -> Result<Vec<Message>, HttpError> {
        let mut req = self.client.get(Route::raw(
            "/channels/{channel_id}/messages",
            self.channel_id,
        ));
        if let Some(limit) = self.limit {
            req = req.query("limit", limit);
        }
        if let Some(id) = self.before {
            req = req.query("before", id);
        }
        if let Some(id) = self.after {
            req = req.query("after", id);
        }
        if let Some(id) = self.around {
            req = req.query("around", id);
        }
        req.send_json().await
    }
}

impl DeleteMessage {
    /// Audit-log reason.
    pub fn reason(mut self, reason: impl Into<String>) -> Result<Self, HttpError> {
        super::set_reason(&mut self.reason, reason)?;
        Ok(self)
    }

    /// Send the request.
    pub async fn send(self) -> Result<(), HttpError> {
        self.client
            .delete(Route::raw(
                "/channels/{channel_id}/messages/{message_id}",
                (self.channel_id, self.message_id),
            ))
            .maybe_reason(self.reason.as_deref())?
            .send_empty()
            .await
    }
}

#[derive(Serialize)]
struct BulkDeleteBody<'a> {
    messages: &'a [Id<MessageMarker>],
}

/// `POST /channels/{channel_id}/messages/bulk-delete`.
pub struct DeleteMessages {
    client: Client,
    channel_id: Id<ChannelMarker>,
    message_ids: Vec<Id<MessageMarker>>,
    reason: Option<String>,
}

impl DeleteMessages {
    /// Audit-log reason.
    pub fn reason(mut self, reason: impl Into<String>) -> Result<Self, HttpError> {
        super::set_reason(&mut self.reason, reason)?;
        Ok(self)
    }

    /// Send the request.
    pub async fn send(self) -> Result<(), HttpError> {
        let n = self.message_ids.len();
        if !(2..=100).contains(&n) {
            return Err(HttpError::validation(
                format!("bulk delete needs 2–100 message ids (got {n})").into_boxed_str(),
            ));
        }
        self.client
            .post(Route::raw(
                "/channels/{channel_id}/messages/bulk-delete",
                self.channel_id,
            ))
            .maybe_reason(self.reason.as_deref())?
            .json(&BulkDeleteBody {
                messages: &self.message_ids,
            })?
            .send_empty()
            .await
    }
}

/// `PUT /channels/{channel_id}/messages/pins/{message_id}`.
pub struct PinMessage {
    client: Client,
    channel_id: Id<ChannelMarker>,
    message_id: Id<MessageMarker>,
    reason: Option<String>,
}

impl PinMessage {
    /// Audit-log reason.
    pub fn reason(mut self, reason: impl Into<String>) -> Result<Self, HttpError> {
        super::set_reason(&mut self.reason, reason)?;
        Ok(self)
    }

    /// Send the request.
    pub async fn send(self) -> Result<(), HttpError> {
        self.client
            .put(Route::raw(
                "/channels/{channel_id}/messages/pins/{message_id}",
                (self.channel_id, self.message_id),
            ))
            .maybe_reason(self.reason.as_deref())?
            .send_empty()
            .await
    }
}

/// `DELETE /channels/{channel_id}/messages/pins/{message_id}`.
pub struct UnpinMessage {
    client: Client,
    channel_id: Id<ChannelMarker>,
    message_id: Id<MessageMarker>,
    reason: Option<String>,
}

impl UnpinMessage {
    /// Audit-log reason.
    pub fn reason(mut self, reason: impl Into<String>) -> Result<Self, HttpError> {
        super::set_reason(&mut self.reason, reason)?;
        Ok(self)
    }

    /// Send the request.
    pub async fn send(self) -> Result<(), HttpError> {
        self.client
            .delete(Route::raw(
                "/channels/{channel_id}/messages/pins/{message_id}",
                (self.channel_id, self.message_id),
            ))
            .maybe_reason(self.reason.as_deref())?
            .send_empty()
            .await
    }
}

/// `GET /channels/{channel_id}/messages/pins`.
pub struct GetChannelPins {
    client: Client,
    channel_id: Id<ChannelMarker>,
    before: Option<Timestamp>,
    limit: Option<u8>,
}

impl GetChannelPins {
    /// Pins before this pin timestamp.
    #[must_use]
    pub fn before(mut self, before: Timestamp) -> Self {
        self.before = Some(before);
        self
    }

    /// Page size (1–50, Discord default 50).
    #[must_use]
    pub fn limit(mut self, limit: u8) -> Self {
        self.limit = Some(limit.clamp(1, 50));
        self
    }

    /// Send the request.
    pub async fn send(self) -> Result<ChannelPins, HttpError> {
        let mut req = self.client.get(Route::raw(
            "/channels/{channel_id}/messages/pins",
            self.channel_id,
        ));
        if let Some(before) = self.before {
            req = req.query("before", before);
        }
        if let Some(limit) = self.limit {
            req = req.query("limit", limit);
        }
        req.send_json().await
    }
}

pub(crate) async fn send_message(
    client: Client,
    method: Method,
    route: Route,
    mut body: MessageBody,
    files: Vec<File>,
    reason: Option<String>,
    require_body: bool,
) -> Result<Message, HttpError> {
    client.fill_allowed_mentions(&mut body.allowed_mentions);
    if let Some(components) = &body.components {
        super::mark_components_v2(&mut body.flags, components);
    }

    if !files.is_empty() {
        body.attachments = Some(
            files
                .iter()
                .enumerate()
                .map(|(i, file)| PartialAttachment {
                    id: i as u64,
                    filename: file.filename.clone(),
                    description: file.description.clone(),
                })
                .collect(),
        );
    }

    let is_forward = body
        .message_reference
        .as_ref()
        .is_some_and(|r| r.kind == MessageReferenceType::Forward);
    if require_body
        && !is_forward
        && body.content.as_ref().is_none_or(String::is_empty)
        && body.embeds.as_ref().is_none_or(Vec::is_empty)
        && body.sticker_ids.is_empty()
        && body.components.as_ref().is_none_or(Vec::is_empty)
        && files.is_empty()
    {
        return Err(HttpError::validation(
            "message needs content, an embed, a sticker, components, a file or a forward reference",
        ));
    }

    let mut req = client.request(method, route);
    if let Some(reason) = reason {
        req = req.reason(&reason)?;
    }
    if files.is_empty() {
        req = req.json(&body)?;
    } else {
        let payload = serde_json::to_vec(&body)
            .map_err(|e| HttpError::validation(format!("json body: {e}").into_boxed_str()))?;
        req = req.multipart(&Multipart {
            payload_json: payload.into(),
            files,
        })?;
    }
    req.send_json().await
}

impl IntoFuture for CreateMessage {
    type Output = Result<Message, HttpError>;
    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output> + Send>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(self.send())
    }
}

impl IntoFuture for EditMessage {
    type Output = Result<Message, HttpError>;
    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output> + Send>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(self.send())
    }
}

impl IntoFuture for GetChannelMessages {
    type Output = Result<Vec<Message>, HttpError>;
    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output> + Send>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(self.send())
    }
}

impl IntoFuture for DeleteMessage {
    type Output = Result<(), HttpError>;
    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output> + Send>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(self.send())
    }
}

impl IntoFuture for DeleteMessages {
    type Output = Result<(), HttpError>;
    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output> + Send>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(self.send())
    }
}

impl IntoFuture for PinMessage {
    type Output = Result<(), HttpError>;
    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output> + Send>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(self.send())
    }
}

impl IntoFuture for UnpinMessage {
    type Output = Result<(), HttpError>;
    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output> + Send>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(self.send())
    }
}

impl IntoFuture for GetChannelPins {
    type Output = Result<ChannelPins, HttpError>;
    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output> + Send>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(self.send())
    }
}

query_request! {
    /// `GET /channels/{channel_id}/messages/{message_id}/reactions/{emoji}`.
    GetReactions -> Vec<cordex_model::user::User> {
        /// Users after this user ID.
        after: Id<cordex_model::id::marker::UserMarker>,
        /// Max. users (1-100, default 25).
        limit: u8,
    }
}

impl GetReactions {
    /// Only super-reactions (burst) instead of normal ones.
    pub fn burst(mut self) -> Self {
        self.query.push(("type", "1".to_owned()));
        self
    }
}

impl Client {
    simple! {
        /// `POST /channels/{channel_id}/messages/{message_id}/crosspost`: publish
        /// a message in an announcement channel to its followers.
        fn crosspost_message(channel_id: Id<ChannelMarker>, message_id: Id<MessageMarker>)
            -> Message = post "/channels/{channel_id}/messages/{message_id}/crosspost" => send_json
    }

    /// `GET /channels/{channel_id}/messages/{message_id}/reactions/{emoji}`:
    /// users who reacted with `emoji` (Unicode or `name:id`).
    pub fn get_reactions(
        &self,
        channel_id: Id<ChannelMarker>,
        message_id: Id<MessageMarker>,
        emoji: impl Into<String>,
    ) -> GetReactions {
        GetReactions::new(
            self,
            Route::raw(
                "/channels/{channel_id}/messages/{message_id}/reactions/{emoji}",
                (channel_id, message_id, emoji.into()),
            ),
        )
    }
}
