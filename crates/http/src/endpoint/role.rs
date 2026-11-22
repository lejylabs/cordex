//! Guild role mutators.

use std::future::{Future, IntoFuture};
use std::pin::Pin;

use drs_model::guild::Role;
use drs_model::id::Id;
use drs_model::id::marker::{GuildMarker, RoleMarker};
use drs_model::permissions::Permissions;
use serde::Serialize;

use crate::client::Client;
use crate::error::HttpError;
use crate::route::Route;

impl Client {
    /// `POST /guilds/{guild_id}/roles`.
    pub fn create_role(&self, guild_id: Id<GuildMarker>) -> CreateRole {
        CreateRole {
            client: self.clone(),
            guild_id,
            body: RoleBody::default(),
            reason: None,
        }
    }

    /// `PATCH /guilds/{guild_id}/roles/{role_id}`.
    pub fn update_role(&self, guild_id: Id<GuildMarker>, role_id: Id<RoleMarker>) -> UpdateRole {
        UpdateRole {
            client: self.clone(),
            guild_id,
            role_id,
            body: RoleBody::default(),
            reason: None,
        }
    }

    /// `DELETE /guilds/{guild_id}/roles/{role_id}`.
    pub fn delete_role(&self, guild_id: Id<GuildMarker>, role_id: Id<RoleMarker>) -> DeleteRole {
        DeleteRole {
            client: self.clone(),
            guild_id,
            role_id,
            reason: None,
        }
    }

    /// `PATCH /guilds/{guild_id}/roles`.
    pub fn update_role_positions(
        &self,
        guild_id: Id<GuildMarker>,
        positions: impl IntoIterator<Item = (Id<RoleMarker>, i32)>,
    ) -> impl Future<Output = Result<Vec<Role>, HttpError>> + Send + 'static {
        let client = self.clone();
        let positions: Vec<RolePosition> = positions
            .into_iter()
            .map(|(id, position)| RolePosition { id, position })
            .collect();
        async move {
            client
                .patch(Route::raw("/guilds/{guild_id}/roles", guild_id))
                .json(&positions)?
                .send_json()
                .await
        }
    }
}

#[derive(Serialize)]
struct RolePosition {
    id: Id<RoleMarker>,
    position: i32,
}

#[derive(Serialize, Default)]
struct RoleBody {
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    permissions: Option<Permissions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    color: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    hoist: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mentionable: Option<bool>,
}

macro_rules! role_setters {
    ($ty:ty) => {
        impl $ty {
            /// Role name.
            #[must_use]
            pub fn name(mut self, name: impl Into<String>) -> Self {
                self.body.name = Some(name.into());
                self
            }

            /// Role permissions.
            #[must_use]
            pub fn permissions(mut self, permissions: Permissions) -> Self {
                self.body.permissions = Some(permissions);
                self
            }

            /// Deprecated single RGB color.
            #[must_use]
            pub fn color(mut self, color: u32) -> Self {
                self.body.color = Some(color);
                self
            }

            /// Show separately in the member list.
            #[must_use]
            pub fn hoist(mut self, hoist: bool) -> Self {
                self.body.hoist = Some(hoist);
                self
            }

            /// Mentionable by everyone.
            #[must_use]
            pub fn mentionable(mut self, mentionable: bool) -> Self {
                self.body.mentionable = Some(mentionable);
                self
            }

            /// Audit-log reason.
            pub fn reason(mut self, reason: impl Into<String>) -> Result<Self, HttpError> {
                super::set_reason(&mut self.reason, reason)?;
                Ok(self)
            }
        }
    };
}

/// `POST /guilds/{guild_id}/roles`.
pub struct CreateRole {
    client: Client,
    guild_id: Id<GuildMarker>,
    body: RoleBody,
    reason: Option<String>,
}

role_setters!(CreateRole);

impl CreateRole {
    /// Send the request.
    pub async fn send(self) -> Result<Role, HttpError> {
        self.client
            .post(Route::raw("/guilds/{guild_id}/roles", self.guild_id))
            .maybe_reason(self.reason.as_deref())?
            .json(&self.body)?
            .send_json()
            .await
    }
}

impl IntoFuture for CreateRole {
    type Output = Result<Role, HttpError>;
    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output> + Send>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(self.send())
    }
}

/// `PATCH /guilds/{guild_id}/roles/{role_id}`.
pub struct UpdateRole {
    client: Client,
    guild_id: Id<GuildMarker>,
    role_id: Id<RoleMarker>,
    body: RoleBody,
    reason: Option<String>,
}

role_setters!(UpdateRole);

impl UpdateRole {
    /// Send the request.
    pub async fn send(self) -> Result<Role, HttpError> {
        self.client
            .patch(Route::raw(
                "/guilds/{guild_id}/roles/{role_id}",
                (self.guild_id, self.role_id),
            ))
            .maybe_reason(self.reason.as_deref())?
            .json(&self.body)?
            .send_json()
            .await
    }
}

impl IntoFuture for UpdateRole {
    type Output = Result<Role, HttpError>;
    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output> + Send>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(self.send())
    }
}

/// `DELETE /guilds/{guild_id}/roles/{role_id}`.
pub struct DeleteRole {
    client: Client,
    guild_id: Id<GuildMarker>,
    role_id: Id<RoleMarker>,
    reason: Option<String>,
}

impl DeleteRole {
    /// Audit-log reason.
    pub fn reason(mut self, reason: impl Into<String>) -> Result<Self, HttpError> {
        super::set_reason(&mut self.reason, reason)?;
        Ok(self)
    }

    /// Send the request.
    pub async fn send(self) -> Result<(), HttpError> {
        self.client
            .delete(Route::raw(
                "/guilds/{guild_id}/roles/{role_id}",
                (self.guild_id, self.role_id),
            ))
            .maybe_reason(self.reason.as_deref())?
            .send_empty()
            .await
    }
}

impl IntoFuture for DeleteRole {
    type Output = Result<(), HttpError>;
    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output> + Send>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(self.send())
    }
}
