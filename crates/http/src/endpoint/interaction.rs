//! Interaction callback and follow-ups.

use std::future::{Future, IntoFuture};
use std::pin::Pin;

use cordex_model::component::Component;
use cordex_model::id::Id;
use cordex_model::id::marker::{ApplicationMarker, InteractionMarker};
use cordex_model::interaction::{
    InteractionCallbackResponse, InteractionCallbackType, InteractionResponse,
};
use cordex_model::message::{AllowedMentions, Embed, Message, MessageFlags};
use cordex_model::token::InteractionToken;
use serde::Serialize;

use crate::client::Client;
use crate::error::HttpError;
use crate::multipart::{File, Multipart};
use crate::route::Route;
use crate::validate::{content_len, embed_limits};

impl Client {
    /// `POST /interactions/{interaction_id}/{interaction_token}/callback`.
    ///
    /// Exempt from the global 50 rps limiter.
    pub fn create_interaction_response(
        &self,
        interaction_id: Id<InteractionMarker>,
        token: &InteractionToken,
        response: &InteractionResponse,
    ) -> impl Future<Output = Result<(), HttpError>> + Send + 'static {
        let client = self.clone();
        let token = token.expose().to_owned();
        let mut response = response.clone();
        apply_default_mentions(&client, &mut response);
        async move {
            client
                .post(Route::raw(
                    "/interactions/{interaction_id}/{interaction_token}/callback",
                    (interaction_id, token),
                ))
                .json(&response)?
                .send_empty()
                .await
        }
    }

    /// Like [`Self::create_interaction_response`] with `?with_response=true`;
    /// returns the callback object (e.g. the created message).
    /// `POST /interactions/{interaction_id}/{interaction_token}/callback`.
    pub fn create_interaction_response_with_response(
        &self,
        interaction_id: Id<InteractionMarker>,
        token: &InteractionToken,
        response: &InteractionResponse,
    ) -> impl Future<Output = Result<InteractionCallbackResponse, HttpError>> + Send + 'static {
        let client = self.clone();
        let token = token.expose().to_owned();
        let mut response = response.clone();
        apply_default_mentions(&client, &mut response);
        async move {
            client
                .post(Route::raw(
                    "/interactions/{interaction_id}/{interaction_token}/callback",
                    (interaction_id, token),
                ))
                .query("with_response", "true")
                .json(&response)?
                .send_json()
                .await
        }
    }

    /// `PATCH /webhooks/{application_id}/{interaction_token}/messages/@original`.
    pub fn update_interaction_response(
        &self,
        application_id: Id<ApplicationMarker>,
        token: &InteractionToken,
    ) -> UpdateInteractionResponse {
        UpdateInteractionResponse {
            client: self.clone(),
            application_id,
            token: token.expose().to_owned(),
            body: FollowupBody::default(),
            files: Vec::new(),
        }
    }

    /// `DELETE /webhooks/{application_id}/{interaction_token}/messages/@original`.
    pub fn delete_interaction_response(
        &self,
        application_id: Id<ApplicationMarker>,
        token: &InteractionToken,
    ) -> impl Future<Output = Result<(), HttpError>> + Send + 'static {
        let client = self.clone();
        let token = token.expose().to_owned();
        async move {
            client
                .delete(
                    Route::raw(
                        "/webhooks/{webhook_id}/{webhook_token}/messages/@original",
                        (application_id, token),
                    )
                    .interaction(),
                )
                .send_empty()
                .await
        }
    }

    /// `POST /webhooks/{application_id}/{interaction_token}`.
    pub fn create_followup_message(
        &self,
        application_id: Id<ApplicationMarker>,
        token: &InteractionToken,
    ) -> CreateFollowupMessage {
        CreateFollowupMessage {
            client: self.clone(),
            application_id,
            token: token.expose().to_owned(),
            body: FollowupBody::default(),
            files: Vec::new(),
        }
    }
}

#[derive(Serialize, Default)]
struct FollowupBody {
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    embeds: Option<Vec<Embed>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    allowed_mentions: Option<AllowedMentions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    flags: Option<MessageFlags>,
    #[serde(skip_serializing_if = "Option::is_none")]
    components: Option<Vec<Component>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    attachments: Option<Vec<FollowupAttachment>>,
}

#[derive(Serialize)]
struct FollowupAttachment {
    id: u64,
    filename: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
}

/// Edit the original interaction response.
pub struct UpdateInteractionResponse {
    client: Client,
    application_id: Id<ApplicationMarker>,
    token: String,
    body: FollowupBody,
    files: Vec<File>,
}

/// Create a follow-up message.
pub struct CreateFollowupMessage {
    client: Client,
    application_id: Id<ApplicationMarker>,
    token: String,
    body: FollowupBody,
    files: Vec<File>,
}

