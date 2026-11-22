//! Gateway intents (events/gateway.mdx, "List of Intents").

use crate::serde_util::bitflags_serde_int;

bitflags::bitflags! {
    /// Which gateway events a session receives. Sent as a number in Identify.
    ///
    /// Unknown bits are kept. `GUILD_MEMBERS`, `GUILD_PRESENCES` and
    /// `MESSAGE_CONTENT` are privileged and must be enabled in the developer
    /// portal, otherwise the gateway closes with 4014.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
    pub struct Intents: u64 {
        /// Guild, role, channel, thread and stage instance events.
        const GUILDS = 1 << 0;
        /// Member add/update/remove. **Privileged.**
        const GUILD_MEMBERS = 1 << 1;
        /// Audit log entries, bans.
        const GUILD_MODERATION = 1 << 2;
        /// Emojis, stickers, soundboard sounds.
        const GUILD_EXPRESSIONS = 1 << 3;
        /// Integrations.
        const GUILD_INTEGRATIONS = 1 << 4;
        /// Webhooks.
        const GUILD_WEBHOOKS = 1 << 5;
        /// Invites.
        const GUILD_INVITES = 1 << 6;
        /// Voice states, voice channel effects.
        const GUILD_VOICE_STATES = 1 << 7;
        /// Presence updates. **Privileged.**
        const GUILD_PRESENCES = 1 << 8;
        /// Guild messages.
        const GUILD_MESSAGES = 1 << 9;
        /// Guild message reactions.
        const GUILD_MESSAGE_REACTIONS = 1 << 10;
        /// Guild typing.
        const GUILD_MESSAGE_TYPING = 1 << 11;
        /// Direct messages.
        const DIRECT_MESSAGES = 1 << 12;
        /// Direct message reactions.
        const DIRECT_MESSAGE_REACTIONS = 1 << 13;
        /// Direct message typing.
        const DIRECT_MESSAGE_TYPING = 1 << 14;
        /// Message content across the APIs. **Privileged.**
        const MESSAGE_CONTENT = 1 << 15;
        /// Scheduled events.
        const GUILD_SCHEDULED_EVENTS = 1 << 16;
        /// Auto moderation rule changes.
        const AUTO_MODERATION_CONFIGURATION = 1 << 20;
        /// Auto moderation actions.
        const AUTO_MODERATION_EXECUTION = 1 << 21;
        /// Guild poll votes.
        const GUILD_MESSAGE_POLLS = 1 << 24;
        /// Direct message poll votes.
        const DIRECT_MESSAGE_POLLS = 1 << 25;
    }
}

bitflags_serde_int!(Intents, u64);

impl Intents {
    /// The privileged intents.
    pub const PRIVILEGED: Self = Self::GUILD_MEMBERS
        .union(Self::GUILD_PRESENCES)
        .union(Self::MESSAGE_CONTENT);

    /// All known intents that need no approval.
    pub const fn non_privileged() -> Self {
        Self::all().difference(Self::PRIVILEGED)
    }

    /// The privileged intents contained in `self`.
    #[must_use]
    pub const fn privileged(self) -> Self {
        self.intersection(Self::PRIVILEGED)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_match_the_docs() {
        assert_eq!(Intents::GUILDS.bits(), 1);
        assert_eq!(Intents::MESSAGE_CONTENT.bits(), 32_768);
        assert_eq!(Intents::DIRECT_MESSAGE_POLLS.bits(), 1 << 25);
        assert_eq!(Intents::PRIVILEGED.bits(), (1 << 1) | (1 << 8) | (1 << 15));
        assert!(Intents::non_privileged().privileged().is_empty());
    }

    #[test]
    fn serde_number_keeps_unknown_bits() {
        let raw = (1u64 << 40) | 1;
        let i: Intents = serde_json::from_str(&raw.to_string()).unwrap();
        assert!(i.contains(Intents::GUILDS));
        assert_eq!(serde_json::to_string(&i).unwrap(), raw.to_string());
    }
}
