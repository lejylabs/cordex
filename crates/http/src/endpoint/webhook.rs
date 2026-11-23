//! Webhook execute, CRUD and webhook messages.

use std::future::{Future, IntoFuture};
use std::pin::Pin;

use cordex_model::id::Id;
use cordex_model::id::marker::{ChannelMarker, MessageMarker, WebhookMarker};
use cordex_model::message::{AllowedMentions, Embed, Message, MessageFlags};
use cordex_model::token::WebhookToken;
use cordex_model::webhook::Webhook;
use serde::Serialize;

use crate::client::Client;
use crate::error::HttpError;
use crate::multipart::{File, Multipart};
use crate::route::Route;
use crate::validate::{FILE_COUNT_LIMIT, FILE_SIZE_LIMIT, content_len, embed_limits};

/// `POST /webhooks/{webhook_id}/{webhook_token}`.
pub struct ExecuteWebhook {
    client: Client,
    webhook_id: Id<WebhookMarker>,
    token: WebhookToken,
    wait: bool,
    thread_id: Option<Id<ChannelMarker>>,
    body: WebhookBody,
    files: Vec<File>,
}

#[derive(Serialize, Default)]
struct WebhookBody {
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    avatar_url: Option<String>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    tts: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    embeds: Vec<Embed>,
    #[serde(skip_serializing_if = "Option::is_none")]
    allowed_mentions: Option<AllowedMentions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    flags: Option<MessageFlags>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    components: Vec<cordex_model::component::Component>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    attachments: Vec<WebhookAttachment>,
}

#[derive(Serialize)]
struct WebhookAttachment {
    id: u64,
    filename: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
}

impl Client {
    /// Execute a webhook (`POST /webhooks/{webhook_id}/{webhook_token}`). `wait` defaults to `true` so Discord returns the message.
    pub fn execute_webhook(
        &self,
        webhook_id: Id<WebhookMarker>,
        token: &WebhookToken,
    ) -> ExecuteWebhook {
        ExecuteWebhook {
            client: self.clone(),
            webhook_id,
            token: token.clone(),
            wait: true,
            thread_id: None,
            body: WebhookBody::default(),
            files: Vec::new(),
        }
    }

    /// `POST /channels/{channel_id}/webhooks`.
    pub fn create_webhook(
        &self,
        channel_id: Id<ChannelMarker>,
        name: impl Into<String>,
    ) -> CreateWebhook {
        CreateWebhook {
            client: self.clone(),
            channel_id,
            name: name.into(),
            reason: None,
        }
    }

