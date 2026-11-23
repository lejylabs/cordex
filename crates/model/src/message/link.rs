//! Discord message link parsing (`https://discord.com/channels/...`).

use crate::id::Id;
use crate::id::marker::{ChannelMarker, GuildMarker, MessageMarker};

/// Parsed Discord message link (`https://discord.com/channels/{guild|@me}/{channel}/{message}`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MessageLink {
    /// Guild ID, or `None` for DM links (`@me`).
    pub guild_id: Option<Id<GuildMarker>>,
    /// Channel ID.
    pub channel_id: Id<ChannelMarker>,
    /// Message ID.
    pub message_id: Id<MessageMarker>,
}

/// Parse a Discord message URL into its typed IDs.
///
/// Accepts `discord.com` and `discordapp.com` (with optional `ptb.` or
/// `canary.` subdomain), `http://` or `https://`, and optional trailing
/// slash, query string or fragment.
pub fn parse_message_link(url: &str) -> Option<MessageLink> {
    let rest = url
        .trim()
        .strip_prefix("https://")
        .or_else(|| url.trim().strip_prefix("http://"))?;
    let (host, path) = rest.split_once('/')?;
    if !matches!(
        host,
        "discord.com"
            | "ptb.discord.com"
            | "canary.discord.com"
            | "discordapp.com"
            | "ptb.discordapp.com"
            | "canary.discordapp.com"
    ) {
        return None;
    }
    let path = path
        .split(['?', '#'])
        .next()
        .unwrap_or(path)
        .trim_end_matches('/');
    let mut parts = path.split('/');
    if parts.next()? != "channels" {
        return None;
    }
    let guild_part = parts.next()?;
    let channel_part = parts.next()?;
    let message_part = parts.next()?;
    if parts.next().is_some() {
        return None;
    }
    let guild_id = if guild_part == "@me" {
        None
    } else {
        Some(guild_part.parse().ok()?)
    };
    let channel_id = channel_part.parse().ok()?;
    let message_id = message_part.parse().ok()?;
    Some(MessageLink {
        guild_id,
        channel_id,
        message_id,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_guild_and_dm_message_links() {
        let guild =
            parse_message_link("https://discord.com/channels/100/200/300").expect("guild link");
        assert_eq!(guild.guild_id, Some(Id::new(100).unwrap()));
        assert_eq!(guild.channel_id, Id::new(200).unwrap());
        assert_eq!(guild.message_id, Id::new(300).unwrap());

        let dm = parse_message_link("https://ptb.discord.com/channels/@me/200/300/?foo=1#bar")
            .expect("dm link");
        assert_eq!(dm.guild_id, None);
        assert_eq!(dm.channel_id, Id::new(200).unwrap());
        assert_eq!(dm.message_id, Id::new(300).unwrap());

        let canary = parse_message_link("https://canary.discordapp.com/channels/1/2/3")
            .expect("canary link");
        assert_eq!(canary.message_id, Id::new(3).unwrap());
    }

    #[test]
    fn rejects_invalid_links() {
        assert!(parse_message_link("https://example.com/channels/1/2/3").is_none());
        assert!(parse_message_link("https://discord.com/channels/1/2").is_none());
        assert!(parse_message_link("https://discord.com/channels/1/2/3/4").is_none());
        assert!(parse_message_link("https://discord.com/channels/0/2/3").is_none());
        assert!(parse_message_link("not a url").is_none());
    }
}
