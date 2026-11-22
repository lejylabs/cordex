//! Guild endpoints.

use std::future::Future;

use drs_model::channel::Channel;
use drs_model::guild::{Guild, Member, Role};
use drs_model::id::Id;
use drs_model::id::marker::{GuildMarker, UserMarker};
use drs_model::invite::Invite;

use crate::client::Client;
use crate::error::HttpError;
use crate::route::Route;

impl Client {
    /// `GET /guilds/{guild_id}`.
    pub fn get_guild(
        &self,
        guild_id: Id<GuildMarker>,
    ) -> impl Future<Output = Result<Guild, HttpError>> + Send + 'static {
        let client = self.clone();
        async move {
            client
                .get(Route::raw("/guilds/{guild_id}", guild_id))
                .query("with_counts", "true")
                .send_json()
                .await
        }
    }

    /// `GET /guilds/{guild_id}/channels`.
    pub fn get_guild_channels(
        &self,
        guild_id: Id<GuildMarker>,
    ) -> impl Future<Output = Result<Vec<Channel>, HttpError>> + Send + 'static {
        let client = self.clone();
        async move {
            client
                .get(Route::raw("/guilds/{guild_id}/channels", guild_id))
                .send_json()
                .await
        }
    }

    /// `GET /guilds/{guild_id}/members/{user_id}`.
    pub fn get_guild_member(
        &self,
        guild_id: Id<GuildMarker>,
        user_id: Id<UserMarker>,
    ) -> impl Future<Output = Result<Member, HttpError>> + Send + 'static {
        let client = self.clone();
        async move {
            client
                .get(Route::raw(
                    "/guilds/{guild_id}/members/{user_id}",
                    (guild_id, user_id),
                ))
                .send_json()
                .await
        }
    }

    /// `GET /guilds/{guild_id}/roles`.
    pub fn get_guild_roles(
        &self,
        guild_id: Id<GuildMarker>,
    ) -> impl Future<Output = Result<Vec<Role>, HttpError>> + Send + 'static {
        let client = self.clone();
        async move {
            client
                .get(Route::raw("/guilds/{guild_id}/roles", guild_id))
                .send_json()
                .await
        }
    }

    /// `GET /guilds/{guild_id}/invites`.
    pub fn get_guild_invites(
        &self,
        guild_id: Id<GuildMarker>,
    ) -> impl Future<Output = Result<Vec<Invite>, HttpError>> + Send + 'static {
        let client = self.clone();
        async move {
            client
                .get(Route::raw("/guilds/{guild_id}/invites", guild_id))
                .send_json()
                .await
        }
    }
}
