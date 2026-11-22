//! Discord interactions: command builders, responses and the HTTP
//! interaction endpoint with signature verification (PLAN.md §11).

pub mod command;
pub mod error;
pub mod handle;
pub mod options;
pub mod reply;

pub use command::{Choice, Command, CommandOption};
pub use error::InteractionError;
pub use handle::{InteractionHandle, ResponseState};
pub use options::{CommandOptions, Mentionable, ResolvedUser};
pub use reply::Reply;
