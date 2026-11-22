//! Typed access to the options of a command interaction.
//!
//! [`CommandOptions`] walks down to the invoked subcommand, so getters look
//! at the options the user actually filled in:
//!
//! ```
//! # use cordex_interactions::options::CommandOptions;
//! # use cordex_model::interaction::CommandData;
//! let data: CommandData = serde_json::from_str(r#"{
//!     "id": "1", "name": "role", "type": 1,
//!     "options": [{"name": "add", "type": 1, "options": [
//!         {"name": "name", "type": 3, "value": "mods"},
//!         {"name": "color", "type": 4, "value": 255}
//!     ]}]
//! }"#).unwrap();
//! let opts = CommandOptions::new(&data);
//! assert_eq!(opts.subcommand(), ["add"]);
//! assert_eq!(opts.string("name").unwrap(), "mods");
//! assert_eq!(opts.integer_opt("color").unwrap(), Some(255));
//! assert!(opts.string("missing").is_err());
//! ```

use cordex_model::channel::Channel;
use cordex_model::command::CommandOptionType;
use cordex_model::guild::{Member, Role};
use cordex_model::id::Id;
use cordex_model::id::marker::{
    AttachmentMarker, ChannelMarker, GenericMarker, RoleMarker, UserMarker,
};
use cordex_model::interaction::{CommandData, CommandDataOption, CommandOptionValue, Resolved};
use cordex_model::message::Attachment;
use cordex_model::user::User;

use crate::error::InteractionError;

/// A user option with its guild member, if the command ran in a guild.
#[derive(Debug, Clone, Copy)]
pub struct ResolvedUser<'a> {
    /// The user.
    pub user: &'a User,
    /// Partial guild member (no `user`, `deaf`, `mute`), in guilds only.
    pub member: Option<&'a Member>,
}

/// Value of a mentionable option.
#[derive(Debug, Clone, Copy)]
pub enum Mentionable<'a> {
    /// A user (and member).
    User(ResolvedUser<'a>),
    /// A role.
    Role(&'a Role),
}

/// Options of one command interaction, starting at the invoked subcommand.
#[derive(Debug, Clone, Copy)]
pub struct CommandOptions<'a> {
    data: &'a CommandData,
    leaf: &'a [CommandDataOption],
    group: Option<&'a str>,
    sub: Option<&'a str>,
}

impl<'a> CommandOptions<'a> {
    /// Walk to the invoked subcommand (group → subcommand) of `data`.
    pub fn new(data: &'a CommandData) -> Self {
        let mut leaf = data.options.as_slice();
        let mut group = None;
        let mut sub = None;
        if let [first] = leaf
            && first.kind == CommandOptionType::SubCommandGroup
        {
            group = Some(&*first.name);
            leaf = &first.options;
        }
        if let [first] = leaf
            && first.kind == CommandOptionType::SubCommand
        {
            sub = Some(&*first.name);
            leaf = &first.options;
        }
        Self {
            data,
            leaf,
            group,
            sub,
        }
    }

