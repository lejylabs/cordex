//! User endpoints.

use std::future::Future;

use cordex_model::channel::Channel;
use cordex_model::gateway::GatewayBot;
use cordex_model::id::Id;
use cordex_model::id::marker::{GuildMarker, UserMarker};
use cordex_model::user::User;
use serde::Serialize;

use crate::client::Client;
use crate::error::HttpError;
use crate::route::Route;

#[derive(Serialize)]
struct CreateDmBody {
    recipient_id: Id<UserMarker>,
}

impl Client {
    /// `GET /gateway/bot`.
    pub fn gateway_bot(
        &self,
    ) -> impl Future<Output = Result<GatewayBot, HttpError>> + Send + 'static {
        let client = self.clone();
        async move { client.get(Route::new("/gateway/bot")).send_json().await }
    }

    /// `GET /users/@me`.
    pub fn current_user(&self) -> impl Future<Output = Result<User, HttpError>> + Send + 'static {
        let client = self.clone();
        async move { client.get(Route::new("/users/@me")).send_json().await }
    }

    /// `GET /users/{user_id}`.
    pub fn user(
        &self,
        user_id: Id<UserMarker>,
    ) -> impl Future<Output = Result<User, HttpError>> + Send + 'static {
        let client = self.clone();
        async move {
            client
                .get(Route::raw("/users/{user_id}", user_id))
                .send_json()
                .await
        }
    }

    /// `POST /users/@me/channels` — open a DM.
    pub fn create_dm(
        &self,
        recipient_id: Id<UserMarker>,
    ) -> impl Future<Output = Result<Channel, HttpError>> + Send + 'static {
        let client = self.clone();
        async move {
            client
                .post(Route::new("/users/@me/channels"))
                .json(&CreateDmBody { recipient_id })?
                .send_json()
                .await
        }
    }

    /// `DELETE /users/@me/guilds/{guild_id}`.
    pub fn leave_guild(
        &self,
        guild_id: Id<GuildMarker>,
    ) -> impl Future<Output = Result<(), HttpError>> + Send + 'static {
        let client = self.clone();
        async move {
            client
                .delete(Route::raw("/users/@me/guilds/{guild_id}", guild_id))
                .send_empty()
                .await
        }
    }
}

query_request! {
    /// `GET /users/@me/guilds`.
    GetCurrentUserGuilds -> Vec<cordex_model::guild::UserGuild> {
        /// Guilds before this guild ID.
        before: Id<GuildMarker>,
        /// Guilds after this guild ID.
        after: Id<GuildMarker>,
        /// Max. guilds (1-200, default 200).
        limit: u8,
        /// Include approximate member and presence counts.
        with_counts: bool,
    }
}

/// `PATCH /users/@me`: change the bot's username, avatar or banner.
#[must_use = "send the request with `.await`"]
pub struct UpdateCurrentUser {
    client: Client,
    body: serde_json::Map<String, serde_json::Value>,
}

impl UpdateCurrentUser {
    /// New username.
    pub fn username(mut self, name: impl Into<String>) -> Self {
        self.body.insert("username".into(), name.into().into());
        self
    }

    /// New avatar as a data URI (`data:image/png;base64,...`); `None`
    /// removes it.
    pub fn avatar(mut self, data_uri: Option<String>) -> Self {
        self.body.insert("avatar".into(), data_uri.into());
        self
    }

    /// New banner as a data URI; `None` removes it.
    pub fn banner(mut self, data_uri: Option<String>) -> Self {
        self.body.insert("banner".into(), data_uri.into());
        self
    }
}

impl std::future::IntoFuture for UpdateCurrentUser {
    type Output = Result<User, HttpError>;
    type IntoFuture = std::pin::Pin<Box<dyn Future<Output = Self::Output> + Send>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(async move {
            self.client
                .patch(Route::raw("/users/@me", ()))
                .json(&self.body)?
                .send_json()
                .await
        })
    }
}

impl Client {
    /// `PATCH /users/@me`.
    pub fn update_current_user(&self) -> UpdateCurrentUser {
        UpdateCurrentUser {
            client: self.clone(),
            body: serde_json::Map::new(),
        }
    }

    /// `GET /users/@me/guilds`.
    pub fn get_current_user_guilds(&self) -> GetCurrentUserGuilds {
        GetCurrentUserGuilds::new(self, Route::raw("/users/@me/guilds", ()))
    }

    simple! {
        /// `GET /users/@me/guilds/{guild_id}/member` (Bearer token with the
        /// `guilds.members.read` scope).
        fn get_current_user_guild_member(guild_id: Id<GuildMarker>)
            -> cordex_model::guild::Member = get "/users/@me/guilds/{guild_id}/member" => send_json
    }

    simple! {
        /// `GET /gateway`: the gateway URL (no authentication needed).
        fn get_gateway() -> GatewayUrl = get "/gateway" => send_json
    }

    simple! {
        /// `GET /users/@me/connections` (Bearer token with the `connections`
        /// scope).
        fn get_current_user_connections()
            -> Vec<cordex_model::user::Connection> = get "/users/@me/connections" => send_json
    }

    simple! {
        /// `GET /users/@me/applications/{application_id}/role-connection`
        /// (Bearer token with the `role_connections.write` scope).
        fn get_current_user_role_connection(application_id: Id<cordex_model::id::marker::ApplicationMarker>)
            -> cordex_model::user::ApplicationRoleConnection
            = get "/users/@me/applications/{application_id}/role-connection" => send_json
    }

    simple! {
        /// `DELETE /users/@me/applications/{application_id}/role-connection`.
        fn delete_current_user_role_connection(application_id: Id<cordex_model::id::marker::ApplicationMarker>)
            -> () = delete "/users/@me/applications/{application_id}/role-connection" => send_empty
    }

    /// `PUT /users/@me/applications/{application_id}/role-connection`
    /// (Bearer token with the `role_connections.write` scope).
    pub fn update_current_user_role_connection(
        &self,
        application_id: Id<cordex_model::id::marker::ApplicationMarker>,
        connection: &cordex_model::user::ApplicationRoleConnection,
    ) -> impl std::future::Future<
        Output = Result<cordex_model::user::ApplicationRoleConnection, crate::error::HttpError>,
    > + Send
    + 'static {
        let client = self.clone();
        let body = serde_json::to_value(connection);
        async move {
            let body = body.map_err(|e| HttpError::Validation(e.to_string().into()))?;
            client
                .put(Route::raw(
                    "/users/@me/applications/{application_id}/role-connection",
                    application_id,
                ))
                .json(&body)?
                .send_json()
                .await
        }
    }
}

/// Response of `GET /gateway`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[non_exhaustive]
pub struct GatewayUrl {
    /// WebSocket URL.
    pub url: Box<str>,
}
