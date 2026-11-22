//! User endpoints.

use std::future::Future;

use drs_model::channel::Channel;
use drs_model::id::Id;
use drs_model::id::marker::{GuildMarker, UserMarker};
use drs_model::user::User;
use serde::Serialize;

use crate::client::Client;
use crate::error::HttpError;
use crate::route::Route;

#[derive(Serialize)]
struct CreateDmBody {
    recipient_id: Id<UserMarker>,
}

impl Client {
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
