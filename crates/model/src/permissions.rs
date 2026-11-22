//! Permission bitfield (topics/permissions.mdx, "Bitwise Permission Flags").

use std::str::FromStr;

use crate::error::ParseError;
use crate::serde_util::bitflags_serde_string;

bitflags::bitflags! {
    /// Guild and channel permissions. Serialized as a decimal string.
    ///
    /// Unknown bits (permissions Discord adds later) are kept.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
    pub struct Permissions: u64 {
        /// Create instant invites.
        const CREATE_INSTANT_INVITE = 1 << 0;
        /// Kick members. Requires 2FA on guilds with MFA.
        const KICK_MEMBERS = 1 << 1;
        /// Ban members. Requires 2FA on guilds with MFA.
        const BAN_MEMBERS = 1 << 2;
        /// All permissions, bypasses channel overwrites. Requires 2FA on guilds with MFA.
        const ADMINISTRATOR = 1 << 3;
        /// Manage and edit channels. Requires 2FA on guilds with MFA.
        const MANAGE_CHANNELS = 1 << 4;
        /// Manage and edit the guild. Requires 2FA on guilds with MFA.
        const MANAGE_GUILD = 1 << 5;
        /// Add new reactions to messages.
        const ADD_REACTIONS = 1 << 6;
        /// View audit logs.
        const VIEW_AUDIT_LOG = 1 << 7;
        /// Use priority speaker in a voice channel.
        const PRIORITY_SPEAKER = 1 << 8;
        /// Go live.
        const STREAM = 1 << 9;
        /// View a channel (read messages, join voice).
        const VIEW_CHANNEL = 1 << 10;
        /// Send messages and create forum threads.
        const SEND_MESSAGES = 1 << 11;
        /// Send `/tts` messages.
        const SEND_TTS_MESSAGES = 1 << 12;
        /// Delete messages of others. Requires 2FA on guilds with MFA.
        const MANAGE_MESSAGES = 1 << 13;
        /// Links are auto-embedded.
        const EMBED_LINKS = 1 << 14;
        /// Upload images and files.
        const ATTACH_FILES = 1 << 15;
        /// Read message history.
        const READ_MESSAGE_HISTORY = 1 << 16;
        /// Use `@everyone` and `@here`.
        const MENTION_EVERYONE = 1 << 17;
        /// Use custom emojis from other servers.
        const USE_EXTERNAL_EMOJIS = 1 << 18;
        /// View guild insights.
        const VIEW_GUILD_INSIGHTS = 1 << 19;
        /// Join a voice channel.
        const CONNECT = 1 << 20;
        /// Speak in a voice channel.
        const SPEAK = 1 << 21;
        /// Mute members in a voice channel.
        const MUTE_MEMBERS = 1 << 22;
        /// Deafen members in a voice channel.
        const DEAFEN_MEMBERS = 1 << 23;
        /// Move members between voice channels.
        const MOVE_MEMBERS = 1 << 24;
        /// Use voice activity detection.
        const USE_VAD = 1 << 25;
        /// Change own nickname.
        const CHANGE_NICKNAME = 1 << 26;
        /// Change nicknames of others.
        const MANAGE_NICKNAMES = 1 << 27;
        /// Manage roles. Requires 2FA on guilds with MFA.
        const MANAGE_ROLES = 1 << 28;
        /// Manage webhooks. Requires 2FA on guilds with MFA.
        const MANAGE_WEBHOOKS = 1 << 29;
        /// Edit and delete emojis, stickers and soundboard sounds. Requires 2FA on guilds with MFA.
        const MANAGE_GUILD_EXPRESSIONS = 1 << 30;
        /// Use application commands.
        const USE_APPLICATION_COMMANDS = 1 << 31;
        /// Request to speak in stage channels.
        const REQUEST_TO_SPEAK = 1 << 32;
        /// Edit and delete scheduled events of all users.
        const MANAGE_EVENTS = 1 << 33;
        /// Manage threads. Requires 2FA on guilds with MFA.
        const MANAGE_THREADS = 1 << 34;
        /// Create public and announcement threads.
        const CREATE_PUBLIC_THREADS = 1 << 35;
        /// Create private threads.
        const CREATE_PRIVATE_THREADS = 1 << 36;
        /// Use custom stickers from other servers.
        const USE_EXTERNAL_STICKERS = 1 << 37;
        /// Send messages in threads.
        const SEND_MESSAGES_IN_THREADS = 1 << 38;
        /// Use activities.
        const USE_EMBEDDED_ACTIVITIES = 1 << 39;
        /// Time out members. Requires 2FA on guilds with MFA.
        const MODERATE_MEMBERS = 1 << 40;
        /// View role subscription insights. Requires 2FA on guilds with MFA.
        const VIEW_CREATOR_MONETIZATION_ANALYTICS = 1 << 41;
        /// Use the soundboard.
        const USE_SOUNDBOARD = 1 << 42;
        /// Create emojis, stickers and soundboard sounds.
        const CREATE_GUILD_EXPRESSIONS = 1 << 43;
        /// Create scheduled events.
        const CREATE_EVENTS = 1 << 44;
        /// Use soundboard sounds from other servers.
        const USE_EXTERNAL_SOUNDS = 1 << 45;
        /// Send voice messages.
        const SEND_VOICE_MESSAGES = 1 << 46;
        /// Set the voice channel status.
        const SET_VOICE_CHANNEL_STATUS = 1 << 48;
        /// Send polls.
        const SEND_POLLS = 1 << 49;
        /// User-installed apps may send public responses.
        const USE_EXTERNAL_APPS = 1 << 50;
        /// Pin and unpin messages.
        const PIN_MESSAGES = 1 << 51;
        /// Bypass slowmode.
        const BYPASS_SLOWMODE = 1 << 52;
    }
}