    /// `GET /channels/{channel_id}/webhooks`.
    pub fn get_channel_webhooks(
        &self,
        channel_id: Id<ChannelMarker>,
    ) -> impl Future<Output = Result<Vec<Webhook>, HttpError>> + Send + 'static {
        let client = self.clone();
        async move {
            client
                .get(Route::raw("/channels/{channel_id}/webhooks", channel_id))
                .send_json()
                .await
        }
    }

    /// `GET /webhooks/{webhook_id}`.
    pub fn get_webhook(
        &self,
        webhook_id: Id<WebhookMarker>,
    ) -> impl Future<Output = Result<Webhook, HttpError>> + Send + 'static {
        let client = self.clone();
        async move {
            client
                .get(Route::raw("/webhooks/{webhook_id}", webhook_id))
                .send_json()
                .await
        }
    }

    /// `GET /webhooks/{webhook_id}/{webhook_token}`.
    pub fn get_webhook_with_token(
        &self,
        webhook_id: Id<WebhookMarker>,
        token: &WebhookToken,
    ) -> impl Future<Output = Result<Webhook, HttpError>> + Send + 'static {
        let client = self.clone();
        let token = token.expose().to_owned();
        async move {
            client
                .get(Route::raw(
                    "/webhooks/{webhook_id}/{webhook_token}",
                    (webhook_id, token),
                ))
                .send_json()
                .await
        }
    }

    /// `DELETE /webhooks/{webhook_id}`.
    pub fn delete_webhook(&self, webhook_id: Id<WebhookMarker>) -> DeleteWebhook {
        DeleteWebhook {
            client: self.clone(),
            webhook_id,
            token: None,
            reason: None,
        }
    }

    /// `DELETE /webhooks/{webhook_id}/{webhook_token}`.
    pub fn delete_webhook_with_token(
        &self,
        webhook_id: Id<WebhookMarker>,
        token: &WebhookToken,
    ) -> DeleteWebhook {
        DeleteWebhook {
            client: self.clone(),
            webhook_id,
            token: Some(token.clone()),
            reason: None,
        }
    }

    /// `GET /webhooks/{webhook_id}/{webhook_token}/messages/{message_id}`.
    pub fn get_webhook_message(
        &self,
        webhook_id: Id<WebhookMarker>,
        token: &WebhookToken,
        message_id: Id<MessageMarker>,
    ) -> impl Future<Output = Result<Message, HttpError>> + Send + 'static {
        let client = self.clone();
        let token = token.expose().to_owned();
        async move {
            client
                .get(Route::raw(
                    "/webhooks/{webhook_id}/{webhook_token}/messages/{message_id}",
                    (webhook_id, token, message_id),
                ))
                .send_json()
                .await
        }
    }

    /// `PATCH /webhooks/{webhook_id}/{webhook_token}/messages/{message_id}`.
    pub fn update_webhook_message(
        &self,
        webhook_id: Id<WebhookMarker>,
        token: &WebhookToken,
        message_id: Id<MessageMarker>,
    ) -> UpdateWebhookMessage {
        UpdateWebhookMessage {
            client: self.clone(),
            webhook_id,
            token: token.clone(),
            message_id,
            body: WebhookBody::default(),
            files: Vec::new(),
        }
    }

    /// `DELETE /webhooks/{webhook_id}/{webhook_token}/messages/{message_id}`.
    pub fn delete_webhook_message(
        &self,
        webhook_id: Id<WebhookMarker>,
        token: &WebhookToken,
        message_id: Id<MessageMarker>,
    ) -> impl Future<Output = Result<(), HttpError>> + Send + 'static {
        let client = self.clone();
        let token = token.expose().to_owned();
        async move {
            client
                .delete(Route::raw(
                    "/webhooks/{webhook_id}/{webhook_token}/messages/{message_id}",
                    (webhook_id, token, message_id),
                ))
                .send_empty()
                .await
        }
    }
}

impl ExecuteWebhook {
    /// When `false`, Discord returns 204 and no message body.
    #[must_use]
    pub fn wait(mut self, wait: bool) -> Self {
        self.wait = wait;
        self
    }

    /// Post into a thread of the webhook's channel.
    #[must_use]
    pub fn thread_id(mut self, thread_id: Id<ChannelMarker>) -> Self {
        self.thread_id = Some(thread_id);
        self
    }

    /// Message content (max. 2000 characters).
    pub fn content(mut self, content: impl Into<String>) -> Result<Self, HttpError> {
        let content = content.into();
        content_len(&content)?;
        self.body.content = Some(content);
        Ok(self)
    }

    /// Override the webhook user name for this post.
    #[must_use]
    pub fn username(mut self, name: impl Into<String>) -> Self {
        self.body.username = Some(name.into());
        self
    }

    /// Override the webhook avatar URL for this post.
    #[must_use]
    pub fn avatar_url(mut self, url: impl Into<String>) -> Self {
        self.body.avatar_url = Some(url.into());
        self
    }

    /// One embed.
    pub fn embed(mut self, embed: Embed) -> Result<Self, HttpError> {
        self.body.embeds.push(embed);
        embed_limits(&self.body.embeds)?;
        Ok(self)
    }

