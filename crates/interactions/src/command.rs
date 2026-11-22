//! Command builders with local validation (PLAN.md §11.1).
//!
//! ```
//! use drs_interactions::command::{Command, CommandOption};
//!
//! let greet = Command::slash("greet", "Greets a user")
//!     .option(CommandOption::user("user", "Who to greet").required())
//!     .option(CommandOption::string("text", "Extra text").max_length(200));
//! assert!(greet.validate().is_ok());
//!
//! let bad = Command::slash("Greet", "Greets a user");
//! assert_eq!(
//!     bad.validate().unwrap_err().to_string(),
//!     "invalid command: command name 'Greet' must be lowercase"
//! );
//! ```

use std::collections::{BTreeMap, HashSet};

use drs_http::CommandBody;
use drs_model::application::ApplicationIntegrationType;
use drs_model::channel::ChannelType;
use drs_model::command::{CommandOptionType, CommandType, InteractionContextType};
use drs_model::permissions::Permissions;
use serde::Serialize;

use crate::error::InteractionError;

/// Max. options per command or subcommand, and choices per option.
pub const MAX_OPTIONS: usize = 25;
/// Max. combined characters of names, descriptions and choices per command.
pub const MAX_TOTAL_CHARS: usize = 4000;

type Localizations = BTreeMap<Box<str>, Box<str>>;

/// Application command definition (slash, user or message command).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Command {
    #[serde(rename = "type")]
    kind: CommandType,
    name: Box<str>,
    #[serde(skip_serializing_if = "Localizations::is_empty")]
    name_localizations: Localizations,
    #[serde(skip_serializing_if = "str::is_empty")]
    description: Box<str>,
    #[serde(skip_serializing_if = "Localizations::is_empty")]
    description_localizations: Localizations,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    options: Vec<CommandOption>,
    #[serde(skip_serializing_if = "Option::is_none")]
    default_member_permissions: Option<Permissions>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    nsfw: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    integration_types: Option<Vec<ApplicationIntegrationType>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    contexts: Option<Vec<InteractionContextType>>,
}

impl Command {
    fn new(kind: CommandType, name: &str, description: &str) -> Self {
        Self {
            kind,
            name: name.into(),
            name_localizations: Localizations::new(),
            description: description.into(),
            description_localizations: Localizations::new(),
            options: Vec::new(),
            default_member_permissions: None,
            nsfw: false,
            integration_types: None,
            contexts: None,
        }
    }

    /// Slash command (`/name`). Name: lowercase, 1–32 characters; description 1–100.
    pub fn slash(name: impl AsRef<str>, description: impl AsRef<str>) -> Self {
        Self::new(CommandType::ChatInput, name.as_ref(), description.as_ref())
    }

    /// User context-menu command (name may contain spaces and capitals).
    pub fn user(name: impl AsRef<str>) -> Self {
        Self::new(CommandType::User, name.as_ref(), "")
    }

    /// Message context-menu command (name may contain spaces and capitals).
    pub fn message(name: impl AsRef<str>) -> Self {
        Self::new(CommandType::Message, name.as_ref(), "")
    }

    /// Command name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Command type.
    pub fn kind(&self) -> CommandType {
        self.kind
    }

    /// Add an option, subcommand or subcommand group (slash commands only).
    #[must_use]
    pub fn option(mut self, option: CommandOption) -> Self {
        self.options.push(option);
        self
    }

    /// Localized name, e.g. `("de", "grüßen")`.
    #[must_use]
    pub fn name_localization(mut self, locale: impl AsRef<str>, name: impl AsRef<str>) -> Self {
        self.name_localizations
            .insert(locale.as_ref().into(), name.as_ref().into());
        self
    }

    /// Localized description.
    #[must_use]
    pub fn description_localization(
        mut self,
        locale: impl AsRef<str>,
        description: impl AsRef<str>,
    ) -> Self {
        self.description_localizations
            .insert(locale.as_ref().into(), description.as_ref().into());
        self
    }

    /// Permissions a member needs by default (`Permissions::empty()` = admins only).
    #[must_use]
    pub fn default_member_permissions(mut self, permissions: Permissions) -> Self {
        self.default_member_permissions = Some(permissions);
        self
    }

    /// Age-restricted command.
    #[must_use]
    pub fn nsfw(mut self, nsfw: bool) -> Self {
        self.nsfw = nsfw;
        self
    }