macro_rules! followup_setters {
    ($ty:ty) => {
        impl $ty {
            /// Message content (max. 2000 characters).
            pub fn content(mut self, content: impl Into<String>) -> Result<Self, HttpError> {
                let content = content.into();
                content_len(&content)?;
                self.body.content = Some(content);
                Ok(self)
            }

            /// One embed.
            pub fn embed(mut self, embed: Embed) -> Result<Self, HttpError> {
                let embeds = self.body.embeds.get_or_insert_with(Vec::new);
                embeds.push(embed);
                embed_limits(embeds)?;
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

            /// Message flags (`EPHEMERAL`, …).
            #[must_use]
            pub fn flags(mut self, flags: MessageFlags) -> Self {
                self.body.flags = Some(flags);
                self
            }

            /// Message components (an empty list removes them on edit).
            #[must_use]
            pub fn components(mut self, components: Vec<Component>) -> Self {
                self.body.components = Some(components);
                self
            }

            /// Attach a file.
            pub fn attach(mut self, file: File) -> Result<Self, HttpError> {
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
        }
    };
}

followup_setters!(UpdateInteractionResponse);
followup_setters!(CreateFollowupMessage);

impl UpdateInteractionResponse {
    /// Send the request.
    pub async fn send(self) -> Result<Message, HttpError> {
        let route = Route::raw(
            "/webhooks/{webhook_id}/{webhook_token}/messages/@original",
            (self.application_id, self.token),
        )
        .interaction();
        let client = self.client.clone();
        followup_send(client.clone(), client.patch(route), self.body, self.files).await
    }
}

impl CreateFollowupMessage {
    /// Send the request.
    pub async fn send(self) -> Result<Message, HttpError> {
        let route = Route::raw(
            "/webhooks/{webhook_id}/{webhook_token}",
            (self.application_id, self.token),
        )
        .interaction();
        let client = self.client.clone();
        followup_send(client.clone(), client.post(route), self.body, self.files).await
    }
}

fn apply_default_mentions(client: &Client, response: &mut InteractionResponse) {
    let Some(data) = &mut response.data else {
        return;
    };
    if !matches!(
        response.kind,
        InteractionCallbackType::ChannelMessageWithSource | InteractionCallbackType::UpdateMessage
    ) {
        return;
    }
    client.fill_allowed_mentions(&mut data.allowed_mentions);
    if cordex_model::component::needs_components_v2(&data.components) {
        data.flags |= cordex_model::message::MessageFlags::IS_COMPONENTS_V2;
    }
}

fn fill_followup_attachments(body: &mut FollowupBody, files: &[File]) {
    if files.is_empty() {
        return;
    }
    body.attachments = Some(
        files
            .iter()
            .enumerate()
            .map(|(i, file)| FollowupAttachment {
                id: i as u64,
                filename: file.filename.clone(),
                description: file.description.clone(),
            })
            .collect(),
    );
}

async fn followup_send(
    client: Client,
    req: crate::RequestBuilder,
    mut body: FollowupBody,
    files: Vec<File>,
) -> Result<Message, HttpError> {
    client.fill_allowed_mentions(&mut body.allowed_mentions);
    if let Some(components) = &body.components {
        super::mark_components_v2(&mut body.flags, components);
    }
    fill_followup_attachments(&mut body, &files);
    let req = if files.is_empty() {
        req.json(&body)?
    } else {
        let payload = serde_json::to_vec(&body)
            .map_err(|e| HttpError::validation(format!("json body: {e}").into_boxed_str()))?;
        req.multipart(&Multipart {
            payload_json: payload.into(),
            files,
        })?
    };
    req.send_json().await
}

impl IntoFuture for UpdateInteractionResponse {
    type Output = Result<Message, HttpError>;
    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output> + Send>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(self.send())
    }
}

impl IntoFuture for CreateFollowupMessage {
    type Output = Result<Message, HttpError>;
    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output> + Send>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(self.send())
    }
}

impl Client {
    /// `GET /webhooks/{application_id}/{interaction_token}/messages/@original`:
    /// the original interaction response.
    pub fn get_original_interaction_response(
        &self,
        application_id: cordex_model::id::Id<cordex_model::id::marker::ApplicationMarker>,
        token: &cordex_model::token::InteractionToken,
    ) -> impl std::future::Future<
        Output = Result<cordex_model::message::Message, crate::error::HttpError>,
    > + Send
    + 'static {
        let client = self.clone();
        let token = token.expose().to_owned();
        async move {
            client
                .get(
                    crate::route::Route::raw(
                        "/webhooks/{application_id}/{interaction_token}/messages/@original",
                        (application_id, token),
                    )
                    .interaction(),
                )
                .send_json()
                .await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn followup_body_fills_attachments_from_files() {
        let mut body = FollowupBody::default();
        let files = vec![File::bytes("captcha.png", b"png".to_vec())];
        fill_followup_attachments(&mut body, &files);
        let json = serde_json::to_value(&body).unwrap();
        assert_eq!(
            json["attachments"],
            serde_json::json!([{"id": 0, "filename": "captcha.png"}])
        );
    }

    #[test]
    fn followup_body_clear_attachments_empty_content_and_components() {
        let body = FollowupBody {
            content: Some(String::new()),
            components: Some(Vec::new()),
            attachments: Some(Vec::new()),
            ..FollowupBody::default()
        };
        let json = serde_json::to_value(&body).unwrap();
        assert_eq!(json["content"], serde_json::json!(""));
        assert_eq!(json["components"], serde_json::json!([]));
        assert_eq!(json["attachments"], serde_json::json!([]));
    }
}
