//! Application commands (interactions/application-commands.mdx).

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::application::ApplicationIntegrationType;
use crate::channel::ChannelType;
use crate::id::Id;
use crate::id::marker::{
    ApplicationMarker, ChannelMarker, CommandMarker, GenericMarker, GuildMarker, RoleMarker,
    UserMarker,
};
use crate::permissions::Permissions;
use crate::serde_util::int_enum;

int_enum! {
    /// Where an interaction can be used, or where it was triggered.
    pub enum InteractionContextType: u8 {
        /// Inside a server.
        Guild = 0,
        /// DM with the app's bot user.
        BotDm = 1,
        /// Group DMs and DMs other than the app's bot user.
        PrivateChannel = 2,
    }
}

int_enum! {
    /// Application command type.
    pub enum CommandType: u8 {
        /// Slash command (`/`).
        ChatInput = 1,
        /// Context menu on a user.
        User = 2,
        /// Context menu on a message.
        Message = 3,
        /// Primary way to launch an Activity.
        PrimaryEntryPoint = 4,
    }
}

impl CommandType {
    const fn default_chat() -> Self {
        Self::ChatInput
    }
}

int_enum! {
    /// Application command option type.
    pub enum CommandOptionType: u8 {
        /// Nested subcommand.
        SubCommand = 1,
        /// Group of subcommands.
        SubCommandGroup = 2,
        /// String.
        String = 3,
        /// Integer (`-2^53+1` … `2^53-1`).
        Integer = 4,
        /// Boolean.
        Boolean = 5,
        /// User.
        User = 6,
        /// Channel (any type, including categories).
        Channel = 7,
        /// Role.
        Role = 8,
        /// User or role.
        Mentionable = 9,
        /// Floating-point number (`-2^53` … `2^53`).
        Number = 10,
        /// Attachment.
        Attachment = 11,
    }
}

int_enum! {
    /// Who handles a primary entry point command.
    pub enum EntryPointHandlerType: u8 {
        /// The app responds to the interaction.
        AppHandler = 1,
        /// Discord launches the Activity.
        DiscordLaunchActivity = 2,
    }
}

/// An application command, as returned by Discord.
///
/// Create/edit bodies omit `id`, `application_id` and `version`; those fields
/// default so the documented create examples parse as well.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ApplicationCommand {
    /// Command ID (missing on create bodies).
    pub id: Option<Id<CommandMarker>>,
    /// Command type (default `CHAT_INPUT`).
    #[serde(rename = "type", default = "CommandType::default_chat")]
    pub kind: CommandType,
    /// Parent application (missing on create bodies).
    pub application_id: Option<Id<ApplicationMarker>>,
    /// Guild of a guild command.
    pub guild_id: Option<Id<GuildMarker>>,
    /// Name (1–32 characters).
    pub name: Box<str>,
    /// Localized names.
    pub name_localizations: Option<HashMap<Box<str>, Box<str>>>,
    /// Description (`CHAT_INPUT` / `PRIMARY_ENTRY_POINT`; empty for context menus).
    #[serde(default)]
    pub description: Box<str>,
    /// Localized descriptions.
    pub description_localizations: Option<HashMap<Box<str>, Box<str>>>,
    /// Options (`CHAT_INPUT` only, max. 25).
    #[serde(default)]
    pub options: Vec<CommandOption>,
    /// Default member permissions required to use the command (`null` = everyone).
    pub default_member_permissions: Option<Permissions>,
    /// Deprecated: whether the command is available in DMs with the app.
    pub dm_permission: Option<bool>,
    /// Deprecated: whether the command is enabled by default in a guild.
    pub default_permission: Option<bool>,
    /// Age-restricted command.
    #[serde(default)]
    pub nsfw: bool,
    /// Installation contexts (global commands).
    #[serde(default)]
    pub integration_types: Vec<ApplicationIntegrationType>,
    /// Interaction contexts (global commands).
    pub contexts: Option<Vec<InteractionContextType>>,
    /// Autoincrementing version identifier (missing on create bodies).
    pub version: Option<Id<GenericMarker>>,
    /// Entry-point handler (`PRIMARY_ENTRY_POINT` only).
    pub handler: Option<EntryPointHandlerType>,
}

