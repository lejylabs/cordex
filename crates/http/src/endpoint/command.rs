//! Application command endpoints.

use std::future::Future;

use cordex_model::command::{ApplicationCommand, CommandOption, CommandType};
use cordex_model::id::Id;
use cordex_model::id::marker::{ApplicationMarker, CommandMarker, GuildMarker};
use cordex_model::permissions::Permissions;
use serde::Serialize;

use crate::client::Client;
use crate::error::HttpError;
use crate::route::Route;

/// A command body the command endpoints accept: [`CommandPayload`] here, or
/// the full builder from the interactions crate.
pub trait CommandBody: Serialize + Send + Sync + 'static {
    /// Local checks before the request is sent; the message names the
    /// problem (e.g. `"command name 'Ping' must be lowercase"`).
    fn check(&self) -> Result<(), Box<str>>;
}

impl CommandBody for CommandPayload {
    fn check(&self) -> Result<(), Box<str>> {
        self.validate()
    }
}

/// Body for creating or bulk-overwriting a command.
#[derive(Clone, Debug, Serialize)]
pub struct CommandPayload {
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<Id<CommandMarker>>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    kind: Option<CommandType>,
    name: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    description: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    options: Vec<CommandOption>,
    #[serde(skip_serializing_if = "Option::is_none")]
    default_member_permissions: Option<Permissions>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    nsfw: bool,
}

impl CommandPayload {
    /// Chat-input command (`CHAT_INPUT`).
    pub fn chat_input(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            id: None,
            kind: Some(CommandType::ChatInput),
            name: name.into(),
            description: description.into(),
            options: Vec::new(),
            default_member_permissions: None,
            nsfw: false,
        }
    }

    /// Context-menu command on a user.
    pub fn user(name: impl Into<String>) -> Self {
        Self {
            id: None,
            kind: Some(CommandType::User),
            name: name.into(),
            description: String::new(),
            options: Vec::new(),
            default_member_permissions: None,
            nsfw: false,
        }
    }

    /// Context-menu command on a message.
    pub fn message(name: impl Into<String>) -> Self {
        Self {
            id: None,
            kind: Some(CommandType::Message),
            name: name.into(),
            description: String::new(),
            options: Vec::new(),
            default_member_permissions: None,
            nsfw: false,
        }
    }

    /// Existing command ID (bulk overwrite keeps the command).
    #[must_use]
    pub fn id(mut self, id: Id<CommandMarker>) -> Self {
        self.id = Some(id);
        self
    }

    /// Command options (slash commands).
    #[must_use]
    pub fn options(mut self, options: Vec<CommandOption>) -> Self {
        self.options = options;
        self
    }

    /// Default member permissions required to use the command.
    #[must_use]
    pub fn default_member_permissions(mut self, permissions: Permissions) -> Self {
        self.default_member_permissions = Some(permissions);
        self
    }

    /// Age-restricted command.
    #[must_use]
    pub fn nsfw(mut self, nsfw: bool) -> Self {
        self.nsfw = nsfw;
        self
    }

    fn validate(&self) -> Result<(), Box<str>> {
        let n = self.name.chars().count();
        if !(1..=32).contains(&n) {
            return Err(format!("command name is {n} characters; must be 1–32").into());
        }
        if self.kind == Some(CommandType::ChatInput) {
            let d = self.description.chars().count();
            if !(1..=100).contains(&d) {
                return Err(format!("command description is {d} characters; must be 1–100").into());
            }
        }
        Ok(())
    }
}

impl Client {
    /// `PUT /applications/{application_id}/commands`.
    pub fn bulk_overwrite_global_commands(
        &self,
        application_id: Id<ApplicationMarker>,
        commands: impl IntoIterator<Item = impl CommandBody>,
    ) -> impl Future<Output = Result<Vec<ApplicationCommand>, HttpError>> + Send + 'static {
        let client = self.clone();
        let route = Route::raw("/applications/{application_id}/commands", application_id);
        let commands: Vec<_> = commands.into_iter().collect();
        async move { put_commands(client, route, commands).await }
    }

    /// `PUT /applications/{application_id}/guilds/{guild_id}/commands`.
    pub fn bulk_overwrite_guild_commands(
        &self,
        application_id: Id<ApplicationMarker>,
        guild_id: Id<GuildMarker>,
        commands: impl IntoIterator<Item = impl CommandBody>,
    ) -> impl Future<Output = Result<Vec<ApplicationCommand>, HttpError>> + Send + 'static {
        let client = self.clone();
        let route = Route::raw(
            "/applications/{application_id}/guilds/{guild_id}/commands",
            (application_id, guild_id),
        );
        let commands: Vec<_> = commands.into_iter().collect();
        async move { put_commands(client, route, commands).await }
    }

    /// `GET /applications/{application_id}/commands`.
    pub fn get_global_commands(
        &self,
        application_id: Id<ApplicationMarker>,
    ) -> impl Future<Output = Result<Vec<ApplicationCommand>, HttpError>> + Send + 'static {
        let client = self.clone();
        async move {
            client
                .get(Route::raw(
                    "/applications/{application_id}/commands",
                    application_id,
                ))
                .send_json()
                .await
        }
    }

    /// `GET /applications/{application_id}/guilds/{guild_id}/commands`.
    pub fn get_guild_commands(
        &self,
        application_id: Id<ApplicationMarker>,
        guild_id: Id<GuildMarker>,
    ) -> impl Future<Output = Result<Vec<ApplicationCommand>, HttpError>> + Send + 'static {
        let client = self.clone();
        async move {
            client
                .get(Route::raw(
                    "/applications/{application_id}/guilds/{guild_id}/commands",
                    (application_id, guild_id),
                ))
                .send_json()
                .await
        }
    }
}

