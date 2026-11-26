//! Discord OAuth2 client for web dashboards (`oauth2` feature, 0 extra deps).
//!
//! Supports authorization URL generation (including PKCE `S256` and bot-invite
//! permissions), code exchange, token refresh/revocation, and typed Bearer
//! endpoints (`current_user`, `current_user_guilds`, `current_user_connections`,
//! `current_user_guild_member`).

#![allow(clippy::doc_markdown)]

use std::fmt;

use bytes::Bytes;
use cordex_model::guild::Member;
use cordex_model::id::{ApplicationId, GuildId};
use cordex_model::image_hash::ImageHash;
use cordex_model::permissions::Permissions;
use cordex_model::user::User;
use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, utf8_percent_encode};

const FORM_ENCODE_SET: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'_')
    .remove(b'.')
    .remove(b'~');
use serde::Deserialize;

use crate::client::Client;
use crate::error::HttpError;
use crate::route::Route;

/// Discord OAuth2 authorization endpoint.
pub const AUTHORIZE_URL: &str = "https://discord.com/oauth2/authorize";

/// OAuth2 application credentials and callback URI.
#[derive(Clone)]
pub struct OAuth2Config {
    /// Discord application / client ID.
    pub client_id: ApplicationId,
    /// OAuth2 client secret (redacted in `Debug`).
    pub client_secret: Box<str>,
    /// Registered redirect URI.
    pub redirect_uri: Box<str>,
}

impl fmt::Debug for OAuth2Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OAuth2Config")
            .field("client_id", &self.client_id)
            .field("client_secret", &"<redacted>")
            .field("redirect_uri", &self.redirect_uri)
            .finish()
    }
}

impl OAuth2Config {
    /// Create a new [`OAuth2Config`].
    pub fn new(
        client_id: ApplicationId,
        client_secret: impl Into<Box<str>>,
        redirect_uri: impl Into<Box<str>>,
    ) -> Self {
        Self {
            client_id,
            client_secret: client_secret.into(),
            redirect_uri: redirect_uri.into(),
        }
    }
}

/// Response from `POST /oauth2/token` (code exchange or token refresh).
#[derive(Clone, Deserialize)]
pub struct TokenResponse {
    /// OAuth2 Bearer access token.
    pub access_token: Box<str>,
    /// Token type (usually `"Bearer"`).
    pub token_type: Box<str>,
    /// Lifetime in seconds.
    pub expires_in: u64,
    /// Refresh token, if granted.
    #[serde(default)]
    pub refresh_token: Option<Box<str>>,
    /// Space-separated granted scopes.
    pub scope: Box<str>,
}

impl fmt::Debug for TokenResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TokenResponse")
            .field("access_token", &"<redacted>")
            .field("token_type", &self.token_type)
            .field("expires_in", &self.expires_in)
            .field(
                "refresh_token",
                &self.refresh_token.as_ref().map(|_| "<redacted>"),
            )
            .field("scope", &self.scope)
            .finish()
    }
}

/// Guild summary returned by `GET /users/@me/guilds`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct PartialGuild {
    /// Guild snowflake ID.
    pub id: GuildId,
    /// Guild name.
    pub name: Box<str>,
    /// Guild icon hash, if set.
    #[serde(default)]
    pub icon: Option<ImageHash>,
    /// Whether the authenticated user is the guild owner.
    #[serde(default)]
    pub owner: bool,
    /// Effective guild permissions for the user.
    #[serde(default)]
    pub permissions: Permissions,
}

impl PartialGuild {
    /// `true` if the user is the guild owner or has `ADMINISTRATOR` / `MANAGE_GUILD`.
    pub fn can_manage(&self) -> bool {
        self.owner
            || self.permissions.contains(Permissions::ADMINISTRATOR)
            || self.permissions.contains(Permissions::MANAGE_GUILD)
    }
}

/// Discord OAuth2 client built on [`Client`].
#[derive(Debug, Clone)]
pub struct OAuth2Client {
    config: OAuth2Config,
    http: Client,
}

