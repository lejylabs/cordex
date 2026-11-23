//! Interaction callback and follow-ups.

use std::future::{Future, IntoFuture};
use std::pin::Pin;

use cordex_model::component::Component;
use cordex_model::id::Id;
use cordex_model::id::marker::{ApplicationMarker, InteractionMarker};
use cordex_model::interaction::{InteractionCallbackResponse, InteractionResponse};
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
        let response = response.clone();
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
        let response = response.clone();
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
    #[serde(skip_serializing_if = "Vec::is_empty")]
    embeds: Vec<Embed>,
    #[serde(skip_serializing_if = "Option::is_none")]
    allowed_mentions: Option<AllowedMentions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    flags: Option<MessageFlags>,
    #[serde(skip_serializing_if = "Option::is_none")]
    components: Option<Vec<Component>>,
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
                self.body.embeds.push(embed);
                embed_limits(&self.body.embeds)?;
                Ok(self)
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
        followup_send(self.client.patch(route), &self.body, self.files).await
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
        followup_send(self.client.post(route), &self.body, self.files).await
    }
}

async fn followup_send(
    req: crate::RequestBuilder,
    body: &FollowupBody,
    files: Vec<File>,
) -> Result<Message, HttpError> {
    let req = if files.is_empty() {
        req.json(body)?
    } else {
        let payload = serde_json::to_vec(body)
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