async fn put_commands<C: CommandBody>(
    client: Client,
    route: Route,
    commands: Vec<C>,
) -> Result<Vec<ApplicationCommand>, HttpError> {
    for command in &commands {
        command.check().map_err(HttpError::validation)?;
    }
    client.put(route).json(&commands)?.send_json().await
}

async fn send_command(
    request: crate::RequestBuilder,
    command: impl CommandBody,
) -> Result<ApplicationCommand, HttpError> {
    command.check().map_err(HttpError::validation)?;
    request.json(&command)?.send_json().await
}

impl Client {
    /// `POST /applications/{application_id}/commands` (create or replace by name).
    pub fn create_global_command(
        &self,
        application_id: Id<ApplicationMarker>,
        command: impl CommandBody,
    ) -> impl Future<Output = Result<ApplicationCommand, HttpError>> + Send + 'static {
        let request = self.post(Route::raw(
            "/applications/{application_id}/commands",
            application_id,
        ));
        send_command(request, command)
    }

    /// `POST /applications/{application_id}/guilds/{guild_id}/commands`.
    pub fn create_guild_command(
        &self,
        application_id: Id<ApplicationMarker>,
        guild_id: Id<GuildMarker>,
        command: impl CommandBody,
    ) -> impl Future<Output = Result<ApplicationCommand, HttpError>> + Send + 'static {
        let request = self.post(Route::raw(
            "/applications/{application_id}/guilds/{guild_id}/commands",
            (application_id, guild_id),
        ));
        send_command(request, command)
    }

    /// `PATCH /applications/{application_id}/commands/{command_id}` with the
    /// full new definition.
    pub fn edit_global_command(
        &self,
        application_id: Id<ApplicationMarker>,
        command_id: Id<CommandMarker>,
        command: impl CommandBody,
    ) -> impl Future<Output = Result<ApplicationCommand, HttpError>> + Send + 'static {
        let request = self.patch(Route::raw(
            "/applications/{application_id}/commands/{command_id}",
            (application_id, command_id),
        ));
        send_command(request, command)
    }

    /// `PATCH /applications/{application_id}/guilds/{guild_id}/commands/{command_id}`.
    pub fn edit_guild_command(
        &self,
        application_id: Id<ApplicationMarker>,
        guild_id: Id<GuildMarker>,
        command_id: Id<CommandMarker>,
        command: impl CommandBody,
    ) -> impl Future<Output = Result<ApplicationCommand, HttpError>> + Send + 'static {
        let request = self.patch(Route::raw(
            "/applications/{application_id}/guilds/{guild_id}/commands/{command_id}",
            (application_id, guild_id, command_id),
        ));
        send_command(request, command)
    }

    /// `DELETE /applications/{application_id}/commands/{command_id}`.
    pub fn delete_global_command(
        &self,
        application_id: Id<ApplicationMarker>,
        command_id: Id<CommandMarker>,
    ) -> impl Future<Output = Result<(), HttpError>> + Send + 'static {
        self.delete(Route::raw(
            "/applications/{application_id}/commands/{command_id}",
            (application_id, command_id),
        ))
        .send_empty()
    }

    /// `DELETE /applications/{application_id}/guilds/{guild_id}/commands/{command_id}`.
    pub fn delete_guild_command(
        &self,
        application_id: Id<ApplicationMarker>,
        guild_id: Id<GuildMarker>,
        command_id: Id<CommandMarker>,
    ) -> impl Future<Output = Result<(), HttpError>> + Send + 'static {
        self.delete(Route::raw(
            "/applications/{application_id}/guilds/{guild_id}/commands/{command_id}",
            (application_id, guild_id, command_id),
        ))
        .send_empty()
    }
}
