//! Facade crate: re-exports the `cordex-*` crates behind features, one
//! [`Error`] type over all of them and a [`prelude`].
//!
//! The rest of this page is the project README.
#![doc = ""]
#![doc = include_str!("../README.md")]

use std::error::Error as StdError;
use std::fmt;

pub use cordex_model as model;

#[cfg(feature = "cache")]
pub use cordex_cache as cache;

#[cfg(all(feature = "cache", feature = "http"))]
pub mod backfill;

#[cfg(feature = "bot")]
pub mod bot;

#[cfg(feature = "gateway")]
pub use cordex_gateway as gateway;
#[cfg(feature = "http")]
pub use cordex_http as http;
#[cfg(feature = "interactions")]
pub use cordex_interactions as interactions;
#[cfg(feature = "lavalink")]
pub use cordex_lavalink as lavalink;
#[cfg(feature = "voice")]
pub use cordex_voice as voice;

/// Any error from the enabled `cordex-*` crates.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// Parsing a model value failed.
    Model(cordex_model::ParseError),
    /// A REST request failed.
    #[cfg(feature = "http")]
    Http(cordex_http::HttpError),
    /// A gateway shard or the shard manager failed.
    #[cfg(feature = "gateway")]
    Gateway(cordex_gateway::GatewayError),
    /// Building a command or answering an interaction failed.
    #[cfg(feature = "interactions")]
    Interaction(cordex_interactions::InteractionError),
    /// A voice signaling operation failed.
    #[cfg(feature = "voice")]
    Voice(cordex_voice::VoiceError),
    /// A Lavalink v4 operation failed.
    #[cfg(feature = "lavalink")]
    Lavalink(cordex_lavalink::LavalinkError),
    /// An embed builder check failed.
    Embed(cordex_model::EmbedError),
    /// A required environment variable (e.g. `DISCORD_TOKEN`) was missing or empty.
    MissingEnv(Box<str>),
    /// The invoking user or the bot is missing a required Discord permission.
    MissingPermission {
        /// The permission that was required.
        permission: cordex_model::Permissions,
        /// Who is missing the permission (`"user"` or `"bot"`).
        actor: &'static str,
    },
}

/// `Result` with the facade [`Error`].
pub type Result<T, E = Error> = std::result::Result<T, E>;

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Model(err) => fmt::Display::fmt(err, f),
            #[cfg(feature = "http")]
            Self::Http(err) => fmt::Display::fmt(err, f),
            #[cfg(feature = "gateway")]
            Self::Gateway(err) => fmt::Display::fmt(err, f),
            #[cfg(feature = "interactions")]
            Self::Interaction(err) => fmt::Display::fmt(err, f),
            #[cfg(feature = "voice")]
            Self::Voice(err) => fmt::Display::fmt(err, f),
            #[cfg(feature = "lavalink")]
            Self::Lavalink(err) => fmt::Display::fmt(err, f),
            Self::Embed(err) => fmt::Display::fmt(err, f),
            Self::MissingEnv(var) => write!(
                f,
                "environment variable `{var}` is not set (export it in your shell or add it to a .env file)"
            ),
            Self::MissingPermission { permission, actor } => {
                write!(
                    f,
                    "missing required permission `{permission:?}` for {actor}"
                )
            }
        }
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Model(err) => Some(err),
            #[cfg(feature = "http")]
            Self::Http(err) => Some(err),
            #[cfg(feature = "gateway")]
            Self::Gateway(err) => Some(err),
            #[cfg(feature = "interactions")]
            Self::Interaction(err) => Some(err),
            #[cfg(feature = "voice")]
            Self::Voice(err) => Some(err),
            #[cfg(feature = "lavalink")]
            Self::Lavalink(err) => Some(err),
            Self::Embed(err) => Some(err),
            Self::MissingEnv(_) | Self::MissingPermission { .. } => None,
        }
    }
}

impl From<cordex_model::ParseError> for Error {
    fn from(err: cordex_model::ParseError) -> Self {
        Self::Model(err)
    }
}

impl From<cordex_model::EmbedError> for Error {
    fn from(err: cordex_model::EmbedError) -> Self {
        Self::Embed(err)
    }
}

