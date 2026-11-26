//! Traits implemented by `#[derive(CommandArgs)]` and `#[derive(CommandChoice)]`
//! (or implemented manually without macros).

use crate::command::{Choice, CommandOption};
use crate::error::InteractionError;
use crate::options::CommandOptions;

/// Enum whose variants map to Discord slash-command string choices.
pub trait CommandChoice: Sized {
    /// All choices to register on the [`CommandOption`].
    fn choices() -> Vec<Choice>;

    /// Parse a wire string value back into a variant.
    fn from_choice_value(value: &str) -> Option<Self>;
}

/// Struct representing typed slash-command arguments.
pub trait CommandArgs: Sized {
    /// Option definitions to attach to a [`Command`](crate::Command).
    fn options() -> Vec<CommandOption>;

    /// Extract typed arguments from incoming [`CommandOptions`].
    fn from_options(options: &CommandOptions<'_>) -> Result<Self, InteractionError>;
}
