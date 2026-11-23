//! Computed permissions of members and roles (guild and channel level).

use cordex_model::id::Id;
use cordex_model::id::marker::{ChannelMarker, GuildMarker, RoleMarker, UserMarker};
use cordex_model::permissions::Permissions;
use serde::Deserialize;

use crate::client::Client;
use crate::route::Route;

#[derive(Deserialize)]
struct PermissionsResponse {
    permissions: Permissions,
}

query_request! {
    /// Computed permissions; see the `Client` method that returns it.
    /// `implicit(false)` turns off implicit masking (missing `VIEW_CHANNEL`
    /// or `CONNECT`).
    GetPermissions -> PermissionsOnly {
        /// Apply implicit permission masking (default `true`).
        implicit: bool,
    }
}

/// Wrapper so `GetPermissions` resolves to plain [`Permissions`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PermissionsOnly(pub Permissions);

impl<'de> Deserialize<'de> for PermissionsOnly {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        PermissionsResponse::deserialize(d).map(|r| Self(r.permissions))
    }
}

impl Client {
    /// `GET /channels/{channel_id}/permissions/member/{user_id}`: the
    /// member's computed permissions in the channel.
    pub fn get_channel_member_permissions(
        &self,
        channel_id: Id<ChannelMarker>,
        user_id: Id<UserMarker>,
    ) -> GetPermissions {
        GetPermissions::new(
            self,
            Route::raw(
                "/channels/{channel_id}/permissions/member/{user_id}",
                (channel_id, user_id),
            ),
        )
    }

    /// `GET /channels/{channel_id}/permissions/role/{role_id}`: what the
    /// role together with `@everyone` grants in the channel.
    pub fn get_channel_role_permissions(
        &self,
        channel_id: Id<ChannelMarker>,
        role_id: Id<RoleMarker>,
    ) -> GetPermissions {
        GetPermissions::new(
            self,
            Route::raw(
                "/channels/{channel_id}/permissions/role/{role_id}",
                (channel_id, role_id),
            ),
        )
    }

    /// `GET /guilds/{guild_id}/permissions/member/{user_id}`: the member's
    /// guild-level permissions.
    pub fn get_guild_member_permissions(
        &self,
        guild_id: Id<GuildMarker>,
        user_id: Id<UserMarker>,
    ) -> GetPermissions {
        GetPermissions::new(
            self,
            Route::raw(
                "/guilds/{guild_id}/permissions/member/{user_id}",
                (guild_id, user_id),
            ),
        )
    }

    /// `GET /guilds/{guild_id}/permissions/role/{role_id}`: what the role
    /// together with `@everyone` grants guild-wide.
    pub fn get_guild_role_permissions(
        &self,
        guild_id: Id<GuildMarker>,
        role_id: Id<RoleMarker>,
    ) -> GetPermissions {
        GetPermissions::new(
            self,
            Route::raw(
                "/guilds/{guild_id}/permissions/role/{role_id}",
                (guild_id, role_id),
            ),
        )
    }
}