    /// Components. Webhooks not owned by an application can only send
    /// non-interactive ones (link buttons, Components V2 layout); the
    /// `IS_COMPONENTS_V2` flag is set automatically when needed.
    #[must_use]
    pub fn components(mut self, components: Vec<cordex_model::component::Component>) -> Self {
        self.body.components = components;
        self
    }

    /// Allowed mentions.
    #[must_use]
    pub fn allowed_mentions(mut self, mentions: AllowedMentions) -> Self {
        self.body.allowed_mentions = Some(mentions);
        self
    }

    /// Message flags.
    #[must_use]
    pub fn flags(mut self, flags: MessageFlags) -> Self {
        self.body.flags = Some(flags);
        self
    }

    /// Attach a file.
    pub fn attach(mut self, file: File) -> Result<Self, HttpError> {
        attach_file(&mut self.files, file)?;
        Ok(self)
    }

    /// Send the request. Returns `None` when `wait` is `false`.
    pub async fn send(self) -> Result<Option<Message>, HttpError> {
        let mut body = self.body;
        self.client
            .fill_allowed_mentions(&mut body.allowed_mentions);
        super::mark_components_v2(&mut body.flags, &body.components);
        fill_attachments(&mut body.attachments, &self.files);

        let route = Route::raw(
            "/webhooks/{webhook_id}/{webhook_token}",
            (self.webhook_id, self.token.expose()),
        );
        let mut req = self.client.post(route);
        req = req.query("wait", self.wait);
        if !body.components.is_empty() {
            // Needed for webhooks that are not owned by an application.
            req = req.query("with_components", true);
        }
        if let Some(thread_id) = self.thread_id {
            req = req.query("thread_id", thread_id);
        }
        req = with_body(req, &body, self.files)?;

        if self.wait {
            req.send_json().await.map(Some)
        } else {
            req.send_empty().await.map(|()| None)
        }
    }
}

impl IntoFuture for ExecuteWebhook {
    type Output = Result<Option<Message>, HttpError>;
    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output> + Send>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(self.send())
    }
}

#[derive(Serialize)]
struct CreateWebhookBody<'a> {
    name: &'a str,
}

/// `POST /channels/{channel_id}/webhooks`.
pub struct CreateWebhook {
    client: Client,
    channel_id: Id<ChannelMarker>,
    name: String,
    reason: Option<String>,
}

impl CreateWebhook {
    /// Audit-log reason.
    pub fn reason(mut self, reason: impl Into<String>) -> Result<Self, HttpError> {
        super::set_reason(&mut self.reason, reason)?;
        Ok(self)
    }

    /// Send the request.
    pub async fn send(self) -> Result<Webhook, HttpError> {
        let n = self.name.chars().count();
        if !(1..=80).contains(&n) {
            return Err(HttpError::validation(
                format!("webhook name is {n} characters; must be 1–80").into_boxed_str(),
            ));
        }
        self.client
            .post(Route::raw(
                "/channels/{channel_id}/webhooks",
                self.channel_id,
            ))
            .maybe_reason(self.reason.as_deref())?
            .json(&CreateWebhookBody { name: &self.name })?
            .send_json()
            .await
    }
}

impl IntoFuture for CreateWebhook {
    type Output = Result<Webhook, HttpError>;
    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output> + Send>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(self.send())
    }
}

/// Delete a webhook, with or without a token.
pub struct DeleteWebhook {
    client: Client,
    webhook_id: Id<WebhookMarker>,
    token: Option<WebhookToken>,
    reason: Option<String>,
}

impl DeleteWebhook {
    /// Audit-log reason.
    pub fn reason(mut self, reason: impl Into<String>) -> Result<Self, HttpError> {
        super::set_reason(&mut self.reason, reason)?;
        Ok(self)
    }