impl OAuth2Client {
    /// Create a new [`OAuth2Client`] using a default HTTP client.
    pub fn new(config: OAuth2Config) -> Result<Self, HttpError> {
        let http = Client::builder("unused").build()?;
        Ok(Self { config, http })
    }

    /// Create an [`OAuth2Client`] backed by a custom [`Client`] (e.g. a mock client in tests).
    pub fn with_client(config: OAuth2Config, http: Client) -> Self {
        Self { config, http }
    }

    /// Build a Discord OAuth2 authorization URL.
    ///
    /// Pass `pkce_challenge` (base64url SHA-256) to enable `code_challenge_method=S256`,
    /// and `permissions` when inviting a bot (`scope` includes `"bot"`).
    pub fn authorize_url(
        &self,
        scopes: &[&str],
        state: Option<&str>,
        pkce_challenge: Option<&str>,
        permissions: Option<Permissions>,
    ) -> String {
        let mut params = vec![
            ("client_id", self.config.client_id.get().to_string()),
            ("response_type", "code".to_owned()),
            ("redirect_uri", self.config.redirect_uri.to_string()),
            ("scope", scopes.join(" ")),
        ];
        if let Some(state) = state {
            params.push(("state", state.to_owned()));
        }
        if let Some(challenge) = pkce_challenge {
            params.push(("code_challenge", challenge.to_owned()));
            params.push(("code_challenge_method", "S256".to_owned()));
        }
        if let Some(perms) = permissions {
            params.push(("permissions", perms.bits().to_string()));
        }
        format!("{AUTHORIZE_URL}?{}", encode_form(&params))
    }

    /// Exchange an authorization `code` for an access/refresh token pair.
    pub async fn exchange_code(
        &self,
        code: &str,
        pkce_verifier: Option<&str>,
    ) -> Result<TokenResponse, HttpError> {
        let mut params = vec![
            ("client_id", self.config.client_id.get().to_string()),
            ("client_secret", self.config.client_secret.to_string()),
            ("grant_type", "authorization_code".to_owned()),
            ("code", code.to_owned()),
            ("redirect_uri", self.config.redirect_uri.to_string()),
        ];
        if let Some(verifier) = pkce_verifier {
            params.push(("code_verifier", verifier.to_owned()));
        }
        let body = Bytes::from(encode_form(&params));
        self.http
            .post(Route::raw("/oauth2/token", ()))
            .body(body, "application/x-www-form-urlencoded")
            .send_json()
            .await
    }

    /// Refresh an expired access token using `refresh_token`.
    pub async fn refresh_token(&self, refresh_token: &str) -> Result<TokenResponse, HttpError> {
        let params = [
            ("client_id", self.config.client_id.get().to_string()),
            ("client_secret", self.config.client_secret.to_string()),
            ("grant_type", "refresh_token".to_owned()),
            ("refresh_token", refresh_token.to_owned()),
        ];
        let body = Bytes::from(encode_form(&params));
        self.http
            .post(Route::raw("/oauth2/token", ()))
            .body(body, "application/x-www-form-urlencoded")
            .send_json()
            .await
    }

    /// Revoke an access or refresh token (`POST /oauth2/token/revoke`).
    pub async fn revoke_token(&self, token: &str) -> Result<(), HttpError> {
        let params = [
            ("client_id", self.config.client_id.get().to_string()),
            ("client_secret", self.config.client_secret.to_string()),
            ("token", token.to_owned()),
        ];
        let body = Bytes::from(encode_form(&params));
        self.http
            .post(Route::raw("/oauth2/token/revoke", ()))
            .body(body, "application/x-www-form-urlencoded")
            .send_empty()
            .await
    }

    /// Build a [`Client`] authenticated with a user's OAuth2 `Bearer` token.
    pub fn bearer_client(access_token: &str) -> Result<Client, HttpError> {
        Client::builder(access_token).bearer().build()
    }

    /// Fetch the authenticated user (`GET /users/@me`, requires `identify` scope).
    pub async fn current_user(access_token: &str) -> Result<User, HttpError> {
        let client = Self::bearer_client(access_token)?;
        client.get(Route::raw("/users/@me", ())).send_json().await
    }