    /// Installation contexts (guild install, user install).
    #[must_use]
    pub fn integration_types(
        mut self,
        types: impl IntoIterator<Item = ApplicationIntegrationType>,
    ) -> Self {
        self.integration_types = Some(types.into_iter().collect());
        self
    }

    /// Where the command can be used (guild, bot DM, private channel).
    #[must_use]
    pub fn contexts(mut self, contexts: impl IntoIterator<Item = InteractionContextType>) -> Self {
        self.contexts = Some(contexts.into_iter().collect());
        self
    }

    /// Check every Discord limit locally.
    pub fn validate(&self) -> Result<(), InteractionError> {
        self.check_inner().map_err(InteractionError::InvalidCommand)
    }

    fn check_inner(&self) -> Result<(), Box<str>> {
        let slash = self.kind == CommandType::ChatInput;
        if slash {
            check_slash_name("command", &self.name)?;
            check_len("command description", &self.description, 1, 100)?;
        } else {
            check_len("command name", &self.name, 1, 32)?;
            if !self.description.is_empty() {
                return Err("context-menu commands have no description".into());
            }
            if !self.options.is_empty() {
                return Err("context-menu commands have no options".into());
            }
        }
        for (locale, name) in &self.name_localizations {
            if slash {
                check_slash_name(&format!("localized ({locale}) command"), name)?;
            } else {
                check_len(&format!("localized ({locale}) command name"), name, 1, 32)?;
            }
        }
        for (locale, desc) in &self.description_localizations {
            check_len(
                &format!("localized ({locale}) command description"),
                desc,
                1,
                100,
            )?;
        }
        check_options(&self.options, Level::Top)?;
        let total = self.name.chars().count()
            + self.description.chars().count()
            + self
                .options
                .iter()
                .map(CommandOption::total_chars)
                .sum::<usize>();
        if total > MAX_TOTAL_CHARS {
            return Err(format!(
                "command '{}' has {total} characters in names, descriptions and choices; max. {MAX_TOTAL_CHARS}",
                self.name
            )
            .into());
        }
        Ok(())
    }
}

impl CommandBody for Command {
    fn check(&self) -> Result<(), Box<str>> {
        self.check_inner()
    }
}

/// Fixed choice of a string, integer or number option.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Choice {
    name: Box<str>,
    #[serde(skip_serializing_if = "Localizations::is_empty")]
    name_localizations: Localizations,
    value: ChoiceValue,
}

