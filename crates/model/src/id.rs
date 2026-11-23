//! Snowflake IDs.
//!
//! Discord IDs are 64-bit snowflakes, sent as decimal strings in JSON.
//! [`Id<T>`] carries a zero-sized marker so a guild ID cannot be passed
//! where a channel ID is expected.

use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::num::NonZeroU64;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::error::ParseError;
use crate::serde_util::u64_from_str_or_int;

/// Discord epoch (first second of 2015) in Unix milliseconds.
pub const DISCORD_EPOCH_MS: u64 = 1_420_070_400_000;

/// A snowflake ID of the resource kind `T` (a marker from [`marker`]).
///
/// Serializes as a string; deserializes from a string or a number. Zero is
/// rejected, so `Option<Id<T>>` is the same size as `Id<T>`.
pub struct Id<T> {
    value: NonZeroU64,
    // `fn() -> T` keeps `Id<T>` `Send + Sync + Copy` for every marker.
    marker: PhantomData<fn() -> T>,
}

impl<T> Id<T> {
    /// Creates an ID; `None` for zero.
    pub const fn new(value: u64) -> Option<Self> {
        match NonZeroU64::new(value) {
            Some(value) => Some(Self::from_non_zero(value)),
            None => None,
        }
    }

    /// Creates an ID from a non-zero value.
    pub const fn from_non_zero(value: NonZeroU64) -> Self {
        Self {
            value,
            marker: PhantomData,
        }
    }

    /// The raw value.
    pub const fn get(self) -> u64 {
        self.value.get()
    }

    /// Changes the marker type (e.g. a guild ID used as its `@everyone` role ID).
    pub const fn cast<U>(self) -> Id<U> {
        Id::from_non_zero(self.value)
    }

    /// Creation time in Unix milliseconds.
    pub const fn timestamp_ms(self) -> u64 {
        (self.get() >> 22) + DISCORD_EPOCH_MS
    }

    /// Smallest ID created at `timestamp_ms` (Unix milliseconds), for
    /// `before`/`after` pagination: `(timestamp_ms - DISCORD_EPOCH) << 22`.
    /// `None` before the Discord epoch or past the 42-bit timestamp range.
    pub const fn from_timestamp_ms(timestamp_ms: u64) -> Option<Self> {
        match timestamp_ms.checked_sub(DISCORD_EPOCH_MS) {
            Some(since) if since < (1 << 42) => Self::new(since << 22),
            _ => None,
        }
    }

    /// Internal worker ID (bits 17–21).
    pub const fn worker_id(self) -> u8 {
        ((self.get() >> 17) & 0x1F) as u8
    }

    /// Internal process ID (bits 12–16).
    pub const fn process_id(self) -> u8 {
        ((self.get() >> 12) & 0x1F) as u8
    }

    /// Per-process increment (bits 0–11).
    pub const fn increment(self) -> u16 {
        (self.get() & 0xFFF) as u16
    }
}

// Manual impls: derives would require `T: Clone` etc. on the marker.
impl<T> Clone for Id<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for Id<T> {}

impl<T> PartialEq for Id<T> {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

impl<T> Eq for Id<T> {}

impl<T> PartialOrd for Id<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T> Ord for Id<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.value.cmp(&other.value)
    }
}

impl<T> Hash for Id<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.value.hash(state);
    }
}

impl<T> fmt::Debug for Id<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Id({})", self.value)
    }
}

impl<T> fmt::Display for Id<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.value, f)
    }
}

impl<T> FromStr for Id<T> {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        s.parse::<u64>()
            .ok()
            .and_then(Self::new)
            .ok_or(ParseError::Snowflake)
    }
}

impl<T> From<NonZeroU64> for Id<T> {
    fn from(value: NonZeroU64) -> Self {
        Self::from_non_zero(value)
    }
}

impl<T> From<Id<T>> for u64 {
    fn from(id: Id<T>) -> Self {
        id.get()
    }
}

impl<T> Serialize for Id<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        // `collect_str` writes the digits straight into the output (no String).
        serializer.collect_str(&self.value)
    }
}

impl<'de, T> Deserialize<'de> for Id<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = u64_from_str_or_int(deserializer, "a non-zero snowflake")?;
        Self::new(raw).ok_or_else(|| serde::de::Error::custom("snowflake must not be zero"))
    }
}