#[cfg(feature = "http")]
impl From<cordex_http::HttpError> for Error {
    fn from(err: cordex_http::HttpError) -> Self {
        Self::Http(err)
    }
}

#[cfg(feature = "gateway")]
impl From<cordex_gateway::GatewayError> for Error {
    fn from(err: cordex_gateway::GatewayError) -> Self {
        Self::Gateway(err)
    }
}

#[cfg(feature = "interactions")]
impl From<cordex_interactions::InteractionError> for Error {
    fn from(err: cordex_interactions::InteractionError) -> Self {
        Self::Interaction(err)
    }
}

#[cfg(feature = "voice")]
impl From<cordex_voice::VoiceError> for Error {
    fn from(err: cordex_voice::VoiceError) -> Self {
        Self::Voice(err)
    }
}

#[cfg(feature = "lavalink")]
impl From<cordex_lavalink::LavalinkError> for Error {
    fn from(err: cordex_lavalink::LavalinkError) -> Self {
        Self::Lavalink(err)
    }
}

/// The types most bots need: `use cordex::prelude::*;`.
pub mod prelude {
    pub use crate::{Error, Result};
    pub use cordex_model::cdn::{CdnUrl, ImageFormat};
    pub use cordex_model::component::{
        ActionRow, Button, Component, Label, SelectOption, StringSelect, TextInput,
    };
    pub use cordex_model::gateway::{Event, Intents};
    pub use cordex_model::mention::{Mentionable, ParsedMention, parse_mentions};
    pub use cordex_model::message::{
        AllowedMentions, Embed, EmbedError, MessageBuilder, MessageLink, colors, parse_message_link,
    };
    pub use cordex_model::{
        ApplicationId, AttachmentId, Channel, ChannelId, CommandId, EmojiId, Guild, GuildId, Id,
        InteractionId, Locale, Member, Message, MessageId, Permissions, Role, RoleId, Timestamp,
        TimestampStyle, Token, User, UserId, WebhookId, escape_markdown, escape_mentions,
        parse_webhook_url, split_message, split_message_2000, split_message_4096,
    };

    #[cfg(feature = "bot")]
    pub use crate::bot::{
        AutocompleteContext, Bot, BotModule, CommandContext, ComponentContext, Context,
        CooldownScope, Cooldowns, EventContext, MessageContext, ModalContext, Next, ReadyContext,
        SchedulerHandle, ShutdownToken,
    };
    #[cfg(feature = "cache")]
    pub use cordex_cache::{CacheConfig, CacheParts, CachePolicy, InMemoryCache, ResourceType};
    #[cfg(feature = "gateway")]
    pub use cordex_gateway::{CloseMode, EventTypeFlags, Shard, ShardId, ShardManager, Standby};
    #[cfg(feature = "http")]
    pub use cordex_http::{Client, InteractionClient, Typing};
    #[cfg(feature = "interactions")]
    pub use cordex_interactions::{
        Choice, Command, CommandOption, CommandOptions, CustomIdSigner, InteractionHandle,
        LayoutBuilder, Modal, Reply, SignedCustomId, SyncReport,
    };
    #[cfg(feature = "voice")]
    pub use cordex_voice::{
        LavalinkVoiceState, VoiceConnectionInfo, VoiceError, VoiceEvent, VoiceGateway,
        VoiceManager, VoiceOptions,
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "gateway")]
    #[test]
    fn gateway_error_converts_and_keeps_message() {
        fn fails() -> Result<()> {
            Err(cordex_gateway::GatewayError::NoRuntime)?;
            Ok(())
        }
        let err = fails().unwrap_err();
        assert!(matches!(err, Error::Gateway(_)));
        assert_eq!(
            err.to_string(),
            cordex_gateway::GatewayError::NoRuntime.to_string()
        );
        assert!(err.source().is_some());
    }

    #[cfg(feature = "http")]
    #[test]
    fn http_error_converts() {
        let err: Error = cordex_http::HttpError::Unauthorized.into();
        assert!(matches!(err, Error::Http(_)));
    }
}
