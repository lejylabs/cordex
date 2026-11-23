//! [`InteractionClient`]: the application-scoped endpoints without passing
//! the application ID every time.

use std::future::Future;

use cordex_model::application::{ActivityInstance, RoleConnectionMetadata};
use cordex_model::command::{ApplicationCommand, CommandPermission, GuildCommandPermissions};
use cordex_model::emoji::Emoji;
use cordex_model::entitlement::Entitlement;
use cordex_model::id::Id;
use cordex_model::id::marker::{
    ApplicationMarker, CommandMarker, EmojiMarker, EntitlementMarker, GuildMarker,
    InteractionMarker,
};
use cordex_model::interaction::{InteractionCallbackResponse, InteractionResponse};
use cordex_model::message::Message;
use cordex_model::sku::Sku;
use cordex_model::token::InteractionToken;

use super::CommandBody;
use crate::client::Client;
use crate::error::HttpError;

/// Commands, interaction responses, follow-ups, application emojis,
/// entitlements and SKUs of one application, without repeating its ID.
///
/// ```no_run
/// use cordex_http::Client;
///
/// # async fn run(token: &str) -> Result<(), cordex_http::HttpError> {
/// let client = Client::new(token)?;
/// let app = client.interaction_for_current_app().await?;
/// let commands = app.global_commands().await?;
/// println!("{} global commands", commands.len());
/// # Ok(())
/// # }
/// ```
#[derive(Clone, Debug)]
pub struct InteractionClient {
    client: Client,
    application_id: Id<ApplicationMarker>,
}

impl Client {
    /// Endpoints of application `application_id` without passing the ID
    /// again (the bot's application ID is in `READY` and in
    /// `GET /applications/@me`).
    pub fn interaction(&self, application_id: Id<ApplicationMarker>) -> InteractionClient {
        InteractionClient {
            client: self.clone(),
            application_id,
        }
    }

    /// Like [`Self::interaction`], with the ID of the token's own
    /// application (one `GET /applications/@me` request).
    pub async fn interaction_for_current_app(&self) -> Result<InteractionClient, HttpError> {
        let app = self.get_current_application().await?;
        Ok(self.interaction(app.id))
    }
}

impl InteractionClient {
    /// The application ID.
    pub const fn application_id(&self) -> Id<ApplicationMarker> {
        self.application_id
    }

    /// The underlying client.
    pub const fn client(&self) -> &Client {
        &self.client
    }

    // ---- global commands ---------------------------------------------------