    /// Send the request.
    pub async fn send(self) -> Result<(), HttpError> {
        let route = match self.token {
            Some(token) => Route::raw(
                "/webhooks/{webhook_id}/{webhook_token}",
                (self.webhook_id, token.expose()),
            ),
            None => Route::raw("/webhooks/{webhook_id}", self.webhook_id),
        };
        self.client
            .delete(route)
            .maybe_reason(self.reason.as_deref())?
            .send_empty()
            .await
    }
}

impl IntoFuture for DeleteWebhook {
    type Output = Result<(), HttpError>;
    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output> + Send>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(self.send())
    }
}

/// `PATCH /webhooks/{webhook_id}/{webhook_token}/messages/{message_id}`.
pub struct UpdateWebhookMessage {
    client: Client,
    webhook_id: Id<WebhookMarker>,
    token: WebhookToken,
    message_id: Id<MessageMarker>,
    body: WebhookBody,
    files: Vec<File>,
}

impl UpdateWebhookMessage {
    /// Message content.
    pub fn content(mut self, content: impl Into<String>) -> Result<Self, HttpError> {
        let content = content.into();
        content_len(&content)?;
        self.body.content = Some(content);
        Ok(self)
    }

    /// One embed.
    pub fn embed(mut self, embed: Embed) -> Result<Self, HttpError> {
        self.body.embeds.push(embed);
        embed_limits(&self.body.embeds)?;
        Ok(self)
    }

    /// Attach a file.
    pub fn attach(mut self, file: File) -> Result<Self, HttpError> {
        attach_file(&mut self.files, file)?;
        Ok(self)
    }

    /// Send the request.
    pub async fn send(self) -> Result<Message, HttpError> {
        let mut body = self.body;
        fill_attachments(&mut body.attachments, &self.files);
        let route = Route::raw(
            "/webhooks/{webhook_id}/{webhook_token}/messages/{message_id}",
            (self.webhook_id, self.token.expose(), self.message_id),
        );
        with_body(self.client.patch(route), &body, self.files)?
            .send_json()
            .await
    }
}

impl IntoFuture for UpdateWebhookMessage {
    type Output = Result<Message, HttpError>;
    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output> + Send>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(self.send())
    }
}

fn attach_file(files: &mut Vec<File>, file: File) -> Result<(), HttpError> {
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
    if files.len() >= FILE_COUNT_LIMIT {
        return Err(HttpError::validation(
            format!("a message can have at most {FILE_COUNT_LIMIT} files").into_boxed_str(),
        ));
    }
    files.push(file);
    Ok(())
}

fn fill_attachments(attachments: &mut Vec<WebhookAttachment>, files: &[File]) {
    if files.is_empty() {
        return;
    }
    *attachments = files
        .iter()
        .enumerate()
        .map(|(i, file)| WebhookAttachment {
            id: i as u64,
            filename: file.filename.clone(),
            description: file.description.clone(),
        })
        .collect();
}

fn with_body(
    req: crate::RequestBuilder,
    body: &WebhookBody,
    files: Vec<File>,
) -> Result<crate::RequestBuilder, HttpError> {
    if files.is_empty() {
        req.json(body)
    } else {
        let payload = serde_json::to_vec(body)
            .map_err(|e| HttpError::validation(format!("json body: {e}").into_boxed_str()))?;
        req.multipart(&Multipart {
            payload_json: payload.into(),
            files,
        })
    }
}

/// `PATCH /webhooks/{webhook_id}` (or with its token): change a webhook.
#[must_use = "send the request with `.await`"]
pub struct UpdateWebhook {
    client: Client,
    route: Route,
    body: serde_json::Map<String, serde_json::Value>,
    reason: Option<String>,
    with_token: bool,
}

impl UpdateWebhook {
    /// New default name.
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.body.insert("name".into(), name.into().into());
        self
    }

    /// New avatar as a data URI; `None` removes it.
    pub fn avatar(mut self, data_uri: Option<String>) -> Self {
        self.body.insert("avatar".into(), data_uri.into());
        self
    }

    /// Move to another channel (not possible with the token variant).
    pub fn channel_id(mut self, channel_id: Id<ChannelMarker>) -> Self {
        self.body
            .insert("channel_id".into(), channel_id.to_string().into());
        self
    }

    /// Audit-log reason.
    pub fn reason(mut self, reason: impl Into<String>) -> Result<Self, HttpError> {
        super::set_reason(&mut self.reason, reason)?;
        Ok(self)
    }
}

