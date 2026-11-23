//! Mention formatting and parsing (topics/message-formatting.mdx).

use crate::id::Id;
use crate::id::marker::{ChannelMarker, CommandMarker, EmojiMarker, RoleMarker, UserMarker};
use crate::timestamp::TimestampStyle;

/// Types that format as a Discord mention.
pub trait Mentionable {
    /// Mention string (`<@id>`, `<#id>`, …).
    fn mention(&self) -> String;
}

impl Mentionable for Id<UserMarker> {
    fn mention(&self) -> String {
        format!("<@{self}>")
    }
}

impl Mentionable for Id<ChannelMarker> {
    fn mention(&self) -> String {
        format!("<#{self}>")
    }
}

impl Mentionable for Id<RoleMarker> {
    fn mention(&self) -> String {
        format!("<@&{self}>")
    }
}

impl Mentionable for Id<EmojiMarker> {
    fn mention(&self) -> String {
        format!("<:_:{self}>")
    }
}

impl Id<CommandMarker> {
    /// Slash-command mention `</name:id>`. `name` may include subcommands
    /// (`"foo bar"`).
    pub fn command_mention(self, name: &str) -> String {
        format!("</{name}:{self}>")
    }
}

impl Id<EmojiMarker> {
    /// Custom emoji mention `<:name:id>` or `<a:name:id>`.
    pub fn emoji_mention(self, name: &str, animated: bool) -> String {
        if animated {
            format!("<a:{name}:{self}>")
        } else {
            format!("<:{name}:{self}>")
        }
    }
}

/// A mention found in message text.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ParsedMention {
    /// `<@id>` or `<@!id>`.
    User(Id<UserMarker>),
    /// `<#id>`.
    Channel(Id<ChannelMarker>),
    /// `<@&id>`.
    Role(Id<RoleMarker>),
    /// `<:name:id>` or `<a:name:id>`.
    Emoji {
        /// Animated (`<a:…>`).
        animated: bool,
        /// Emoji name.
        name: Box<str>,
        /// Emoji ID.
        id: Id<EmojiMarker>,
    },
    /// `</name:id>`.
    SlashCommand {
        /// Command path (`"ping"` or `"foo bar"`).
        name: Box<str>,
        /// Command ID.
        id: Id<CommandMarker>,
    },
    /// `<t:unix>` or `<t:unix:style>`.
    Timestamp {
        /// Unix seconds.
        unix: i64,
        /// Display style (`None` = Discord default).
        style: Option<TimestampStyle>,
    },
    /// `@everyone`.
    Everyone,
    /// `@here`.
    Here,
}

/// Mentions in `text`, left to right. Unknown `<…>` spans are skipped.
pub fn parse_mentions(text: &str) -> Vec<ParsedMention> {
    let mut out = Vec::new();
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'@' && text[i..].starts_with("@everyone") {
            out.push(ParsedMention::Everyone);
            i += "@everyone".len();
            continue;
        }
        if bytes[i] == b'@' && text[i..].starts_with("@here") {
            out.push(ParsedMention::Here);
            i += "@here".len();
            continue;
        }
        if bytes[i] != b'<' {
            i += 1;
            continue;
        }
        let Some(end) = text[i + 1..].find('>').map(|n| i + 1 + n) else {
            break;
        };
        if let Some(parsed) = parse_one(&text[i + 1..end]) {
            out.push(parsed);
        }
        i = end + 1;
    }
    out
}

fn parse_one(inner: &str) -> Option<ParsedMention> {
    if let Some(rest) = inner.strip_prefix("@&") {
        return rest.parse().ok().map(ParsedMention::Role);
    }
    if let Some(rest) = inner.strip_prefix("@!") {
        return rest.parse().ok().map(ParsedMention::User);
    }
    if let Some(rest) = inner.strip_prefix('@') {
        return rest.parse().ok().map(ParsedMention::User);
    }
    if let Some(rest) = inner.strip_prefix('#') {
        return rest.parse().ok().map(ParsedMention::Channel);
    }
    if let Some(rest) = inner.strip_prefix("t:") {
        return parse_timestamp(rest);
    }
    if let Some(rest) = inner.strip_prefix('/') {
        let (name, id) = rest.rsplit_once(':')?;
        return Some(ParsedMention::SlashCommand {
            name: name.into(),
            id: id.parse().ok()?,
        });
    }
    let animated = inner.starts_with("a:");
    let rest = inner
        .strip_prefix("a:")
        .or_else(|| inner.strip_prefix(':'))?;
    let (name, id) = rest.rsplit_once(':')?;
    Some(ParsedMention::Emoji {
        animated,
        name: name.into(),
        id: id.parse().ok()?,
    })
}

fn parse_timestamp(rest: &str) -> Option<ParsedMention> {
    let (num, style) = match rest.split_once(':') {
        Some((n, s)) if s.len() == 1 => (n, TimestampStyle::from_char(s.as_bytes()[0])),
        None => (rest, None),
        Some(_) => return None,
    };
    Some(ParsedMention::Timestamp {
        unix: num.parse().ok()?,
        style,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_and_parses() {
        let user = Id::<UserMarker>::new(1).unwrap();
        let channel = Id::<ChannelMarker>::new(2).unwrap();
        let role = Id::<RoleMarker>::new(3).unwrap();
        let cmd = Id::<CommandMarker>::new(4).unwrap();
        assert_eq!(user.mention(), "<@1>");
        assert_eq!(channel.mention(), "<#2>");
        assert_eq!(role.mention(), "<@&3>");
        assert_eq!(cmd.command_mention("ping"), "</ping:4>");
        let ts = crate::timestamp::Timestamp::from_millis(1_000);
        assert_eq!(ts.markdown(TimestampStyle::Relative), "<t:1:R>");

        let found = parse_mentions(
            "hi <@1> <#2> <@&3> <:wave:5> <a:run:6> </ping:4> <t:1:R> @everyone @here",
        );
        assert_eq!(found.len(), 9);
        assert_eq!(found[0], ParsedMention::User(user));
        assert_eq!(found[7], ParsedMention::Everyone);
        assert_eq!(found[8], ParsedMention::Here);
    }
}
