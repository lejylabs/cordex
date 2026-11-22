//! Soundboard sounds (resources/soundboard.mdx).

use serde::{Deserialize, Serialize};

use crate::emoji::Emoji;
use crate::id::Id;
use crate::id::marker::{ChannelMarker, EmojiMarker, GuildMarker, SoundMarker, UserMarker};
use crate::serde_util::int_enum;
use crate::user::User;

/// A soundboard sound (default or guild).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SoundboardSound {
    /// Display name.
    pub name: Box<str>,
    /// Sound ID (small integers for default sounds).
    pub sound_id: Id<SoundMarker>,
    /// Volume from 0.0 to 1.0.
    pub volume: f64,
    /// Custom emoji.
    pub emoji_id: Option<Id<EmojiMarker>>,
    /// Unicode emoji.
    pub emoji_name: Option<Box<str>>,
    /// Guild; omitted for default sounds.
    pub guild_id: Option<Id<GuildMarker>>,
    /// Whether the sound can be used (false after lost boosts).
    #[serde(default)]
    pub available: bool,
    /// Creator (`CREATE_GUILD_EXPRESSIONS` / `MANAGE_GUILD_EXPRESSIONS`).
    pub user: Option<User>,
}

/// List-guild-sounds REST wrapper (`{ "items": [...] }`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SoundboardSoundList {
    /// Sounds.
    #[serde(default)]
    pub items: Vec<SoundboardSound>,
}

/// `GUILD_SOUNDBOARD_SOUND_DELETE` payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct GuildSoundboardSoundDelete {
    /// Deleted sound.
    pub sound_id: Id<SoundMarker>,
    /// Guild.
    pub guild_id: Id<GuildMarker>,
}

/// `GUILD_SOUNDBOARD_SOUNDS_UPDATE` / `SOUNDBOARD_SOUNDS` payload.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SoundboardSounds {
    /// Sounds.
    #[serde(default)]
    pub soundboard_sounds: Vec<SoundboardSound>,
    /// Guild.
    pub guild_id: Id<GuildMarker>,
}

int_enum! {
    /// Voice-channel emoji animation.
    pub enum AnimationType: u8 {
        /// Nitro animation.
        Premium = 0,
        /// Standard animation.
        Basic = 1,
    }
}

/// `VOICE_CHANNEL_EFFECT_SEND` payload.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct VoiceChannelEffectSend {
    /// Voice channel.
    pub channel_id: Id<ChannelMarker>,
    /// Guild.
    pub guild_id: Id<GuildMarker>,
    /// User who sent the effect.
    pub user_id: Id<UserMarker>,
    /// Emoji (reaction / soundboard).
    pub emoji: Option<Emoji>,
    /// Animation type.
    pub animation_type: Option<AnimationType>,
    /// Animation ID.
    pub animation_id: Option<u64>,
    /// Soundboard sound (snowflake or small integer).
    pub sound_id: Option<Id<SoundMarker>>,
    /// Sound volume (0.0–1.0).
    pub sound_volume: Option<f64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_sound() {
        let json = r#"{
            "name":"quack","sound_id":"1","volume":1.0,
            "emoji_id":null,"emoji_name":"🦆","available":true
        }"#;
        let sound: SoundboardSound = serde_json::from_str(json).unwrap();
        assert_eq!(sound.sound_id.get(), 1);
        assert!((sound.volume - 1.0).abs() < f64::EPSILON);
        assert!(sound.guild_id.is_none());
    }
}
