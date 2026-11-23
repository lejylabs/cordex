//! Discord interactions: command builders, responses and the HTTP
//! interaction endpoint with signature verification.

pub mod command;
pub mod error;
pub mod handle;
pub mod modal;
pub mod options;
pub mod reply;
#[cfg(feature = "server")]
pub mod server;
#[cfg(feature = "verify")]
pub mod verify;

pub use command::{Choice, Command, CommandOption};
pub use error::InteractionError;
pub use handle::{InteractionHandle, ResponseState};
pub use modal::Modal;
pub use options::{CommandOptions, Mentionable, ResolvedUser};
pub use reply::Reply;
#[cfg(feature = "server")]
pub use server::{InteractionEndpoint, InteractionEndpointBuilder};
#[cfg(feature = "verify")]
pub use verify::{PublicKey, verify_signature};
