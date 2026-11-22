//! Entitlement list.

use std::future::{Future, IntoFuture};
use std::pin::Pin;

use drs_model::entitlement::Entitlement;
use drs_model::id::Id;
use drs_model::id::marker::{
    ApplicationMarker, EntitlementMarker, GuildMarker, SkuMarker, UserMarker,
};

use crate::client::Client;
use crate::error::HttpError;
use crate::route::Route;

impl Client {
    /// `GET /applications/{application_id}/entitlements`.
    pub fn get_entitlements(&self, application_id: Id<ApplicationMarker>) -> GetEntitlements {
        GetEntitlements {
            client: self.clone(),
            application_id,
            user_id: None,
            sku_ids: Vec::new(),
            before: None,
            after: None,
            limit: None,
            guild_id: None,
            exclude_ended: None,
        }
    }
}

/// `GET /applications/{application_id}/entitlements`.
pub struct GetEntitlements {
    client: Client,
    application_id: Id<ApplicationMarker>,
    user_id: Option<Id<UserMarker>>,
    sku_ids: Vec<Id<SkuMarker>>,
    before: Option<Id<EntitlementMarker>>,
    after: Option<Id<EntitlementMarker>>,
    limit: Option<u8>,
    guild_id: Option<Id<GuildMarker>>,
    exclude_ended: Option<bool>,
}

impl GetEntitlements {
    /// Filter by user.
    #[must_use]
    pub fn user_id(mut self, user_id: Id<UserMarker>) -> Self {
        self.user_id = Some(user_id);
        self
    }

    /// Filter by SKUs.
    #[must_use]
    pub fn sku_ids(mut self, sku_ids: Vec<Id<SkuMarker>>) -> Self {
        self.sku_ids = sku_ids;
        self
    }

    /// Page before this entitlement ID.
    #[must_use]
    pub fn before(mut self, id: Id<EntitlementMarker>) -> Self {
        self.before = Some(id);
        self
    }

    /// Page after this entitlement ID.
    #[must_use]
    pub fn after(mut self, id: Id<EntitlementMarker>) -> Self {
        self.after = Some(id);
        self
    }

    /// Page size (1–100, Discord default 100).
    #[must_use]
    pub fn limit(mut self, limit: u8) -> Self {
        self.limit = Some(limit.clamp(1, 100));
        self
    }

    /// Filter by guild.
    #[must_use]
    pub fn guild_id(mut self, guild_id: Id<GuildMarker>) -> Self {
        self.guild_id = Some(guild_id);
        self
    }

    /// Omit ended entitlements.
    #[must_use]
    pub fn exclude_ended(mut self, exclude: bool) -> Self {
        self.exclude_ended = Some(exclude);
        self
    }

    /// Send the request.
    pub async fn send(self) -> Result<Vec<Entitlement>, HttpError> {
        let mut req = self.client.get(Route::raw(
            "/applications/{application_id}/entitlements",
            self.application_id,
        ));
        if let Some(user_id) = self.user_id {
            req = req.query("user_id", user_id);
        }
        if !self.sku_ids.is_empty() {
            let joined = self
                .sku_ids
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(",");
            req = req.query("sku_ids", joined);
        }
        if let Some(id) = self.before {
            req = req.query("before", id);
        }
        if let Some(id) = self.after {
            req = req.query("after", id);
        }
        if let Some(limit) = self.limit {
            req = req.query("limit", limit);
        }
        if let Some(guild_id) = self.guild_id {
            req = req.query("guild_id", guild_id);
        }
        if let Some(exclude) = self.exclude_ended {
            req = req.query("exclude_ended", exclude);
        }
        req.send_json().await
    }
}

impl IntoFuture for GetEntitlements {
    type Output = Result<Vec<Entitlement>, HttpError>;
    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output> + Send>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(self.send())
    }
}
