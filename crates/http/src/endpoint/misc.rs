//! Guild templates, voice states and regions, invite target users and
//! guild message search.

use std::future::Future;

use cordex_model::channel::Channel;
use cordex_model::guild::GuildTemplate;
use cordex_model::id::Id;
use cordex_model::id::marker::{ChannelMarker, GuildMarker, MessageMarker, RoleMarker, UserMarker};
use cordex_model::message::Message;
use cordex_model::timestamp::Timestamp;
use cordex_model::voice::{VoiceRegion, VoiceState};
use http::{Method, StatusCode};
use serde::Deserialize;

use crate::client::Client;
use crate::error::HttpError;
use crate::multipart::{File, encode_form};
use crate::route::Route;

/// Max. users per bulk target-user request.
pub const MAX_TARGET_USERS_BULK: usize = 1000;

json_request! {
    /// `PATCH /guilds/{guild_id}/templates/{template_code}`.
    UpdateGuildTemplate -> GuildTemplate => send_json {
        /// Name (1-100 characters).
        name: &str,
        /// Description (0-120 characters).
        description: Option<&str>,
    }
}

json_request! {
    /// `PATCH /guilds/{guild_id}/voice-states/@me`.
    UpdateCurrentVoiceState -> () => send_empty {
        /// Stage channel the bot is in.
        channel_id: Id<ChannelMarker>,
        /// Suppress (move to the audience).
        suppress: bool,
        /// Raise hand (`None` lowers it).
        request_to_speak_timestamp: Option<Timestamp>,
    }
}

json_request! {
    /// `PATCH /guilds/{guild_id}/voice-states/{user_id}`.
    UpdateVoiceState -> () => send_empty {
        /// Stage channel the user is in.
        channel_id: Id<ChannelMarker>,
        /// Suppress (move to the audience).
        suppress: bool,
    }
}

/// Status of an asynchronous target-users CSV job.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[non_exhaustive]
pub struct TargetUsersJob {
    /// 0 unspecified, 1 processing, 2 completed, 3 failed.
    pub status: u8,
    /// Users in the file.
    #[serde(default)]
    pub total_users: u64,
    /// Users processed so far.
    #[serde(default)]
    pub processed_users: u64,
    /// When the job started.
    pub created_at: Option<Timestamp>,
    /// When it finished.
    pub completed_at: Option<Timestamp>,
    /// Why it failed.
    pub error_message: Option<Box<str>>,
}

/// Response of `GET /oauth2/@me`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[non_exhaustive]
pub struct AuthorizationInfo {
    /// The application the token was issued for.
    pub application: cordex_model::application::Application,
    /// Granted scopes.
    #[serde(default)]
    pub scopes: Vec<Box<str>>,
    /// When the token expires.
    pub expires: Timestamp,
    /// The user (with the `identify` scope).
    #[serde(default)]
    pub user: Option<cordex_model::user::User>,
}

/// Result of [`Client::search_guild_messages`].
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum SearchResult {
    /// Matches.
    Found(Box<SearchResults>),
    /// The guild is not indexed yet (HTTP 202); retry after the given
    /// seconds (0 = shortly).
    NotIndexed {
        /// Seconds to wait.
        retry_after: f64,
        /// Documents indexed so far.
        documents_indexed: u64,
    },
}

/// Matches of a guild message search.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[non_exhaustive]
pub struct SearchResults {
    /// Total matches (may be inexact while messages change).
    pub total_results: u64,
    /// Matching messages; each inner list holds one hit (with context).
    #[serde(default)]
    pub messages: Vec<Vec<Message>>,
    /// Threads that contain hits.
    #[serde(default)]
    pub threads: Vec<Channel>,
    /// Whether old history is still being indexed.
    #[serde(default)]
    pub doing_deep_historical_index: bool,
}

#[derive(Deserialize)]
struct NotIndexedBody {
    #[serde(default)]
    retry_after: f64,
    #[serde(default)]
    documents_indexed: u64,
}

/// `GET /guilds/{guild_id}/messages/search` with filters.
#[must_use = "send the request with `.await`"]
pub struct SearchGuildMessages {
    client: Client,
    guild_id: Id<GuildMarker>,
    query: Vec<(&'static str, String)>,
}

impl SearchGuildMessages {
    fn push(mut self, key: &'static str, value: String) -> Self {
        self.query.push((key, value));
        self
    }