impl std::future::IntoFuture for UpdateWebhook {
    type Output = Result<Webhook, HttpError>;
    type IntoFuture = std::pin::Pin<Box<dyn std::future::Future<Output = Self::Output> + Send>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(async move {
            if self.with_token && self.body.contains_key("channel_id") {
                return Err(HttpError::Validation(
                    "a webhook cannot change its channel with the token variant".into(),
                ));
            }
            let mut req = self.client.patch(self.route).json(&self.body)?;
            if let Some(reason) = &self.reason {
                req = req.reason(reason)?;
            }
            req.send_json().await
        })
    }
}

impl Client {
    simple! {
        /// `GET /guilds/{guild_id}/webhooks`. Needs `MANAGE_WEBHOOKS`.
        fn get_guild_webhooks(guild_id: Id<cordex_model::id::marker::GuildMarker>)
            -> Vec<Webhook> = get "/guilds/{guild_id}/webhooks" => send_json
    }

    /// `PATCH /webhooks/{webhook_id}`.
    pub fn update_webhook(&self, webhook_id: Id<WebhookMarker>) -> UpdateWebhook {
        UpdateWebhook {
            client: self.clone(),
            route: Route::raw("/webhooks/{webhook_id}", webhook_id),
            body: serde_json::Map::new(),
            reason: None,
            with_token: false,
        }
    }

    /// `PATCH /webhooks/{webhook_id}/{webhook_token}` (no bot token needed).
    pub fn update_webhook_with_token(
        &self,
        webhook_id: Id<WebhookMarker>,
        token: &cordex_model::token::WebhookToken,
    ) -> UpdateWebhook {
        UpdateWebhook {
            client: self.clone(),
            route: Route::raw(
                "/webhooks/{webhook_id}/{webhook_token}",
                (webhook_id, token.expose().to_owned()),
            ),
            body: serde_json::Map::new(),
            reason: None,
            with_token: true,
        }
    }

    /// `POST /webhooks/{webhook_id}/{webhook_token}/slack`: execute with a
    /// Slack-formatted body (sent as is).
    pub fn execute_slack_webhook(
        &self,
        webhook_id: Id<WebhookMarker>,
        token: &cordex_model::token::WebhookToken,
        body: serde_json::Value,
    ) -> impl std::future::Future<Output = Result<(), crate::error::HttpError>> + Send + 'static
    {
        self.execute_compat(
            webhook_id,
            token,
            "/webhooks/{webhook_id}/{webhook_token}/slack",
            body,
        )
    }

    /// `POST /webhooks/{webhook_id}/{webhook_token}/github`: execute with a
    /// GitHub event body (sent as is).
    pub fn execute_github_webhook(
        &self,
        webhook_id: Id<WebhookMarker>,
        token: &cordex_model::token::WebhookToken,
        body: serde_json::Value,
    ) -> impl std::future::Future<Output = Result<(), crate::error::HttpError>> + Send + 'static
    {
        self.execute_compat(
            webhook_id,
            token,
            "/webhooks/{webhook_id}/{webhook_token}/github",
            body,
        )
    }

    fn execute_compat(
        &self,
        webhook_id: Id<WebhookMarker>,
        token: &cordex_model::token::WebhookToken,
        template: &'static str,
        body: serde_json::Value,
    ) -> impl std::future::Future<Output = Result<(), crate::error::HttpError>> + Send + 'static
    {
        let client = self.clone();
        let route = Route::raw(template, (webhook_id, token.expose().to_owned()));
        async move { client.post(route).json(&body)?.send_empty().await }
    }
}
