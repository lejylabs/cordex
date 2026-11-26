//! Integration tests for `#[derive(CommandChoice)]` and `#[derive(CommandArgs)]`.

#![cfg(feature = "macros")]

use cordex_interactions::{Command, CommandArgs, CommandChoice};

#[derive(Debug, Clone, Copy, PartialEq, Eq, CommandChoice)]
enum Color {
    #[choice(name = "Discord Blurple", value = "blurple")]
    Blurple,
    Red,
}

#[derive(Debug, PartialEq, CommandArgs)]
struct GreetArgs {
    /// Message to include
    note: String,
    #[arg(choice, description = "Embed color")]
    color: Color,
    #[arg(description = "Repeat count")]
    count: Option<i64>,
}

#[test]
fn derive_command_choice_and_args_generate_valid_command() {
    let choices = Color::choices();
    assert_eq!(choices.len(), 2);
    assert_eq!(Color::from_choice_value("blurple"), Some(Color::Blurple));
    assert_eq!(Color::from_choice_value("red"), Some(Color::Red));
    assert_eq!(Color::from_choice_value("unknown"), None);

    let opts = GreetArgs::options();
    assert_eq!(opts.len(), 3);

    let cmd = Command::slash("greet", "Greet someone").with_args::<GreetArgs>();
    assert!(cmd.validate().is_ok());
}