    /// Text to search for (max. 1024 characters).
    pub fn content(self, text: &str) -> Self {
        self.push("content", text.to_string())
    }

    /// Max. results (1-25, default 25).
    pub fn limit(self, limit: u8) -> Self {
        self.push("limit", limit.to_string())
    }

    /// Skip results (max. 9975).
    pub fn offset(self, offset: u16) -> Self {
        self.push("offset", offset.to_string())
    }

    /// Only messages before this ID.
    pub fn max_id(self, id: Id<MessageMarker>) -> Self {
        self.push("max_id", id.to_string())
    }

    /// Only messages after this ID.
    pub fn min_id(self, id: Id<MessageMarker>) -> Self {
        self.push("min_id", id.to_string())
    }

    /// Only in these channels (repeat for several).
    pub fn channel_id(self, id: Id<ChannelMarker>) -> Self {
        self.push("channel_id", id.to_string())
    }

    /// Only by these authors.
    pub fn author_id(self, id: Id<UserMarker>) -> Self {
        self.push("author_id", id.to_string())
    }

    /// Only `user`, `bot` or `webhook` authors.
    pub fn author_type(self, kind: &str) -> Self {
        self.push("author_type", kind.to_string())
    }

    /// Only messages mentioning this user.
    pub fn mentions(self, id: Id<UserMarker>) -> Self {
        self.push("mentions", id.to_string())
    }

    /// Only messages mentioning this role.
    pub fn mentions_role_id(self, id: Id<RoleMarker>) -> Self {
        self.push("mentions_role_id", id.to_string())
    }

    /// Only messages that have `image`, `video`, `file`, `link`, `embed`,
    /// `poll`, … (repeat for several).
    pub fn has(self, kind: &str) -> Self {
        self.push("has", kind.to_string())
    }

    /// Only pinned (or unpinned) messages.
    pub fn pinned(self, pinned: bool) -> Self {
        self.push("pinned", pinned.to_string())
    }

    /// Sort by `timestamp` (default) or `relevance`.
    pub fn sort_by(self, by: &str) -> Self {
        self.push("sort_by", by.to_string())
    }

    /// `asc` or `desc` (default).
    pub fn sort_order(self, order: &str) -> Self {
        self.push("sort_order", order.to_string())
    }

    /// Include age-restricted channels.
    pub fn include_nsfw(self, include: bool) -> Self {
        self.push("include_nsfw", include.to_string())
    }

    /// Any other documented filter (`slop`, `embed_type`, `link_hostname`,
    /// …).
    pub fn param(self, key: &'static str, value: &impl ToString) -> Self {
        self.push(key, value.to_string())
    }
}

impl std::future::IntoFuture for SearchGuildMessages {
    type Output = Result<SearchResult, HttpError>;
    type IntoFuture = std::pin::Pin<Box<dyn Future<Output = Self::Output> + Send>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(async move {
            let mut req = self.client.get(Route::raw(
                "/guilds/{guild_id}/messages/search",
                self.guild_id,
            ));
            for (key, value) in &self.query {
                req = req.query(key, value);
            }
            let response = req.send().await?;
            if response.status() == StatusCode::ACCEPTED {
                let body: NotIndexedBody = response.json()?;
                return Ok(SearchResult::NotIndexed {
                    retry_after: body.retry_after,
                    documents_indexed: body.documents_indexed,
                });
            }
            Ok(SearchResult::Found(Box::new(response.json()?)))
        })
    }
}

/// Parse the `user_id` CSV of `GET /invites/{code}/target-users`.
fn parse_target_users(csv: &[u8]) -> Vec<Id<UserMarker>> {
    String::from_utf8_lossy(csv)
        .lines()
        .filter_map(|line| line.trim().parse().ok())
        .collect()
}

fn check_bulk(user_ids: &[Id<UserMarker>]) -> Result<(), HttpError> {
    if user_ids.is_empty() || user_ids.len() > MAX_TARGET_USERS_BULK {
        return Err(HttpError::Validation(
            "bulk target-user requests take 1-1000 users".into(),
        ));
    }
    Ok(())
}

impl Client {
    // ---- guild templates ---------------------------------------------------