/// Parameter, subcommand or subcommand group of a chat-input command.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct CommandOption {
    /// Option type.
    #[serde(rename = "type")]
    pub kind: CommandOptionType,
    /// Name (unique within the parent, 1–32 characters).
    pub name: Box<str>,
    /// Localized names.
    pub name_localizations: Option<HashMap<Box<str>, Box<str>>>,
    /// Description (1–100 characters).
    pub description: Box<str>,
    /// Localized descriptions.
    pub description_localizations: Option<HashMap<Box<str>, Box<str>>>,
    /// Whether the option is required (default false; not for subcommands).
    #[serde(default)]
    pub required: bool,
    /// Fixed choices (`STRING` / `INTEGER` / `NUMBER`, max. 25).
    #[serde(default)]
    pub choices: Vec<CommandOptionChoice>,
    /// Nested options (subcommand or group).
    #[serde(default)]
    pub options: Vec<CommandOption>,
    /// Allowed channel types (`CHANNEL` options).
    #[serde(default)]
    pub channel_types: Vec<ChannelType>,
    /// Minimum value (`INTEGER` / `NUMBER`).
    pub min_value: Option<serde_json::Number>,
    /// Maximum value (`INTEGER` / `NUMBER`).
    pub max_value: Option<serde_json::Number>,
    /// Minimum string length (0–6000).
    pub min_length: Option<u16>,
    /// Maximum string length (1–6000).
    pub max_length: Option<u16>,
    /// Autocomplete enabled (`STRING` / `INTEGER` / `NUMBER`; not with `choices`).
    #[serde(default)]
    pub autocomplete: bool,
    /// File type filters (`ATTACHMENT`, max. 10).
    #[serde(default)]
    pub file_types: Vec<Box<str>>,
}

/// Choice of a string, integer or number option.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct CommandOptionChoice {
    /// Shown name (1–100 characters).
    pub name: Box<str>,
    /// Localized names.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name_localizations: Option<HashMap<Box<str>, Box<str>>>,
    /// Value (string, integer or number matching the option type).
    pub value: CommandOptionChoiceValue,
}

/// Value of a command option choice.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CommandOptionChoiceValue {
    /// Integer choice.
    Integer(i64),
    /// Floating-point choice.
    Number(f64),
    /// String choice (max. 100 characters).
    String(Box<str>),
}

/// Guild-wide overwrites for a command (or all commands when `id` is the
/// application ID).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct GuildCommandPermissions {
    /// Command ID, or the application ID for defaults that apply to all
    /// commands without an explicit overwrite.
    pub id: Id<GenericMarker>,
    /// Application.
    pub application_id: Id<ApplicationMarker>,
    /// Guild.
    pub guild_id: Id<GuildMarker>,
    /// Overwrites (max. 100).
    #[serde(default)]
    pub permissions: Vec<CommandPermission>,
}

/// One user, role or channel overwrite for a command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct CommandPermission {
    /// Role, user or channel ID, or a permission constant (`guild_id` for
    /// `@everyone`, `guild_id - 1` for all channels).
    pub id: Id<GenericMarker>,
    /// Kind of `id`.
    #[serde(rename = "type")]
    pub kind: CommandPermissionType,
    /// `true` to allow, `false` to deny.
    pub permission: bool,
}

int_enum! {
    /// Application command permission type.
    pub enum CommandPermissionType: u8 {
        /// Role (or `@everyone` via the guild ID).
        Role = 1,
        /// User.
        User = 2,
        /// Channel (or all channels via `guild_id - 1`).
        Channel = 3,
    }
}

impl CommandPermission {
    /// Overwrite for every channel in a guild (`guild_id - 1`).
    pub fn all_channels(guild_id: Id<GuildMarker>, permission: bool) -> Option<Self> {
        Id::new(guild_id.get().wrapping_sub(1)).map(|id| Self {
            id,
            kind: CommandPermissionType::Channel,
            permission,
        })
    }

    /// Overwrite for `@everyone` (the guild ID as a role).
    pub const fn everyone(guild_id: Id<GuildMarker>, permission: bool) -> Self {
        Self {
            id: guild_id.cast(),
            kind: CommandPermissionType::Role,
            permission,
        }
    }

    /// Role this overwrite targets.
    pub const fn role_id(self) -> Option<Id<RoleMarker>> {
        match self.kind {
            CommandPermissionType::Role => Some(self.id.cast()),
            _ => None,
        }
    }

    /// User this overwrite targets.
    pub const fn user_id(self) -> Option<Id<UserMarker>> {
        match self.kind {
            CommandPermissionType::User => Some(self.id.cast()),
            _ => None,
        }
    }

    /// Channel this overwrite targets.
    pub const fn channel_id(self) -> Option<Id<ChannelMarker>> {
        match self.kind {
            CommandPermissionType::Channel => Some(self.id.cast()),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::marker::GuildMarker;

    #[test]
    fn everyone_and_all_channels_constants() {
        let guild = Id::<GuildMarker>::new(100).unwrap();
        let everyone = CommandPermission::everyone(guild, false);
        assert_eq!(everyone.id.get(), 100);
        assert_eq!(everyone.kind, CommandPermissionType::Role);

        let all = CommandPermission::all_channels(guild, true).unwrap();
        assert_eq!(all.id.get(), 99);
        assert_eq!(all.kind, CommandPermissionType::Channel);
    }

    #[test]
    fn command_type_defaults_to_slash() {
        let cmd: ApplicationCommand = serde_json::from_str(
            r#"{"name":"ping","description":"pong","application_id":"1","id":"2","version":"3"}"#,
        )
        .unwrap();
        assert_eq!(cmd.kind, CommandType::ChatInput);
        assert!(!cmd.nsfw);
    }
}