    /// Command name.
    pub fn command(&self) -> &'a str {
        &self.data.name
    }

    /// Invoked group and subcommand names, e.g. `["role", "add"]`, `["add"]`
    /// or `[]`.
    pub fn subcommand(&self) -> Vec<&'a str> {
        self.group.into_iter().chain(self.sub).collect()
    }

    /// Options the user filled in (at the subcommand level).
    pub fn raw(&self) -> &'a [CommandDataOption] {
        self.leaf
    }

    /// Focused option while autocompleting.
    pub fn focused(&self) -> Option<&'a CommandDataOption> {
        self.leaf.iter().find(|o| o.focused)
    }

    /// Target user of a user context-menu command.
    pub fn target_user(&self) -> Result<ResolvedUser<'a>, InteractionError> {
        let id = self
            .data
            .target_id
            .ok_or_else(|| invalid("command has no target (not a context-menu command)"))?;
        self.resolve_user(id.cast(), "target")
    }

    /// Target message of a message context-menu command.
    pub fn target_message(&self) -> Result<&'a cordex_model::Message, InteractionError> {
        let id = self
            .data
            .target_id
            .ok_or_else(|| invalid("command has no target (not a context-menu command)"))?;
        self.resolved()
            .and_then(|r| r.messages.get(&id.cast()))
            .ok_or_else(|| invalid("target message is missing in resolved data"))
    }

    fn value(&self, name: &str) -> Option<&'a CommandOptionValue> {
        self.leaf
            .iter()
            .find(|o| &*o.name == name)
            .and_then(|o| o.value.as_ref())
    }

    fn resolved(&self) -> Option<&'a Resolved> {
        self.data.resolved.as_ref()
    }

    fn id<T>(&self, name: &str) -> Result<Option<Id<T>>, InteractionError> {
        match self.value(name) {
            None => Ok(None),
            Some(CommandOptionValue::String(s)) => s
                .parse()
                .map(Some)
                .map_err(|_| wrong(name, "an ID", "a non-ID string")),
            Some(other) => Err(wrong(name, "an ID", kind_of(other))),
        }
    }

    fn resolve_user(
        &self,
        id: Id<UserMarker>,
        name: &str,
    ) -> Result<ResolvedUser<'a>, InteractionError> {
        let resolved = self.resolved();
        let user = resolved.and_then(|r| r.users.get(&id)).ok_or_else(|| {
            invalid(format!(
                "user for option '{name}' is missing in resolved data"
            ))
        })?;
        Ok(ResolvedUser {
            user,
            member: resolved.and_then(|r| r.members.get(&id)),
        })
    }

    /// Optional string option.
    pub fn string_opt(&self, name: &str) -> Result<Option<&'a str>, InteractionError> {
        match self.value(name) {
            None => Ok(None),
            Some(CommandOptionValue::String(s)) => Ok(Some(s)),
            Some(other) => Err(wrong(name, "a string", kind_of(other))),
        }
    }

    /// Optional integer option.
    pub fn integer_opt(&self, name: &str) -> Result<Option<i64>, InteractionError> {
        match self.value(name) {
            None => Ok(None),
            Some(CommandOptionValue::Integer(i)) => Ok(Some(*i)),
            Some(other) => Err(wrong(name, "an integer", kind_of(other))),
        }
    }

    /// Optional number option (integers are accepted too).
    pub fn number_opt(&self, name: &str) -> Result<Option<f64>, InteractionError> {
        match self.value(name) {
            None => Ok(None),
            Some(CommandOptionValue::Number(n)) => Ok(Some(*n)),
            #[allow(
                clippy::cast_precision_loss,
                reason = "Discord integers are limited to ±2^53, exact in f64"
            )]
            Some(CommandOptionValue::Integer(i)) => Ok(Some(*i as f64)),
            Some(other) => Err(wrong(name, "a number", kind_of(other))),
        }
    }

    /// Optional boolean option.
    pub fn boolean_opt(&self, name: &str) -> Result<Option<bool>, InteractionError> {
        match self.value(name) {
            None => Ok(None),
            Some(CommandOptionValue::Boolean(b)) => Ok(Some(*b)),
            Some(other) => Err(wrong(name, "a boolean", kind_of(other))),
        }
    }

    /// Optional user option with its member.
    pub fn user_opt(&self, name: &str) -> Result<Option<ResolvedUser<'a>>, InteractionError> {
        self.id::<UserMarker>(name)?
            .map(|id| self.resolve_user(id, name))
            .transpose()
    }

    /// Optional role option.
    pub fn role_opt(&self, name: &str) -> Result<Option<&'a Role>, InteractionError> {
        self.id::<RoleMarker>(name)?
            .map(|id| {
                self.resolved()
                    .and_then(|r| r.roles.get(&id))
                    .ok_or_else(|| {
                        invalid(format!(
                            "role for option '{name}' is missing in resolved data"
                        ))
                    })
            })
            .transpose()
    }

    /// Optional channel option (partial channel).
    pub fn channel_opt(&self, name: &str) -> Result<Option<&'a Channel>, InteractionError> {
        self.id::<ChannelMarker>(name)?
            .map(|id| {
                self.resolved()
                    .and_then(|r| r.channels.get(&id))
                    .ok_or_else(|| {
                        invalid(format!(
                            "channel for option '{name}' is missing in resolved data"
                        ))
                    })
            })
            .transpose()
    }

    /// Optional attachment option.
    pub fn attachment_opt(&self, name: &str) -> Result<Option<&'a Attachment>, InteractionError> {
        self.id::<AttachmentMarker>(name)?
            .map(|id| {
                self.resolved()
                    .and_then(|r| r.attachments.get(&id))
                    .ok_or_else(|| {
                        invalid(format!(
                            "attachment for option '{name}' is missing in resolved data"
                        ))
                    })
            })
            .transpose()
    }

    /// Optional mentionable option (user or role).
    pub fn mentionable_opt(&self, name: &str) -> Result<Option<Mentionable<'a>>, InteractionError> {
        let Some(id) = self.id::<GenericMarker>(name)? else {
            return Ok(None);
        };
        if let Some(role) = self.resolved().and_then(|r| r.roles.get(&id.cast())) {
            return Ok(Some(Mentionable::Role(role)));
        }
        self.resolve_user(id.cast(), name)
            .map(|u| Some(Mentionable::User(u)))
    }
}

