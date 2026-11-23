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
        }
    }
}

impl From<cordex_model::ParseError> for Error {
    fn from(err: cordex_model::ParseError) -> Self {
        Self::Model(err)
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

/// The types most bots need: `use cordex::prelude::*;`.
pub mod prelude {
    pub use crate::{Error, Result};
    pub use cordex_model::component::{
        ActionRow, Button, Component, Label, SelectOption, StringSelect, TextInput,
    };
    pub use cordex_model::gateway::{Event, Intents};
    pub use cordex_model::{
        Channel, Guild, Id, Member, Message, Permissions, Role, Timestamp, Token, User,
    };

    #[cfg(feature = "bot")]
    pub use crate::bot::{
        AutocompleteContext, Bot, CommandContext, ComponentContext, Context, EventContext,
        MessageContext, ModalContext, ReadyContext,
    };
    #[cfg(feature = "cache")]
    pub use cordex_cache::{CacheConfig, CacheParts, InMemoryCache, ResourceType};
    #[cfg(feature = "gateway")]
    pub use cordex_gateway::{CloseMode, EventTypeFlags, Shard, ShardId, ShardManager};
    #[cfg(feature = "http")]
    pub use cordex_http::Client;
    #[cfg(feature = "interactions")]
    pub use cordex_interactions::{
        Choice, Command, CommandOption, CommandOptions, InteractionHandle, Reply,
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