/// Marker types for [`Id`].
pub mod marker {
    macro_rules! markers {
        ($($(#[$doc:meta])* $name:ident),* $(,)?) => {
            $(
                $(#[$doc])*
                #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
                #[non_exhaustive]
                pub struct $name;
            )*
        };
    }

    markers! {
        /// Application.
        ApplicationMarker,
        /// Application command.
        CommandMarker,
        /// Attachment.
        AttachmentMarker,
        /// Audit log entry.
        AuditLogEntryMarker,
        /// Auto moderation rule.
        AutoModerationRuleMarker,
        /// Channel (including threads).
        ChannelMarker,
        /// Emoji.
        EmojiMarker,
        /// Entitlement.
        EntitlementMarker,
        /// Generic ID whose kind is not known (e.g. mentionable or raw values).
        GenericMarker,
        /// Guild.
        GuildMarker,
        /// Integration.
        IntegrationMarker,
        /// Social SDK lobby.
        LobbyMarker,
        /// Interaction.
        InteractionMarker,
        /// Message.
        MessageMarker,
        /// Role.
        RoleMarker,
        /// Guild scheduled event.
        ScheduledEventMarker,
        /// SKU.
        SkuMarker,
        /// Soundboard sound.
        SoundMarker,
        /// Stage instance.
        StageMarker,
        /// Sticker.
        StickerMarker,
        /// Sticker pack.
        StickerPackMarker,
        /// Forum / media channel tag.
        TagMarker,
        /// Subscription.
        SubscriptionMarker,
        /// Team.
        TeamMarker,
        /// User.
        UserMarker,
        /// Webhook.
        WebhookMarker,
    }
}

#[cfg(test)]
mod tests {
    use super::marker::{ChannelMarker, GuildMarker};
    use super::*;

    // Classic example ID; parts computed with the formulas from reference.mdx
    // ("Snowflake ID Format Structure").
    const DOC_ID: u64 = 175_928_847_299_117_063;

    #[test]
    fn parts_match_the_docs_example() {
        let id = Id::<GuildMarker>::new(DOC_ID).unwrap();
        assert_eq!(id.timestamp_ms(), 1_462_015_105_796);
        assert_eq!(id.worker_id(), 1);
        assert_eq!(id.process_id(), 0);
        assert_eq!(id.increment(), 7);
    }

    #[test]
    fn serde_string_and_number() {
        let id: Id<ChannelMarker> = serde_json::from_str("\"175928847299117063\"").unwrap();
        assert_eq!(id.get(), DOC_ID);
        let id: Id<ChannelMarker> = serde_json::from_str("175928847299117063").unwrap();
        assert_eq!(id.get(), DOC_ID);
        assert_eq!(
            serde_json::to_string(&id).unwrap(),
            "\"175928847299117063\""
        );
    }

    #[test]
    fn rejects_zero_negative_and_garbage() {
        for bad in ["\"0\"", "0", "-1", "\"abc\"", "\"\"", "1.5", "null"] {
            assert!(
                serde_json::from_str::<Id<GuildMarker>>(bad).is_err(),
                "{bad}"
            );
        }
        assert_eq!("0".parse::<Id<GuildMarker>>(), Err(ParseError::Snowflake));
    }

    #[test]
    fn option_has_no_size_overhead() {
        assert_eq!(size_of::<Option<Id<GuildMarker>>>(), size_of::<u64>());
    }

    #[test]
    fn from_timestamp_round_trips() {
        let id = Id::<GuildMarker>::from_timestamp_ms(1_462_015_105_796).unwrap();
        assert_eq!(id.timestamp_ms(), 1_462_015_105_796);
        assert_eq!(id.increment(), 0);
        assert!(Id::<GuildMarker>::from_timestamp_ms(DISCORD_EPOCH_MS).is_none());
        assert!(Id::<GuildMarker>::from_timestamp_ms(0).is_none());
    }

    #[test]
    fn cast_keeps_value() {
        let guild = Id::<GuildMarker>::new(5).unwrap();
        assert_eq!(guild.cast::<ChannelMarker>().get(), 5);
    }
}
