#![doc = include_str!("../README.md")]

pub mod command;
pub mod custom_id;
pub mod error;
pub mod handle;
pub mod modal;
pub mod options;
pub mod reply;
#[cfg(feature = "server")]
pub mod server;
pub mod sync;
pub mod v2;
#[cfg(feature = "verify")]
pub mod verify;

pub use command::{Choice, Command, CommandOption};
pub use custom_id::{CustomIdError, CustomIdSigner, SignedCustomId};
pub use error::InteractionError;
pub use handle::{InteractionHandle, ResponseState};
pub use modal::Modal;
pub use options::{CommandOptions, Mentionable, ResolvedUser};
pub use reply::Reply;
#[cfg(feature = "server")]
pub use server::{InteractionEndpoint, InteractionEndpointBuilder};
pub use sync::{SyncReport, dev_guild_id_from_env, diff_commands};
pub use v2::{LayoutBuilder, LayoutError};
#[cfg(feature = "verify")]
pub use verify::{PublicKey, verify_signature};