    /// All global commands.
    pub fn global_commands(
        &self,
    ) -> impl Future<Output = Result<Vec<ApplicationCommand>, HttpError>> + Send + 'static {
        self.client.get_global_commands(self.application_id)
    }

    /// One global command.
    pub fn global_command(
        &self,
        command_id: Id<CommandMarker>,
    ) -> impl Future<Output = Result<ApplicationCommand, HttpError>> + Send + 'static {
        self.client
            .get_global_command(self.application_id, command_id)
    }

    /// Create (or overwrite by name) one global command.
    pub fn create_global_command(
        &self,
        command: impl CommandBody,
    ) -> impl Future<Output = Result<ApplicationCommand, HttpError>> + Send + 'static {
        self.client
            .create_global_command(self.application_id, command)
    }

    /// Edit one global command.
    pub fn edit_global_command(
        &self,
        command_id: Id<CommandMarker>,
        command: impl CommandBody,
    ) -> impl Future<Output = Result<ApplicationCommand, HttpError>> + Send + 'static {
        self.client
            .edit_global_command(self.application_id, command_id, command)
    }

    /// Delete one global command.
    pub fn delete_global_command(
        &self,
        command_id: Id<CommandMarker>,
    ) -> impl Future<Output = Result<(), HttpError>> + Send + 'static {
        self.client
            .delete_global_command(self.application_id, command_id)
    }

    /// Replace all global commands (bulk overwrite).
    pub fn set_global_commands(
        &self,
        commands: impl IntoIterator<Item = impl CommandBody>,
    ) -> impl Future<Output = Result<Vec<ApplicationCommand>, HttpError>> + Send + 'static {
        self.client
            .bulk_overwrite_global_commands(self.application_id, commands)
    }

    // ---- guild commands ----------------------------------------------------

    /// All commands of a guild.
    pub fn guild_commands(
        &self,
        guild_id: Id<GuildMarker>,
    ) -> impl Future<Output = Result<Vec<ApplicationCommand>, HttpError>> + Send + 'static {
        self.client
            .get_guild_commands(self.application_id, guild_id)
    }

    /// One guild command.
    pub fn guild_command(
        &self,
        guild_id: Id<GuildMarker>,
        command_id: Id<CommandMarker>,
    ) -> impl Future<Output = Result<ApplicationCommand, HttpError>> + Send + 'static {
        self.client
            .get_guild_command(self.application_id, guild_id, command_id)
    }

    /// Create (or overwrite by name) one guild command.
    pub fn create_guild_command(
        &self,
        guild_id: Id<GuildMarker>,
        command: impl CommandBody,
    ) -> impl Future<Output = Result<ApplicationCommand, HttpError>> + Send + 'static {
        self.client
            .create_guild_command(self.application_id, guild_id, command)
    }

    /// Edit one guild command.
    pub fn edit_guild_command(
        &self,
        guild_id: Id<GuildMarker>,
        command_id: Id<CommandMarker>,
        command: impl CommandBody,
    ) -> impl Future<Output = Result<ApplicationCommand, HttpError>> + Send + 'static {
        self.client
            .edit_guild_command(self.application_id, guild_id, command_id, command)
    }

    /// Delete one guild command.
    pub fn delete_guild_command(
        &self,
        guild_id: Id<GuildMarker>,
        command_id: Id<CommandMarker>,
    ) -> impl Future<Output = Result<(), HttpError>> + Send + 'static {
        self.client
            .delete_guild_command(self.application_id, guild_id, command_id)
    }

    /// Replace all commands of a guild (bulk overwrite).
    pub fn set_guild_commands(
        &self,
        guild_id: Id<GuildMarker>,
        commands: impl IntoIterator<Item = impl CommandBody>,
    ) -> impl Future<Output = Result<Vec<ApplicationCommand>, HttpError>> + Send + 'static {
        self.client
            .bulk_overwrite_guild_commands(self.application_id, guild_id, commands)
    }

    // ---- command permissions ----------------------------------------------

    /// Permissions of all commands in a guild.
    pub fn guild_command_permissions(
        &self,
        guild_id: Id<GuildMarker>,
    ) -> impl Future<Output = Result<Vec<GuildCommandPermissions>, HttpError>> + Send + 'static
    {
        self.client
            .get_guild_command_permissions(self.application_id, guild_id)
    }

    /// Permissions of one command in a guild.
    pub fn command_permissions(
        &self,
        guild_id: Id<GuildMarker>,
        command_id: Id<CommandMarker>,
    ) -> impl Future<Output = Result<GuildCommandPermissions, HttpError>> + Send + 'static {
        self.client
            .get_command_permissions(self.application_id, guild_id, command_id)
    }

    /// Replace the permissions of one command (Bearer token only).
    pub fn edit_command_permissions(
        &self,
        guild_id: Id<GuildMarker>,
        command_id: Id<CommandMarker>,
        permissions: Vec<CommandPermission>,
    ) -> impl Future<Output = Result<GuildCommandPermissions, HttpError>> + Send + 'static {
        self.client
            .edit_command_permissions(self.application_id, guild_id, command_id, permissions)
    }

    /// Replace the permissions of several commands (Bearer token only).
    pub fn batch_edit_command_permissions(
        &self,
        guild_id: Id<GuildMarker>,
        permissions: Vec<GuildCommandPermissions>,
    ) -> impl Future<Output = Result<Vec<GuildCommandPermissions>, HttpError>> + Send + 'static
    {
        self.client
            .batch_edit_command_permissions(self.application_id, guild_id, permissions)
    }

    // ---- interaction responses ---------------------------------------------

    /// Initial response to an interaction (within 3 s). For convenience
    /// with defer, edit and follow-ups see `cordex_interactions::InteractionHandle`.
    pub fn create_response(
        &self,
        interaction_id: Id<InteractionMarker>,
        token: &InteractionToken,
        response: &InteractionResponse,
    ) -> impl Future<Output = Result<(), HttpError>> + Send + 'static {
        self.client
            .create_interaction_response(interaction_id, token, response)
    }

    /// Initial response that returns the created callback (e.g. the message).
    pub fn create_response_with_response(
        &self,
        interaction_id: Id<InteractionMarker>,
        token: &InteractionToken,
        response: &InteractionResponse,
    ) -> impl Future<Output = Result<InteractionCallbackResponse, HttpError>> + Send + 'static {
        self.client
            .create_interaction_response_with_response(interaction_id, token, response)
    }

    /// The original response message.
    pub fn original_response(
        &self,
        token: &InteractionToken,
    ) -> impl Future<Output = Result<Message, HttpError>> + Send + 'static {
        self.client
            .get_original_interaction_response(self.application_id, token)
    }

    /// Edit the original response.
    pub fn update_response(&self, token: &InteractionToken) -> super::UpdateInteractionResponse {
        self.client
            .update_interaction_response(self.application_id, token)
    }

    /// Delete the original response.
    pub fn delete_response(
        &self,
        token: &InteractionToken,
    ) -> impl Future<Output = Result<(), HttpError>> + Send + 'static {
        self.client
            .delete_interaction_response(self.application_id, token)
    }

    /// Send a follow-up message.
    pub fn create_followup(&self, token: &InteractionToken) -> super::CreateFollowupMessage {
        self.client
            .create_followup_message(self.application_id, token)
    }

    // ---- application emojis --------------------------------------------------

    /// Emojis owned by the application.
    pub fn emojis(&self) -> impl Future<Output = Result<Vec<Emoji>, HttpError>> + Send + 'static {
        self.client.get_application_emojis(self.application_id)
    }

    /// One application emoji.
    pub fn emoji(
        &self,
        emoji_id: Id<EmojiMarker>,
    ) -> impl Future<Output = Result<Emoji, HttpError>> + Send + 'static {
        self.client
            .get_application_emoji(self.application_id, emoji_id)
    }

    /// Upload an application emoji (`image` as a data URI, max. 256 KiB).
    pub fn create_emoji(
        &self,
        name: &str,
        image: &str,
    ) -> impl Future<Output = Result<Emoji, HttpError>> + Send + 'static {
        self.client
            .create_application_emoji(self.application_id, name, image)
    }

    /// Rename an application emoji.
    pub fn rename_emoji(
        &self,
        emoji_id: Id<EmojiMarker>,
        name: &str,
    ) -> impl Future<Output = Result<Emoji, HttpError>> + Send + 'static {
        self.client
            .update_application_emoji(self.application_id, emoji_id, name)
    }

    /// Delete an application emoji.
    pub fn delete_emoji(
        &self,
        emoji_id: Id<EmojiMarker>,
    ) -> impl Future<Output = Result<(), HttpError>> + Send + 'static {
        self.client
            .delete_application_emoji(self.application_id, emoji_id)
    }

    // ---- monetization and metadata -------------------------------------------

    /// Entitlements (filters on the returned request).
    pub fn entitlements(&self) -> super::GetEntitlements {
        self.client.get_entitlements(self.application_id)
    }

    /// One entitlement.
    pub fn entitlement(
        &self,
        entitlement_id: Id<EntitlementMarker>,
    ) -> impl Future<Output = Result<Entitlement, HttpError>> + Send + 'static {
        self.client
            .get_entitlement(self.application_id, entitlement_id)
    }

    /// Mark a consumable one-time purchase as used.
    pub fn consume_entitlement(
        &self,
        entitlement_id: Id<EntitlementMarker>,
    ) -> impl Future<Output = Result<(), HttpError>> + Send + 'static {
        self.client
            .consume_entitlement(self.application_id, entitlement_id)
    }

    /// The application's SKUs.
    pub fn skus(&self) -> impl Future<Output = Result<Vec<Sku>, HttpError>> + Send + 'static {
        self.client.get_skus(self.application_id)
    }

    /// Linked-role metadata records.
    pub fn role_connection_metadata(
        &self,
    ) -> impl Future<Output = Result<Vec<RoleConnectionMetadata>, HttpError>> + Send + 'static {
        self.client
            .get_role_connection_metadata(self.application_id)
    }

    /// Replace the linked-role metadata (max. 5 records).
    pub fn set_role_connection_metadata(
        &self,
        records: Vec<RoleConnectionMetadata>,
    ) -> impl Future<Output = Result<Vec<RoleConnectionMetadata>, HttpError>> + Send + 'static {
        self.client
            .update_role_connection_metadata(self.application_id, records)
    }

    /// A running activity instance.
    pub fn activity_instance(
        &self,
        instance_id: &str,
    ) -> impl Future<Output = Result<ActivityInstance, HttpError>> + Send + 'static {
        self.client
            .get_activity_instance(self.application_id, instance_id.to_owned())
    }
}