bitflags_serde_string!(Permissions);

impl FromStr for Permissions {
    type Err = ParseError;

    /// Parses the decimal string form Discord uses.
    fn from_str(s: &str) -> Result<Self, ParseError> {
        s.parse()
            .map(Self::from_bits_retain)
            .map_err(|_| ParseError::Bitfield)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bits_match_the_docs() {
        assert_eq!(Permissions::ADMINISTRATOR.bits(), 0x8);
        assert_eq!(
            Permissions::USE_APPLICATION_COMMANDS.bits(),
            0x0000_0000_8000_0000
        );
        assert_eq!(
            Permissions::SET_VOICE_CHANNEL_STATUS.bits(),
            0x0001_0000_0000_0000
        );
        assert_eq!(Permissions::BYPASS_SLOWMODE.bits(), 0x0010_0000_0000_0000);
    }

    #[test]
    fn serde_string_and_unknown_bits() {
        let p: Permissions = serde_json::from_str("\"2048\"").unwrap();
        assert_eq!(p, Permissions::SEND_MESSAGES);
        assert_eq!(serde_json::to_string(&p).unwrap(), "\"2048\"");

        // Bit 63 is unknown today and must survive a round trip.
        let raw = (1u64 << 63) | 8;
        let p: Permissions = serde_json::from_str(&format!("\"{raw}\"")).unwrap();
        assert!(p.contains(Permissions::ADMINISTRATOR));
        assert_eq!(p.bits(), raw);
        assert_eq!(serde_json::to_string(&p).unwrap(), format!("\"{raw}\""));
    }

    #[test]
    fn accepts_number_rejects_garbage() {
        assert_eq!(
            serde_json::from_str::<Permissions>("8").unwrap(),
            Permissions::ADMINISTRATOR
        );
        assert!(serde_json::from_str::<Permissions>("\"x\"").is_err());
        assert_eq!("x".parse::<Permissions>(), Err(ParseError::Bitfield));
    }
}
