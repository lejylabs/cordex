//! Application identity profiles (resources/application-identity-profile.mdx).

use std::future::{Future, IntoFuture};
use std::pin::Pin;

use cordex_model::id::Id;
use cordex_model::id::marker::{ApplicationMarker, UserMarker};
use cordex_model::identity_profile::{
    ApplicationIdentities, ApplicationIdentityProfile, ProfileData,
};
use http::{Method, StatusCode};
use serde_json::{Map, Value};

use crate::client::Client;
use crate::error::HttpError;
use crate::route::Route;

query_request! {
    /// `GET /applications/{application_id}/application-identities/{provider_type}/{provider_issued_user_id}`.
    GetApplicationIdentitiesByExternalId -> ApplicationIdentities {
        /// Disambiguates matching provider type + issued user ID.
        provider_id: &str,
    }
}

/// `PATCH /applications/{application_id}/users/{user_id}/identities/{provider_issued_user_id}/profile`.
#[must_use = "send the request with `.await`"]
pub struct UpdateApplicationIdentityProfile {
    client: Client,
    route: Route,
    body: Map<String, Value>,
    error: Option<String>,
}

impl UpdateApplicationIdentityProfile {
    fn new(client: &Client, route: Route) -> Self {
        Self {
            client: client.clone(),
            route,
            body: Map::new(),
            error: None,
        }
    }

    fn set(mut self, key: &'static str, value: impl serde::Serialize) -> Self {
        match serde_json::to_value(value) {
            Ok(value) => {
                self.body.insert(key.to_owned(), value);
            }
            Err(e) => self.error = Some(e.to_string()),
        }
        self
    }

    /// Username in the external system (max 1024 characters).
    pub fn username(self, value: &str) -> Self {
        self.set("username", value)
    }

    /// Full replacement of stored profile data.
    pub fn data(self, value: &ProfileData) -> Self {
        self.set("data", value)
    }

    /// Send: `201` returns the created profile, `204` returns `None`.
    pub async fn send(self) -> Result<Option<ApplicationIdentityProfile>, HttpError> {
        if let Some(error) = self.error {
            return Err(HttpError::Validation(error.into()));
        }
        let resp = self
            .client
            .request(Method::PATCH, self.route)
            .json(&self.body)?
            .send()
            .await?;
        if resp.status() == StatusCode::NO_CONTENT || resp.body().is_empty() {
            return Ok(None);
        }
        resp.json().map(Some)
    }
}

impl IntoFuture for UpdateApplicationIdentityProfile {
    type Output = Result<Option<ApplicationIdentityProfile>, HttpError>;
    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output> + Send>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(self.send())
    }
}

impl Client {
    /// `PATCH /applications/{application_id}/users/{user_id}/identities/{provider_issued_user_id}/profile`.
    pub fn update_application_identity_profile(
        &self,
        application_id: Id<ApplicationMarker>,
        user_id: Id<UserMarker>,
        provider_issued_user_id: impl Into<String>,
    ) -> UpdateApplicationIdentityProfile {
        UpdateApplicationIdentityProfile::new(
            self,
            Route::raw(
                "/applications/{application_id}/users/{user_id}/identities/{provider_issued_user_id}/profile",
                (application_id, user_id, provider_issued_user_id.into()),
            ),
        )
    }

    simple! {
        /// `GET /applications/{application_id}/users/{user_id}/identities/{provider_issued_user_id}/profile`.
        fn get_application_identity_profile(
            application_id: Id<ApplicationMarker>,
            user_id: Id<UserMarker>,
            provider_issued_user_id: String,
        ) -> ApplicationIdentityProfile = get "/applications/{application_id}/users/{user_id}/identities/{provider_issued_user_id}/profile" => send_json
    }

    simple! {
        /// `GET /users/{user_id}/application-identities/{application_id}`.
        fn get_application_identities(
            user_id: Id<UserMarker>,
            application_id: Id<ApplicationMarker>,
        ) -> ApplicationIdentities = get "/users/{user_id}/application-identities/{application_id}" => send_json
    }

    /// `GET /applications/{application_id}/application-identities/{provider_type}/{provider_issued_user_id}`.
    pub fn get_application_identities_by_external_id(
        &self,
        application_id: Id<ApplicationMarker>,
        provider_type: impl Into<String>,
        provider_issued_user_id: impl Into<String>,
    ) -> GetApplicationIdentitiesByExternalId {
        GetApplicationIdentitiesByExternalId::new(
            self,
            Route::raw(
                "/applications/{application_id}/application-identities/{provider_type}/{provider_issued_user_id}",
                (
                    application_id,
                    provider_type.into(),
                    provider_issued_user_id.into(),
                ),
            ),
        )
    }

    /// `POST /users/{user_id}/application-identities/{application_id}/{provider_type}/{provider_issued_user_id}/delete`.
    pub fn delete_application_identity(
        &self,
        user_id: Id<UserMarker>,
        application_id: Id<ApplicationMarker>,
        provider_type: impl Into<String>,
        provider_issued_user_id: impl Into<String>,
        provider_id: Option<&str>,
    ) -> impl Future<Output = Result<(), HttpError>> + Send + 'static {
        let client = self.clone();
        let provider_type = provider_type.into();
        let provider_issued_user_id = provider_issued_user_id.into();
        let provider_id = provider_id.map(ToOwned::to_owned);
        async move {
            let mut req = client.post(Route::raw(
                "/users/{user_id}/application-identities/{application_id}/{provider_type}/{provider_issued_user_id}/delete",
                (user_id, application_id, provider_type, provider_issued_user_id),
            ));
            if let Some(provider_id) = provider_id {
                req = req.json(&serde_json::json!({ "provider_id": provider_id }))?;
            }
            req.send_empty().await
        }
    }
}