impl Choice {
    /// Localized choice name.
    #[must_use]
    pub fn localization(mut self, locale: impl AsRef<str>, name: impl AsRef<str>) -> Self {
        self.name_localizations
            .insert(locale.as_ref().into(), name.as_ref().into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
enum ChoiceValue {
    String(Box<str>),
    Integer(i64),
    Number(f64),
}

/// Option, subcommand or subcommand group of a slash command.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CommandOption {
    #[serde(rename = "type")]
    kind: CommandOptionType,
    name: Box<str>,
    #[serde(skip_serializing_if = "Localizations::is_empty")]
    name_localizations: Localizations,
    description: Box<str>,
    #[serde(skip_serializing_if = "Localizations::is_empty")]
    description_localizations: Localizations,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    required: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    choices: Vec<Choice>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    options: Vec<CommandOption>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    channel_types: Vec<ChannelType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    min_value: Option<serde_json::Number>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_value: Option<serde_json::Number>,
    #[serde(skip_serializing_if = "Option::is_none")]
    min_length: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_length: Option<u16>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    autocomplete: bool,
}

macro_rules! option_ctor {
    ($($(#[$doc:meta])* $fn:ident => $kind:ident;)+) => {$(
        $(#[$doc])*
        pub fn $fn(name: impl AsRef<str>, description: impl AsRef<str>) -> Self {
            Self::new(CommandOptionType::$kind, name.as_ref(), description.as_ref())
        }
    )+};
}

impl CommandOption {
    fn new(kind: CommandOptionType, name: &str, description: &str) -> Self {
        Self {
            kind,
            name: name.into(),
            name_localizations: Localizations::new(),
            description: description.into(),
            description_localizations: Localizations::new(),
            required: false,
            choices: Vec::new(),
            options: Vec::new(),
            channel_types: Vec::new(),
            min_value: None,
            max_value: None,
            min_length: None,
            max_length: None,
            autocomplete: false,
        }
    }

    option_ctor! {
        /// Subcommand; add its options with [`Self::option`].
        subcommand => SubCommand;
        /// Group of subcommands; add them with [`Self::option`].
        group => SubCommandGroup;
        /// String option.
        string => String;
        /// Integer option (`-2^53+1` … `2^53-1`).
        integer => Integer;
        /// Floating-point option.
        number => Number;
        /// Boolean option.
        boolean => Boolean;
        /// User option (resolved user and member).
        user => User;
        /// Channel option; restrict with [`Self::channel_types`].
        channel => Channel;
        /// Role option.
        role => Role;
        /// User or role option.
        mentionable => Mentionable;
        /// File upload option.
        attachment => Attachment;
    }

    /// Option name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Mark as required (required options must come before optional ones).
    #[must_use]
    pub fn required(mut self) -> Self {
        self.required = true;
        self
    }

    /// Nested option (inside a subcommand) or subcommand (inside a group).
    #[must_use]
    pub fn option(mut self, option: Self) -> Self {
        self.options.push(option);
        self
    }

    /// Fixed string choice (string options).
    #[must_use]
    pub fn choice(self, name: impl AsRef<str>, value: impl AsRef<str>) -> Self {
        self.push_choice(
            name.as_ref(),
            ChoiceValue::String(value.as_ref().into()),
            |c| c,
        )
    }

    /// Fixed integer choice (integer options).
    #[must_use]
    pub fn choice_int(self, name: impl AsRef<str>, value: i64) -> Self {
        self.push_choice(name.as_ref(), ChoiceValue::Integer(value), |c| c)
    }

    /// Fixed number choice (number options).
    #[must_use]
    pub fn choice_number(self, name: impl AsRef<str>, value: f64) -> Self {
        self.push_choice(name.as_ref(), ChoiceValue::Number(value), |c| c)
    }

    /// String choice with extra settings (e.g. localized names).
    #[must_use]
    pub fn choice_with(
        self,
        name: impl AsRef<str>,
        value: impl AsRef<str>,
        f: impl FnOnce(Choice) -> Choice,
    ) -> Self {
        self.push_choice(name.as_ref(), ChoiceValue::String(value.as_ref().into()), f)
    }

    fn push_choice(
        mut self,
        name: &str,
        value: ChoiceValue,
        f: impl FnOnce(Choice) -> Choice,
    ) -> Self {
        self.choices.push(f(Choice {
            name: name.into(),
            name_localizations: Localizations::new(),
            value,
        }));
        self
    }

    /// Allowed channel types (channel options).
    #[must_use]
    pub fn channel_types(mut self, types: impl IntoIterator<Item = ChannelType>) -> Self {
        self.channel_types = types.into_iter().collect();
        self
    }

    /// Minimum value (integer options).
    #[must_use]
    pub fn min_int(mut self, value: i64) -> Self {
        self.min_value = Some(value.into());
        self
    }

    /// Maximum value (integer options).
    #[must_use]
    pub fn max_int(mut self, value: i64) -> Self {
        self.max_value = Some(value.into());
        self
    }

    /// Minimum value (number options). Non-finite values are ignored.
    #[must_use]
    pub fn min_number(mut self, value: f64) -> Self {
        self.min_value = serde_json::Number::from_f64(value);
        self
    }

    /// Maximum value (number options). Non-finite values are ignored.
    #[must_use]
    pub fn max_number(mut self, value: f64) -> Self {
        self.max_value = serde_json::Number::from_f64(value);
        self
    }

    /// Minimum length (string options, 0–6000).
    #[must_use]
    pub fn min_length(mut self, len: u16) -> Self {
        self.min_length = Some(len);
        self
    }

    /// Maximum length (string options, 1–6000).
    #[must_use]
    pub fn max_length(mut self, len: u16) -> Self {
        self.max_length = Some(len);
        self
    }

    /// Enable autocomplete (string, integer, number; not together with choices).
    #[must_use]
    pub fn autocomplete(mut self) -> Self {
        self.autocomplete = true;
        self
    }

    /// Localized name.
    #[must_use]
    pub fn name_localization(mut self, locale: impl AsRef<str>, name: impl AsRef<str>) -> Self {
        self.name_localizations
            .insert(locale.as_ref().into(), name.as_ref().into());
        self
    }

    /// Localized description.
    #[must_use]
    pub fn description_localization(
        mut self,
        locale: impl AsRef<str>,
        description: impl AsRef<str>,
    ) -> Self {
        self.description_localizations
            .insert(locale.as_ref().into(), description.as_ref().into());
        self
    }

    fn total_chars(&self) -> usize {
        let choices: usize = self
            .choices
            .iter()
            .map(|c| {
                c.name.chars().count()
                    + match &c.value {
                        ChoiceValue::String(s) => s.chars().count(),
                        ChoiceValue::Integer(i) => i.to_string().len(),
                        ChoiceValue::Number(n) => n.to_string().len(),
                    }
            })
            .sum();
        self.name.chars().count()
            + self.description.chars().count()
            + choices
            + self.options.iter().map(Self::total_chars).sum::<usize>()
    }

    fn is_subcommand(&self) -> bool {
        matches!(
            self.kind,
            CommandOptionType::SubCommand | CommandOptionType::SubCommandGroup
        )
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Level {
    Top,
    Group,
    Subcommand,
}

fn check_options(options: &[CommandOption], level: Level) -> Result<(), Box<str>> {
    if options.len() > MAX_OPTIONS {
        return Err(format!("{} options; max. {MAX_OPTIONS}", options.len()).into());
    }
    let subs = options.iter().filter(|o| o.is_subcommand()).count();
    if subs != 0 && subs != options.len() {
        return Err("subcommands and plain options cannot be mixed on one level".into());
    }
    let mut names = HashSet::new();
    let mut seen_optional = false;
    for o in options {
        let what = format!("option '{}'", o.name);
        check_slash_name("option", &o.name)?;
        check_len(&format!("{what} description"), &o.description, 1, 100)?;
        for (locale, name) in &o.name_localizations {
            check_slash_name(&format!("localized ({locale}) option"), name)?;
        }
        for (locale, desc) in &o.description_localizations {
            check_len(
                &format!("localized ({locale}) {what} description"),
                desc,
                1,
                100,
            )?;
        }
        if !names.insert(&*o.name) {
            return Err(format!("duplicate {what}").into());
        }
        check_nesting(o, level, &what)?;
        if o.is_subcommand() && o.required {
            return Err(format!("subcommand '{}' cannot be required", o.name).into());
        }
        if o.required && seen_optional {
            return Err(format!("required {what} must come before optional options").into());
        }
        seen_optional |= !o.required;
    }
    Ok(())
}

fn check_nesting(o: &CommandOption, level: Level, what: &str) -> Result<(), Box<str>> {
    match o.kind {
        CommandOptionType::SubCommandGroup => {
            if level != Level::Top {
                return Err(format!("subcommand group '{}' must be top-level", o.name).into());
            }
            if o.options
                .iter()
                .any(|s| s.kind != CommandOptionType::SubCommand)
            {
                return Err(format!("group '{}' may only contain subcommands", o.name).into());
            }
            check_options(&o.options, Level::Group)
        }
        CommandOptionType::SubCommand => {
            if level == Level::Subcommand {
                return Err(format!("subcommand '{}' nested too deep", o.name).into());
            }
            check_options(&o.options, Level::Subcommand)
        }
        _ => {
            if level == Level::Group {
                return Err(format!("group may only contain subcommands, found {what}").into());
            }
            if !o.options.is_empty() {
                return Err(format!("{what} cannot have nested options").into());
            }
            check_value_option(o, what)
        }
    }
}

fn check_value_option(o: &CommandOption, what: &str) -> Result<(), Box<str>> {
    use CommandOptionType as T;
    let choosable = matches!(o.kind, T::String | T::Integer | T::Number);
    if !o.choices.is_empty() {
        if !choosable {
            return Err(format!("{what}: choices need a string, integer or number option").into());
        }
        if o.autocomplete {
            return Err(format!("{what}: autocomplete and choices cannot be combined").into());
        }
        check_choices(o, what)?;
    }
    if o.autocomplete && !choosable {
        return Err(
            format!("{what}: autocomplete needs a string, integer or number option").into(),
        );
    }
    if (o.min_value.is_some() || o.max_value.is_some()) && !matches!(o.kind, T::Integer | T::Number)
    {
        return Err(format!("{what}: min/max value needs an integer or number option").into());
    }
    if let (Some(min), Some(max)) = (&o.min_value, &o.max_value)
        && min.as_f64() > max.as_f64()
    {
        return Err(format!("{what}: min value is greater than max value").into());
    }
    if o.min_length.is_some() || o.max_length.is_some() {
        if o.kind != T::String {
            return Err(format!("{what}: min/max length needs a string option").into());
        }
        if o.min_length.is_some_and(|l| l > 6000)
            || o.max_length.is_some_and(|l| !(1..=6000).contains(&l))
        {
            return Err(format!("{what}: length limits must be min 0–6000, max 1–6000").into());
        }
        if let (Some(min), Some(max)) = (o.min_length, o.max_length)
            && min > max
        {
            return Err(format!("{what}: min length is greater than max length").into());
        }
    }
    if !o.channel_types.is_empty() && o.kind != T::Channel {
        return Err(format!("{what}: channel types need a channel option").into());
    }
    Ok(())
}

fn check_choices(o: &CommandOption, what: &str) -> Result<(), Box<str>> {
    use CommandOptionType as T;
    if o.choices.len() > MAX_OPTIONS {
        return Err(format!("{what}: {} choices; max. {MAX_OPTIONS}", o.choices.len()).into());
    }
    for c in &o.choices {
        check_len(&format!("{what} choice name"), &c.name, 1, 100)?;
        for (locale, name) in &c.name_localizations {
            check_len(
                &format!("{what} localized ({locale}) choice name"),
                name,
                1,
                100,
            )?;
        }
        let ok = match (&c.value, o.kind) {
            (ChoiceValue::String(v), T::String) => {
                check_len(&format!("{what} choice value"), v, 1, 100)?;
                true
            }
            (ChoiceValue::Integer(_), T::Integer)
            | (ChoiceValue::Integer(_) | ChoiceValue::Number(_), T::Number) => true,
            _ => false,
        };
        if !ok {
            return Err(format!("{what}: choice '{}' has the wrong value type", c.name).into());
        }
    }
    Ok(())
}

fn check_len(what: &str, value: &str, min: usize, max: usize) -> Result<(), Box<str>> {
    let n = value.chars().count();
    if (min..=max).contains(&n) {
        Ok(())
    } else {
        Err(format!("{what} is {n} characters; must be {min}–{max}").into())
    }
}

/// Slash command and option names: 1–32 characters out of letters, digits,
/// `-`, `_`, `'` (and combining marks for scripts like Devanagari/Thai);
/// letters with a lowercase form must be lowercase.
fn check_slash_name(what: &str, name: &str) -> Result<(), Box<str>> {
    check_len(&format!("{what} name"), name, 1, 32)?;
    if let Some(c) = name
        .chars()
        .find(|&c| !(c.is_alphanumeric() || matches!(c, '-' | '_' | '\'') || is_mark(c)))
    {
        return Err(format!("{what} name '{name}' contains invalid character {c:?}").into());
    }
    if name
        .chars()
        .any(|c| c.to_lowercase().ne(std::iter::once(c)))
    {
        return Err(format!("{what} name '{name}' must be lowercase").into());
    }
    Ok(())
}

/// Combining marks used by Devanagari and Thai (vowel signs etc.).
fn is_mark(c: char) -> bool {
    matches!(c,
        '\u{0900}'..='\u{0903}'
        | '\u{093A}'..='\u{094F}'
        | '\u{0951}'..='\u{0957}'
        | '\u{0962}'..='\u{0963}'
        | '\u{0E31}'
        | '\u{0E34}'..='\u{0E3A}'
        | '\u{0E47}'..='\u{0E4E}')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn err(c: &Command) -> String {
        c.validate().unwrap_err().to_string()
    }

    #[test]
    fn serializes_minimal_json() {
        let c = Command::slash("ping", "Pong!");
        assert_eq!(
            serde_json::to_value(&c).unwrap(),
            serde_json::json!({"type": 1, "name": "ping", "description": "Pong!"})
        );
        let u = Command::user("User Info");
        assert_eq!(
            serde_json::to_value(&u).unwrap(),
            serde_json::json!({"type": 2, "name": "User Info"})
        );
    }

    #[test]
    fn serializes_full_command() {
        let c = Command::slash("ban", "Ban a member")
            .name_localization("de", "bannen")
            .default_member_permissions(Permissions::BAN_MEMBERS)
            .integration_types([ApplicationIntegrationType::GuildInstall])
            .contexts([InteractionContextType::Guild])
            .option(CommandOption::user("user", "Who").required())
            .option(
                CommandOption::integer("days", "Delete messages")
                    .choice_int("none", 0)
                    .choice_int("week", 7),
            )
            .option(CommandOption::string("reason", "Why").max_length(512));
        c.validate().unwrap();
        assert_eq!(
            serde_json::to_value(&c).unwrap(),
            serde_json::json!({
                "type": 1, "name": "ban", "description": "Ban a member",
                "name_localizations": {"de": "bannen"},
                "default_member_permissions": "4",
                "integration_types": [0], "contexts": [0],
                "options": [
                    {"type": 6, "name": "user", "description": "Who", "required": true},
                    {"type": 4, "name": "days", "description": "Delete messages",
                     "choices": [{"name": "none", "value": 0}, {"name": "week", "value": 7}]},
                    {"type": 3, "name": "reason", "description": "Why", "max_length": 512}
                ]
            })
        );
    }

    #[test]
    fn names() {
        assert!(Command::slash("grüßen", "x").validate().is_ok());
        assert!(Command::slash("it's-a_1", "x").validate().is_ok());
        assert!(Command::slash("नमस्ते", "x").validate().is_ok());
        assert!(err(&Command::slash("Ping", "x")).contains("must be lowercase"));
        assert!(err(&Command::slash("hello world", "x")).contains("invalid character ' '"));
        assert!(err(&Command::slash("", "x")).contains("0 characters"));
        assert!(err(&Command::slash("a".repeat(33), "x")).contains("33 characters"));
        assert!(err(&Command::slash("ping", "")).contains("description is 0"));
        assert!(Command::user("User Info").validate().is_ok());
        assert!(Command::message("Report Message!").validate().is_ok());
        assert!(err(&Command::slash("ping", "x").name_localization("de", "Ping")).contains("(de)"));
    }

    #[test]
    fn option_rules() {
        let c = Command::slash("x", "x")
            .option(CommandOption::string("a", "a"))
            .option(CommandOption::string("b", "b").required());
        assert!(err(&c).contains("must come before optional"));

        let c = Command::slash("x", "x")
            .option(CommandOption::string("a", "a"))
            .option(CommandOption::string("a", "a"));
        assert!(err(&c).contains("duplicate"));

        let c = Command::slash("x", "x")
            .option(CommandOption::subcommand("s", "s"))
            .option(CommandOption::string("a", "a"));
        assert!(err(&c).contains("cannot be mixed"));

        let mut c = Command::slash("x", "x");
        for i in 0..26 {
            c = c.option(CommandOption::boolean(format!("o{i}"), "o"));
        }
        assert!(err(&c).contains("26 options"));

        let c = Command::slash("x", "x").option(CommandOption::boolean("b", "b").choice("y", "y"));
        assert!(err(&c).contains("choices need"));

        let c = Command::slash("x", "x").option(
            CommandOption::string("s", "s")
                .choice("y", "y")
                .autocomplete(),
        );
        assert!(err(&c).contains("cannot be combined"));

        let c = Command::slash("x", "x").option(CommandOption::integer("i", "i").choice("y", "y"));
        assert!(err(&c).contains("wrong value type"));

        let c = Command::slash("x", "x")
            .option(CommandOption::string("s", "s").min_length(10).max_length(5));
        assert!(err(&c).contains("min length is greater"));

        let c = Command::slash("x", "x").option(CommandOption::integer("i", "i").max_length(5));
        assert!(err(&c).contains("needs a string option"));

        let c = Command::user("U").option(CommandOption::string("s", "s"));
        assert!(err(&c).contains("no options"));
    }

    #[test]
    fn nesting() {
        let ok = Command::slash("cfg", "Config").option(
            CommandOption::group("role", "Roles").option(
                CommandOption::subcommand("add", "Add")
                    .option(CommandOption::role("r", "r").required()),
            ),
        );
        ok.validate().unwrap();

        let deep = Command::slash("x", "x").option(
            CommandOption::subcommand("a", "a").option(CommandOption::subcommand("b", "b")),
        );
        assert!(err(&deep).contains("too deep"));

        let group_in_sub = Command::slash("x", "x")
            .option(CommandOption::subcommand("a", "a").option(CommandOption::group("g", "g")));
        assert!(err(&group_in_sub).contains("must be top-level"));

        let plain_in_group = Command::slash("x", "x")
            .option(CommandOption::group("g", "g").option(CommandOption::string("s", "s")));
        assert!(err(&plain_in_group).contains("only contain subcommands"));
    }

    #[test]
    fn total_char_limit() {
        let mut opt = CommandOption::string("s", "s");
        for i in 0..25 {
            opt = opt.choice(format!("{i:0>100}"), format!("{i:0>100}"));
        }
        let c = Command::slash("x", "x").option(opt);
        assert!(err(&c).contains("max. 4000"));
    }
}