    simple! {
        /// `GET /guilds/templates/{template_code}`.
        fn get_guild_template(template_code: String)
            -> GuildTemplate = get "/guilds/templates/{template_code}" => send_json
    }

    simple! {
        /// `GET /guilds/{guild_id}/templates`.
        fn get_guild_templates(guild_id: Id<GuildMarker>)
            -> Vec<GuildTemplate> = get "/guilds/{guild_id}/templates" => send_json
    }

    /// `POST /guilds/{guild_id}/templates`: snapshot the guild.
    pub fn create_guild_template(
        &self,
        guild_id: Id<GuildMarker>,
        name: &str,
        description: Option<&str>,
    ) -> impl Future<Output = Result<GuildTemplate, HttpError>> + Send + 'static {
        let client = self.clone();
        let body = serde_json::json!({ "name": name, "description": description });
        async move {
            client
                .post(Route::raw("/guilds/{guild_id}/templates", guild_id))
                .json(&body)?
                .send_json()
                .await
        }
    }

    simple! {
        /// `PUT /guilds/{guild_id}/templates/{template_code}`: sync the
        /// template with the current guild.
        fn sync_guild_template(guild_id: Id<GuildMarker>, template_code: String)
            -> GuildTemplate = put "/guilds/{guild_id}/templates/{template_code}" => send_json
    }

    /// `PATCH /guilds/{guild_id}/templates/{template_code}`.
    pub fn update_guild_template(
        &self,
        guild_id: Id<GuildMarker>,
        template_code: &str,
    ) -> UpdateGuildTemplate {
        UpdateGuildTemplate::new(
            self,
            Method::PATCH,
            Route::raw(
                "/guilds/{guild_id}/templates/{template_code}",
                (guild_id, template_code.to_owned()),
            ),
        )
    }

    simple! {
        /// `DELETE /guilds/{guild_id}/templates/{template_code}`.
        fn delete_guild_template(guild_id: Id<GuildMarker>, template_code: String)
            -> GuildTemplate = delete "/guilds/{guild_id}/templates/{template_code}" => send_json
    }

    // ---- voice -------------------------------------------------------------

    simple! {
        /// `GET /voice/regions`.
        fn get_voice_regions() -> Vec<VoiceRegion> = get "/voice/regions" => send_json
    }

    simple! {
        /// `GET /guilds/{guild_id}/voice-states/@me`.
        fn get_current_voice_state(guild_id: Id<GuildMarker>)
            -> VoiceState = get "/guilds/{guild_id}/voice-states/@me" => send_json
    }

    simple! {
        /// `GET /guilds/{guild_id}/voice-states/{user_id}`.
        fn get_voice_state(guild_id: Id<GuildMarker>, user_id: Id<UserMarker>)
            -> VoiceState = get "/guilds/{guild_id}/voice-states/{user_id}" => send_json
    }

    /// `PATCH /guilds/{guild_id}/voice-states/@me` (stage channels).
    pub fn update_current_voice_state(&self, guild_id: Id<GuildMarker>) -> UpdateCurrentVoiceState {
        UpdateCurrentVoiceState::new(
            self,
            Method::PATCH,
            Route::raw("/guilds/{guild_id}/voice-states/@me", guild_id),
        )
    }

    /// `PATCH /guilds/{guild_id}/voice-states/{user_id}` (stage channels).
    pub fn update_voice_state(
        &self,
        guild_id: Id<GuildMarker>,
        user_id: Id<UserMarker>,
    ) -> UpdateVoiceState {
        UpdateVoiceState::new(
            self,
            Method::PATCH,
            Route::raw(
                "/guilds/{guild_id}/voice-states/{user_id}",
                (guild_id, user_id),
            ),
        )
    }

    // ---- invite target users -----------------------------------------------

    /// `GET /invites/{invite_code}/target-users`: users allowed to accept
    /// the invite (Discord answers with a CSV).
    pub fn get_invite_target_users(
        &self,
        invite_code: &str,
    ) -> impl Future<Output = Result<Vec<Id<UserMarker>>, HttpError>> + Send + 'static {
        let client = self.clone();
        let route = Route::raw(
            "/invites/{invite_code}/target-users",
            invite_code.to_owned(),
        );
        async move {
            let response = client.get(route).send().await?;
            Ok(parse_target_users(response.body()))
        }
    }

    /// `PUT /invites/{invite_code}/target-users`: replace the allowed users
    /// (sent as a CSV file).
    pub fn update_invite_target_users(
        &self,
        invite_code: &str,
        user_ids: &[Id<UserMarker>],
    ) -> impl Future<Output = Result<(), HttpError>> + Send + 'static {
        let client = self.clone();
        let route = Route::raw(
            "/invites/{invite_code}/target-users",
            invite_code.to_owned(),
        );
        let mut csv = String::from("user_id\n");
        for id in user_ids {
            csv.push_str(&id.to_string());
            csv.push('\n');
        }
        let file = File::bytes("target_users.csv", csv.into_bytes()).content_type("text/csv");
        let body = encode_form(&[], "target_users_file", &file);
        async move { client.put(route).encoded(body?).send_empty().await }
    }

    simple! {
        /// `PUT /invites/{invite_code}/target-users/{user_id}`.
        fn add_invite_target_user(invite_code: String, user_id: Id<UserMarker>)
            -> () = put "/invites/{invite_code}/target-users/{user_id}" => send_empty
    }

    simple! {
        /// `DELETE /invites/{invite_code}/target-users/{user_id}`.
        fn remove_invite_target_user(invite_code: String, user_id: Id<UserMarker>)
            -> () = delete "/invites/{invite_code}/target-users/{user_id}" => send_empty
    }

    /// `POST /invites/{invite_code}/target-users/bulk-add` (max. 1000).
    pub fn bulk_add_invite_target_users(
        &self,
        invite_code: &str,
        user_ids: &[Id<UserMarker>],
    ) -> impl Future<Output = Result<(), HttpError>> + Send + 'static {
        self.bulk_target_users(
            "/invites/{invite_code}/target-users/bulk-add",
            invite_code,
            user_ids,
        )
    }

    /// `POST /invites/{invite_code}/target-users/bulk-delete` (max. 1000).
    pub fn bulk_remove_invite_target_users(
        &self,
        invite_code: &str,
        user_ids: &[Id<UserMarker>],
    ) -> impl Future<Output = Result<(), HttpError>> + Send + 'static {
        self.bulk_target_users(
            "/invites/{invite_code}/target-users/bulk-delete",
            invite_code,
            user_ids,
        )
    }

    fn bulk_target_users(
        &self,
        template: &'static str,
        invite_code: &str,
        user_ids: &[Id<UserMarker>],
    ) -> impl Future<Output = Result<(), HttpError>> + Send + 'static {
        let client = self.clone();
        let route = Route::raw(template, invite_code.to_owned());
        let check = check_bulk(user_ids);
        let body = serde_json::json!({ "user_ids": user_ids });
        async move {
            check?;
            client.post(route).json(&body)?.send_empty().await
        }
    }

    simple! {
        /// `GET /invites/{invite_code}/target-users/job-status`.
        fn get_invite_target_users_job(invite_code: String)
            -> TargetUsersJob = get "/invites/{invite_code}/target-users/job-status" => send_json
    }

    // ---- oauth2 ------------------------------------------------------------

    simple! {
        /// `GET /oauth2/applications/@me`: the bot's application.
        fn get_current_bot_application()
            -> cordex_model::application::Application = get "/oauth2/applications/@me" => send_json
    }

    simple! {
        /// `GET /oauth2/@me`: who the Bearer token belongs to and its scopes.
        fn get_current_authorization()
            -> AuthorizationInfo = get "/oauth2/@me" => send_json
    }

    // ---- message search ----------------------------------------------------

    /// `GET /guilds/{guild_id}/messages/search`. Needs `READ_MESSAGE_HISTORY`
    /// (and the `MESSAGE_CONTENT` intent for content). Returns
    /// [`SearchResult::NotIndexed`] while Discord is still indexing.
    pub fn search_guild_messages(&self, guild_id: Id<GuildMarker>) -> SearchGuildMessages {
        SearchGuildMessages {
            client: self.clone(),
            guild_id,
            query: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_user_csv_parses() {
        let ids = parse_target_users(b"user_id\n1\n 2 \n\nnot-an-id\n3\r\n");
        let raw: Vec<u64> = ids.iter().map(|id| id.get()).collect();
        assert_eq!(raw, vec![1, 2, 3]);
    }
}
