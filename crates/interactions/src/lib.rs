//! Discord interactions: command builders, responses and the HTTP
//! interaction endpoint with signature verification (PLAN.md §11).

pub mod command;
pub mod error;

pub use command::{Choice, Command, CommandOption};
pub use error::InteractionError;
