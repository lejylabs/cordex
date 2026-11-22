//! Invite endpoints.

use std::future::{Future, IntoFuture};
use std::pin::Pin;

use cordex_model::id::Id;
use cordex_model::id::marker::ChannelMarker;
use cordex_model::invite::Invite;
use serde::Serialize;

use crate::client::Client;
use crate::error::HttpError;
use crate::route::Route;

impl Client {
    /// `GET /invites/{invite_code}`.
    pub fn get_invite(
        &self,
        code: impl Into<String>,
    ) -> impl Future<Output = Result<Invite, HttpError>> + Send + 'static {
        let client = self.clone();
        let code = code.into();
        async move {
            client
                .get(Route::raw("/invites/{invite_code}", code))
                .query("with_counts", "true")
                .query("with_expiration", "true")
                .send_json()
                .await
        }
    }

    /// `DELETE /invites/{invite_code}`.
    pub fn delete_invite(&self, code: impl Into<String>) -> DeleteInvite {
        DeleteInvite {
            client: self.clone(),
            code: code.into(),
            reason: None,
        }
    }

    /// `POST /channels/{channel_id}/invites`.
    pub fn create_channel_invite(&self, channel_id: Id<ChannelMarker>) -> CreateChannelInvite {
        CreateChannelInvite {
            client: self.clone(),
            channel_id,
            body: CreateInviteBody::default(),
            reason: None,
        }
    }
}

/// `DELETE /invites/{invite_code}`.
pub struct DeleteInvite {
    client: Client,
    code: String,
    reason: Option<String>,
}

impl DeleteInvite {
    /// Audit-log reason.
    pub fn reason(mut self, reason: impl Into<String>) -> Result<Self, HttpError> {
        super::set_reason(&mut self.reason, reason)?;
        Ok(self)
    }

    /// Send the request.
    pub async fn send(self) -> Result<Invite, HttpError> {
        self.client
            .delete(Route::raw("/invites/{invite_code}", self.code))
            .maybe_reason(self.reason.as_deref())?
            .send_json()
            .await
    }
}

impl IntoFuture for DeleteInvite {
    type Output = Result<Invite, HttpError>;
    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output> + Send>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(self.send())
    }
}

#[derive(Serialize, Default)]
struct CreateInviteBody {
    #[serde(skip_serializing_if = "Option::is_none")]
    max_age: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_uses: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temporary: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    unique: Option<bool>,
}

/// `POST /channels/{channel_id}/invites`.
pub struct CreateChannelInvite {
    client: Client,
    channel_id: Id<ChannelMarker>,
    body: CreateInviteBody,
    reason: Option<String>,
}

impl CreateChannelInvite {
    /// Lifetime in seconds (`0` = never).
    #[must_use]
    pub fn max_age(mut self, seconds: u32) -> Self {
        self.body.max_age = Some(seconds);
        self
    }

    /// Max uses (`0` = unlimited).
    #[must_use]
    pub fn max_uses(mut self, uses: u8) -> Self {
        self.body.max_uses = Some(uses);
        self
    }

    /// Kick the member when they go offline (temporary membership).
    #[must_use]
    pub fn temporary(mut self, temporary: bool) -> Self {
        self.body.temporary = Some(temporary);
        self
    }

    /// Create a unique invite that is never reused.
    #[must_use]
    pub fn unique(mut self, unique: bool) -> Self {
        self.body.unique = Some(unique);
        self
    }

    /// Audit-log reason.
    pub fn reason(mut self, reason: impl Into<String>) -> Result<Self, HttpError> {
        super::set_reason(&mut self.reason, reason)?;
        Ok(self)
    }

    /// Send the request.
    pub async fn send(self) -> Result<Invite, HttpError> {
        self.client
            .post(Route::raw(
                "/channels/{channel_id}/invites",
                self.channel_id,
            ))
            .maybe_reason(self.reason.as_deref())?
            .json(&self.body)?
            .send_json()
            .await
    }
}

impl IntoFuture for CreateChannelInvite {
    type Output = Result<Invite, HttpError>;
    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output> + Send>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(self.send())
    }
}