    /// Fetch the authenticated user's guilds (`GET /users/@me/guilds`, requires `guilds` scope).
    pub async fn current_user_guilds(access_token: &str) -> Result<Vec<PartialGuild>, HttpError> {
        let client = Self::bearer_client(access_token)?;
        client
            .get(Route::raw("/users/@me/guilds", ()))
            .send_json()
            .await
    }

    /// Fetch the authenticated user's connections (`GET /users/@me/connections`, requires `connections` scope).
    pub async fn current_user_connections(
        access_token: &str,
    ) -> Result<Vec<serde_json::Value>, HttpError> {
        let client = Self::bearer_client(access_token)?;
        client
            .get(Route::raw("/users/@me/connections", ()))
            .send_json()
            .await
    }

    /// Fetch the authenticated user's guild member object (`GET /users/@me/guilds/{guild_id}/member`,
    /// requires `guilds.members.read` scope).
    pub async fn current_user_guild_member(
        access_token: &str,
        guild_id: GuildId,
    ) -> Result<Member, HttpError> {
        let client = Self::bearer_client(access_token)?;
        client
            .get(Route::raw("/users/@me/guilds/{guild_id}/member", guild_id))
            .send_json()
            .await
    }
}

fn encode_form(params: &[(&str, String)]) -> String {
    let mut out = String::new();
    for (idx, (k, v)) in params.iter().enumerate() {
        if idx > 0 {
            out.push('&');
        }
        out.push_str(&utf8_percent_encode(k, FORM_ENCODE_SET).to_string());
        out.push('=');
        out.push_str(&utf8_percent_encode(v, FORM_ENCODE_SET).to_string());
    }
    out
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU64;

    use super::*;
    use crate::mock::{self, MockResponse};

    fn test_config() -> OAuth2Config {
        OAuth2Config::new(
            ApplicationId::new(123_456_789).unwrap_or_else(|| ApplicationId::from(NonZeroU64::MIN)),
            "super-secret",
            "https://example.com/callback",
        )
    }

    #[test]
    fn authorize_url_and_can_manage() {
        let Ok(client) = OAuth2Client::new(test_config()) else {
            panic!("oauth2 client");
        };
        let url = client.authorize_url(
            &["identify", "guilds"],
            Some("csrf123"),
            Some("pkce_challenge"),
            Some(Permissions::MANAGE_GUILD),
        );
        assert!(url.starts_with(AUTHORIZE_URL));
        assert!(url.contains("client_id=123456789"));
        assert!(url.contains("state=csrf123"));
        assert!(url.contains("code_challenge=pkce_challenge"));
        assert!(url.contains("code_challenge_method=S256"));

        let g_owner = PartialGuild {
            id: GuildId::from(NonZeroU64::MIN),
            name: "Owned".into(),
            icon: None,
            owner: true,
            permissions: Permissions::empty(),
        };
        assert!(g_owner.can_manage());

        let g_plain = PartialGuild {
            owner: false,
            permissions: Permissions::SEND_MESSAGES,
            ..g_owner
        };
        assert!(!g_plain.can_manage());
    }

    #[tokio::test]
    async fn exchange_refresh_and_revoke_work_with_mock_client() {
        let Ok(mock_http) = mock::client(|req| {
            let path = req.uri().path();
            if path.ends_with("/oauth2/token") {
                MockResponse::json(
                    r#"{"access_token":"acc","token_type":"Bearer","expires_in":604800,"refresh_token":"ref","scope":"identify"}"#,
                )
            } else {
                MockResponse::no_content()
            }
        }) else {
            panic!("mock client");
        };
        let client = OAuth2Client::with_client(test_config(), mock_http);
        let tok = client.exchange_code("abc", Some("verifier")).await;
        assert!(tok.is_ok());
        let refreshed = client.refresh_token("ref").await;
        assert!(refreshed.is_ok());
        let revoked = client.revoke_token("acc").await;
        assert!(revoked.is_ok());
    }
}