macro_rules! required {
    ($($(#[$doc:meta])* $req:ident => $opt:ident -> $ty:ty;)+) => {
        impl<'a> CommandOptions<'a> {$(
            $(#[$doc])*
            pub fn $req(&self, name: &str) -> Result<$ty, InteractionError> {
                self.$opt(name)?.ok_or_else(|| missing(name))
            }
        )+}
    };
}

required! {
    /// Required string option.
    string => string_opt -> &'a str;
    /// Required integer option.
    integer => integer_opt -> i64;
    /// Required number option.
    number => number_opt -> f64;
    /// Required boolean option.
    boolean => boolean_opt -> bool;
    /// Required user option with its member.
    user => user_opt -> ResolvedUser<'a>;
    /// Required role option.
    role => role_opt -> &'a Role;
    /// Required channel option.
    channel => channel_opt -> &'a Channel;
    /// Required attachment option.
    attachment => attachment_opt -> &'a Attachment;
    /// Required mentionable option.
    mentionable => mentionable_opt -> Mentionable<'a>;
}

fn kind_of(v: &CommandOptionValue) -> &'static str {
    match v {
        CommandOptionValue::Boolean(_) => "a boolean",
        CommandOptionValue::Integer(_) => "an integer",
        CommandOptionValue::Number(_) => "a number",
        CommandOptionValue::String(_) => "a string",
    }
}

fn invalid(msg: impl Into<Box<str>>) -> InteractionError {
    InteractionError::InvalidOption(msg.into())
}

fn missing(name: &str) -> InteractionError {
    invalid(format!("option '{name}' is missing"))
}

fn wrong(name: &str, expected: &str, got: &str) -> InteractionError {
    invalid(format!("option '{name}' should be {expected}, got {got}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data(json: &str) -> CommandData {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn plain_options_and_errors() {
        let d = data(
            r#"{"id":"1","name":"t","type":1,"options":[
                {"name":"s","type":3,"value":"hi"},
                {"name":"i","type":4,"value":3},
                {"name":"n","type":10,"value":1.5},
                {"name":"b","type":5,"value":true}]}"#,
        );
        let o = CommandOptions::new(&d);
        assert_eq!(o.subcommand(), Vec::<&str>::new());
        assert_eq!(o.string("s").unwrap(), "hi");
        assert_eq!(o.integer("i").unwrap(), 3);
        assert!((o.number("n").unwrap() - 1.5).abs() < f64::EPSILON);
        assert!((o.number("i").unwrap() - 3.0).abs() < f64::EPSILON);
        assert!(o.boolean("b").unwrap());
        assert_eq!(o.string_opt("x").unwrap(), None);
        assert_eq!(
            o.string("x").unwrap_err().to_string(),
            "invalid option: option 'x' is missing"
        );
        assert_eq!(
            o.integer("s").unwrap_err().to_string(),
            "invalid option: option 's' should be an integer, got a string"
        );
    }

    #[test]
    fn group_path_and_resolved() {
        let d = data(
            r#"{"id":"1","name":"cfg","type":1,
            "options":[{"name":"perm","type":2,"options":[{"name":"grant","type":1,"options":[
                {"name":"who","type":6,"value":"10"},
                {"name":"role","type":8,"value":"20"},
                {"name":"any","type":9,"value":"20"}]}]}],
            "resolved":{
                "users":{"10":{"id":"10","username":"a","discriminator":"0","global_name":null,"avatar":null}},
                "members":{"10":{"roles":[],"joined_at":"2024-01-01T00:00:00.000000+00:00","flags":0,"deaf":false,"mute":false}},
                "roles":{"20":{"id":"20","name":"mods","color":0,"hoist":false,"position":1,"permissions":"0","managed":false,"mentionable":true,"flags":0}}
            }}"#,
        );
        let o = CommandOptions::new(&d);
        assert_eq!(o.command(), "cfg");
        assert_eq!(o.subcommand(), ["perm", "grant"]);
        let who = o.user("who").unwrap();
        assert_eq!(&*who.user.username, "a");
        assert!(who.member.is_some());
        assert_eq!(&*o.role("role").unwrap().name, "mods");
        assert!(matches!(
            o.mentionable("any").unwrap(),
            Mentionable::Role(_)
        ));
        assert!(o.channel_opt("nope").unwrap().is_none());
    }

    #[test]
    fn focused_option() {
        let d = data(
            r#"{"id":"1","name":"t","type":1,"options":[
                {"name":"q","type":3,"value":"ab","focused":true}]}"#,
        );
        assert_eq!(&*CommandOptions::new(&d).focused().unwrap().name, "q");
    }
}
